use crate::domain::catalog::Catalog;
use crate::domain::error::{DomainError, Result};
use crate::domain::facts::Facts;
use crate::domain::resource::{Ensure, FileResource, ExecResource, MetaKind, MetaResource, Resource};
use crate::infrastructure::hiera::Hiera;
use rhai::{Dynamic, Engine, Map, NativeCallContext, Scope};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

thread_local! {
    static CURRENT_EXEC_CTX: RefCell<Option<ExecutionContext>> = RefCell::new(None);
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub resources: Arc<Mutex<Vec<Resource>>>,
    pub included_modules: Arc<Mutex<HashSet<String>>>,
    pub module_stack: Arc<Mutex<Vec<String>>>,
    pub facts: Arc<Facts>,
}

impl ExecutionContext {
    fn new(facts: Facts) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            included_modules: Arc::new(Mutex::new(HashSet::new())),
            module_stack: Arc::new(Mutex::new(Vec::new())),
            facts: Arc::new(facts),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ModuleHandle {
    pub name: String,
    pub start_id: String,
    pub end_id: String,
}

#[derive(Clone)]
pub struct PupoxideEngine {
    engine: Arc<Engine>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
    hiera: Arc<Option<Hiera>>,
}

pub struct PupoxideEngineBuilder {
    engine: Engine,
    hiera: Option<Hiera>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl PupoxideEngineBuilder {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            hiera: None,
            module_path: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_hiera(mut self, hiera: Hiera) -> Self {
        self.hiera = Some(hiera);
        self
    }

    pub fn with_module_path(mut self, path: PathBuf) -> Self {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
        self
    }

    pub fn register_defaults(mut self) -> Self {
        let hiera_arc = Arc::new(self.hiera.clone());

        Self::register_types(&mut self.engine);
        Self::register_hiera_functions(&mut self.engine, hiera_arc);
        Self::register_operators(&mut self.engine);
        Self::register_module_functions(&mut self.engine, self.module_path.clone());
        Self::register_resource_functions(&mut self.engine);

        self
    }

    pub fn build(self) -> PupoxideEngine {
        PupoxideEngine {
            engine: Arc::new(self.engine),
            module_path: self.module_path,
            hiera: Arc::new(self.hiera),
        }
    }

    fn register_types(engine: &mut Engine) {
        engine.register_type_with_name::<Resource>("Resource");
        engine.register_type_with_name::<ModuleHandle>("ModuleHandle");
        engine
            .register_type_with_name::<Ensure>("Ensure")
            .register_fn("present", || Ensure::Present)
            .register_fn("absent", || Ensure::Absent);

        engine.register_fn("to_ensure", |s: String| match s.as_str() {
            "absent" => Ensure::Absent,
            _ => Ensure::Present,
        });
    }

    fn register_hiera_functions(engine: &mut Engine, hiera: Arc<Option<Hiera>>) {
        let h = hiera.clone();
        engine.register_fn("lookup", move |key: String| -> Dynamic {
            Self::hiera_lookup_internal(&h, key, None)
        });

        let h2 = hiera.clone();
        engine.register_fn(
            "lookup",
            move |key: String, default_val: Dynamic| -> Dynamic {
                Self::hiera_lookup_internal(&h2, key, Some(default_val))
            },
        );
    }

    fn hiera_lookup_internal(
        hiera: &Option<Hiera>,
        key: String,
        default_val: Option<Dynamic>,
    ) -> Dynamic {
        let exec_ctx =
            CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("Execution context must be set"));

        if let Some(hiera_impl) = hiera.as_ref() {
            if let Some(val) = hiera_impl.lookup(&key, &exec_ctx.facts) {
                return match val {
                    serde_yaml::Value::String(s) => Dynamic::from(s),
                    serde_yaml::Value::Bool(b) => Dynamic::from(b),
                    serde_yaml::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            Dynamic::from(i)
                        } else if let Some(f) = n.as_f64() {
                            Dynamic::from(f)
                        } else {
                            Dynamic::from(n.to_string())
                        }
                    }
                    _ => Dynamic::from(serde_yaml::to_string(&val).unwrap_or_default()),
                };
            }
        }
        default_val.unwrap_or(Dynamic::UNIT)
    }

    fn register_resource_functions(engine: &mut Engine) {
        // 'directory' function
        engine.register_fn("directory", move |path: String, params: Map| {
            let exec_ctx = Self::get_exec_ctx();
            let ensure = Self::extract_ensure(&params);
            let dependencies = Self::extract_dependencies(&params, &exec_ctx);
            let backup = Self::extract_bool(&params, "backup", false);

            let resource = Resource::Directory(crate::domain::resource::DirectoryResource {
                id: format!("Directory[{}]", path),
                path: PathBuf::from(path),
                ensure,
                dependencies,
                backup,
                owner: Self::extract_string(&params, "owner"),
                group: Self::extract_string(&params, "group"),
                mode: Self::extract_string(&params, "mode"),
            });

            Self::add_resource(&exec_ctx, resource)
        });

        // 'exec' function
        engine.register_fn("exec", move |command: String, params: Map| {
            let exec_ctx = Self::get_exec_ctx();
            let dependencies = Self::extract_dependencies(&params, &exec_ctx);

            let creates = Self::extract_string(&params, "creates").map(PathBuf::from);
            let unless = Self::extract_string(&params, "unless");
            let cwd = Self::extract_string(&params, "cwd").map(PathBuf::from);

            let environment = params
                .get("environment")
                .and_then(|v| v.clone().try_cast::<Map>())
                .map(|map| {
                    map.into_iter()
                        .filter_map(|(k, v)| v.try_cast::<String>().map(|s| (k.to_string(), s)))
                        .collect::<HashMap<String, String>>()
                });

            let resource = Resource::Exec(ExecResource {
                id: format!("Exec[{}]", command),
                command,
                creates,
                unless,
                cwd,
                environment,
                dependencies,
                backup: false,
            });

            Self::add_resource(&exec_ctx, resource)
        });

        // 'file' function
        engine.register_fn("file", move |path: String, params: Map| {
            let exec_ctx = Self::get_exec_ctx();
            let ensure = Self::extract_ensure(&params);
            let dependencies = Self::extract_dependencies(&params, &exec_ctx);
            let backup = Self::extract_bool(&params, "backup", true);

            let content = Self::extract_string(&params, "content");
            let max_backup_size = params.get("max_backup_size").and_then(|v| {
                v.clone()
                    .try_cast::<i64>()
                    .map(|i| i as u64)
                    .or_else(|| v.clone().try_cast::<u64>())
            });

            let resource = Resource::File(FileResource {
                id: format!("File[{}]", path),
                path: PathBuf::from(path),
                ensure,
                content,
                dependencies,
                backup,
                max_backup_size,
                owner: Self::extract_string(&params, "owner"),
                group: Self::extract_string(&params, "group"),
                mode: Self::extract_string(&params, "mode"),
            });

            Self::add_resource(&exec_ctx, resource)
        });
    }

    fn register_operators(engine: &mut Engine) {
        engine
            .register_custom_operator("->", 60)
            .expect("Failed to register custom operator");

        engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
            Self::add_dependency_between_ids(&lhs.id().to_string(), &rhs.id().to_string());
            rhs
        });

        engine.register_fn("->", move |lhs: ModuleHandle, rhs: ModuleHandle| {
            Self::add_dependency_between_ids(&lhs.end_id, &rhs.start_id);
            rhs
        });

        engine.register_fn("->", move |lhs: Resource, rhs: ModuleHandle| {
            Self::add_dependency_between_ids(&lhs.id().to_string(), &rhs.start_id);
            rhs
        });

        engine.register_fn("->", move |lhs: ModuleHandle, rhs: Resource| {
            Self::add_dependency_between_ids(&lhs.end_id, &rhs.id().to_string());
            rhs
        });
    }

    fn register_module_functions(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
        let m_path = module_path.clone();
        engine.register_fn(
            "include",
            move |ctx: NativeCallContext,
                  name: String|
                  -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
                let exec_ctx = Self::get_exec_ctx();

                let handle = ModuleHandle {
                    name: name.clone(),
                    start_id: format!("ModuleStart[{}]", name),
                    end_id: format!("ModuleEnd[{}]", name),
                };

                let mut included = exec_ctx
                    .included_modules
                    .lock()
                    .expect("Failed to lock included modules");
                if included.contains(&name) {
                    return Ok(handle);
                }
                included.insert(name.clone());
                drop(included);

                let base = m_path.lock().expect("Failed to lock module path");
                if let Some(ref bp) = *base {
                    let init_path = bp.join(&name).join("manifests").join("init.rhai");
                    if init_path.exists() {
                        {
                            let mut resources =
                                exec_ctx.resources.lock().expect("Failed to lock resources");
                            resources.push(Resource::Meta(MetaResource {
                                id: handle.start_id.clone(),
                                kind: MetaKind::ModuleStart,
                                dependencies: Vec::new(),
                            }));
                        }

                        exec_ctx
                            .module_stack
                            .lock()
                            .expect("Failed to lock module stack")
                            .push(name.clone());

                        let _ = ctx.engine().eval_file::<Dynamic>(init_path).map_err(|e| {
                            Box::new(rhai::EvalAltResult::ErrorRuntime(
                                format!("Failed to include module '{}': {}", name, e).into(),
                                rhai::Position::NONE,
                            ))
                        })?;

                        exec_ctx
                            .module_stack
                            .lock()
                            .expect("Failed to lock module stack")
                            .pop();

                        {
                            let mut resources =
                                exec_ctx.resources.lock().expect("Failed to lock resources");
                            resources.push(Resource::Meta(MetaResource {
                                id: handle.end_id.clone(),
                                kind: MetaKind::ModuleEnd,
                                dependencies: vec![handle.start_id.clone()],
                            }));
                        }

                        return Ok(handle);
                    }
                }
                Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Module {} not found", name).into(),
                    rhai::Position::NONE,
                )))
            },
        );
    }

    // --- Helpers ---

    fn get_exec_ctx() -> ExecutionContext {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow()
                .clone()
                .expect("Execution context must be set during Rhai evaluation")
        })
    }

    fn add_resource(exec_ctx: &ExecutionContext, resource: Resource) -> Resource {
        exec_ctx
            .resources
            .lock()
            .expect("Failed to lock resources")
            .push(resource.clone());
        resource
    }

    fn add_dependency_between_ids(lhs_id: &str, rhs_id: &str) {
        let exec_ctx = Self::get_exec_ctx();
        let mut resources = exec_ctx.resources.lock().expect("Failed to lock resources");
        if let Some(res) = resources.iter_mut().find(|r| r.id() == rhs_id) {
            res.add_dependency(lhs_id.to_string());
        }
    }

    fn extract_ensure(params: &Map) -> Ensure {
        params
            .get("ensure")
            .and_then(|v| {
                v.clone()
                    .try_cast::<String>()
                    .map(|s| match s.as_str() {
                        "absent" => Ensure::Absent,
                        _ => Ensure::Present,
                    })
                    .or_else(|| v.clone().try_cast::<Ensure>())
            })
            .unwrap_or(Ensure::Present)
    }

    fn extract_dependencies(params: &Map, exec_ctx: &ExecutionContext) -> Vec<String> {
        let mut dependencies = Vec::new();

        // Automatic dependency on current module start
        let stack = exec_ctx
            .module_stack
            .lock()
            .expect("Failed to lock module stack");
        if let Some(curr_mod) = stack.last() {
            dependencies.push(format!("ModuleStart[{}]", curr_mod));
        }
        drop(stack);

        if let Some(req) = params.get("require") {
            Self::push_dependency(&mut dependencies, req.clone());
        }

        dependencies
    }

    fn push_dependency(dependencies: &mut Vec<String>, req: Dynamic) {
        if let Some(dep_res) = req.clone().try_cast::<Resource>() {
            dependencies.push(dep_res.id().to_string());
        } else if let Some(dep_id) = req.clone().try_cast::<String>() {
            dependencies.push(dep_id);
        } else if let Some(m_h) = req.clone().try_cast::<ModuleHandle>() {
            dependencies.push(m_h.end_id);
        } else if let Some(arr) = req.try_cast::<rhai::Array>() {
            for item in arr {
                Self::push_dependency(dependencies, item);
            }
        }
    }

    fn extract_string(params: &Map, key: &str) -> Option<String> {
        params.get(key).and_then(|v| v.clone().try_cast::<String>())
    }

    fn extract_bool(params: &Map, key: &str, default: bool) -> bool {
        params
            .get(key)
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(default)
    }
}

impl PupoxideEngine {
    pub fn new(hiera: Option<Hiera>) -> Self {
        let mut builder = PupoxideEngineBuilder::new();
        if let Some(h) = hiera {
            builder = builder.with_hiera(h);
        }
        builder.register_defaults().build()
    }

    pub fn builder() -> PupoxideEngineBuilder {
        PupoxideEngineBuilder::new()
    }

    pub fn set_module_path(&self, path: PathBuf) {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
    }

    pub fn run_manifest(
        &self,
        path: PathBuf,
        node_name: String,
        environment: String,
        facts: Facts,
    ) -> Result<Catalog> {
        let exec_ctx = ExecutionContext::new(facts.clone());
        let mut scope = Scope::new();

        // Inject facts into scope
        let mut facts_map = Map::new();
        for (k, v) in facts.values {
            facts_map.insert(k.into(), v.into());
        }
        scope.set_value("facts", facts_map);

        let ast = self
            .engine
            .compile_file(path)
            .map_err(|e| DomainError::Internal(format!("Rhai compilation error: {}", e)))?;

        let eval_res = CURRENT_EXEC_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(exec_ctx.clone());
            let r = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);
            *ctx.borrow_mut() = None;
            r
        });

        let _ =
            eval_res.map_err(|e| DomainError::Internal(format!("Rhai execution error: {}", e)))?;

        let resources = exec_ctx
            .resources
            .lock()
            .expect("Failed to lock resources")
            .clone();
        let sorted_resources = self.sort_resources(resources)?;

        Ok(Catalog::new(node_name, environment, sorted_resources))
    }

    pub fn run_manifest_with_modules(
        &self,
        path: PathBuf,
        module_path: PathBuf,
        node_name: String,
        environment: String,
        facts: Facts,
    ) -> Result<Catalog> {
        self.set_module_path(module_path);
        self.run_manifest(path, node_name, environment, facts)
    }

    /// Performs topological sort of resources based on dependencies
    fn sort_resources(&self, resources: Vec<Resource>) -> Result<Vec<Resource>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        let resource_map: HashMap<String, Resource> = resources
            .into_iter()
            .map(|r| (r.id().to_string(), r))
            .collect();

        fn visit(
            id: &str,
            resource_map: &HashMap<String, Resource>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
            sorted: &mut Vec<Resource>,
        ) -> Result<()> {
            if visiting.contains(id) {
                return Err(DomainError::Internal(format!(
                    "Circular dependency detected involving: {}",
                    id
                )));
            }
            if !visited.contains(id) {
                visiting.insert(id.to_string());
                if let Some(res) = resource_map.get(id) {
                    for dep in res.dependencies() {
                        visit(dep, resource_map, visited, visiting, sorted)?;
                    }
                    sorted.push(res.clone());
                }
                visiting.remove(id);
                visited.insert(id.to_string());
            }
            Ok(())
        }

        for id in resource_map.keys() {
            visit(id, &resource_map, &mut visited, &mut visiting, &mut sorted)?;
        }

        Ok(sorted)
    }
}
