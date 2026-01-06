use rhai::{Engine, Scope, Dynamic, Map, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use std::cell::RefCell;
use crate::domain::resource::{Resource, FileResource, Ensure, MetaResource, MetaKind};
use crate::domain::catalog::Catalog;
use crate::domain::error::{Result, DomainError};
use crate::domain::facts::Facts;

thread_local! {
    static CURRENT_EXEC_CTX: RefCell<Option<ExecutionContext>> = RefCell::new(None);
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub resources: Arc<Mutex<Vec<Resource>>>,
    pub included_modules: Arc<Mutex<HashSet<String>>>,
    pub module_stack: Arc<Mutex<Vec<String>>>,
}

impl ExecutionContext {
    fn new() -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            included_modules: Arc::new(Mutex::new(HashSet::new())),
            module_stack: Arc::new(Mutex::new(Vec::new())),
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
}

impl PupoxideEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let module_path: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));

        // Register Resource and ModuleHandle types
        engine.register_type_with_name::<Resource>("Resource");
        engine.register_type_with_name::<ModuleHandle>("ModuleHandle");

        // Register Ensure enum
        engine.register_type_with_name::<Ensure>("Ensure")
              .register_fn("present", || Ensure::Present)
              .register_fn("absent", || Ensure::Absent);

        // Map strings to Ensure
        engine.register_fn("to_ensure", |s: String| match s.as_str() {
            "absent" => Ensure::Absent,
            _ => Ensure::Present,
        });

        // The main 'file' function
        engine.register_fn("file", move |path: String, params: Map| {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            
            let ensure = params.get("ensure")
                .and_then(|v| {
                    if let Some(s) = v.clone().try_cast::<String>() {
                        Some(match s.as_str() {
                            "absent" => Ensure::Absent,
                            _ => Ensure::Present,
                        })
                    } else {
                        v.clone().try_cast::<Ensure>()
                    }
                })
                .unwrap_or(Ensure::Present);

            let content = params.get("content").and_then(|v| v.clone().try_cast::<String>());
            
            let mut dependencies = Vec::new();

            // Automatic dependency on current module start
            let stack = exec_ctx.module_stack.lock().unwrap();
            if let Some(curr_mod) = stack.last() {
                dependencies.push(format!("ModuleStart[{}]", curr_mod));
            }
            drop(stack);

            if let Some(req) = params.get("require") {
                if let Some(dep_res) = req.clone().try_cast::<Resource>() {
                    dependencies.push(dep_res.id().to_string());
                } else if let Some(dep_id) = req.clone().try_cast::<String>() {
                    dependencies.push(dep_id);
                } else if let Some(m_h) = req.clone().try_cast::<ModuleHandle>() {
                    dependencies.push(m_h.end_id);
                } else if let Some(arr) = req.clone().try_cast::<rhai::Array>() {
                    for item in arr {
                        if let Some(r) = item.clone().try_cast::<Resource>() {
                            dependencies.push(r.id().to_string());
                        } else if let Some(s) = item.clone().try_cast::<String>() {
                            dependencies.push(s);
                        } else if let Some(m_h) = item.try_cast::<ModuleHandle>() {
                            dependencies.push(m_h.end_id);
                        }
                    }
                }
            }

            let resource = Resource::File(FileResource {
                id: format!("File[{}]", path),
                path: PathBuf::from(path),
                ensure,
                content,
                dependencies,
            });

            exec_ctx.resources.lock().unwrap().push(resource.clone());
            resource
        });

        engine.register_custom_operator("->", 60).unwrap();
        
        // Resource -> Resource
        engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            let mut resources = exec_ctx.resources.lock().unwrap();
            if let Some(res) = resources.iter_mut().find(|r: &&mut Resource| r.id() == rhs.id()) {
                res.add_dependency(lhs.id().to_string());
            }
            rhs
        });

        // ModuleHandle -> ModuleHandle
        engine.register_fn("->", move |lhs: ModuleHandle, rhs: ModuleHandle| {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            let mut resources = exec_ctx.resources.lock().unwrap();
            if let Some(res) = resources.iter_mut().find(|r: &&mut Resource| r.id() == rhs.start_id) {
                res.add_dependency(lhs.end_id.clone());
            }
            rhs
        });

        // Resource -> ModuleHandle
        engine.register_fn("->", move |lhs: Resource, rhs: ModuleHandle| {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            let mut resources = exec_ctx.resources.lock().unwrap();
            if let Some(res) = resources.iter_mut().find(|r: &&mut Resource| r.id() == rhs.start_id) {
                res.add_dependency(lhs.id().to_string());
            }
            rhs
        });

        // ModuleHandle -> Resource
        engine.register_fn("->", move |lhs: ModuleHandle, rhs: Resource| {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            let mut resources = exec_ctx.resources.lock().unwrap();
            if let Some(res) = resources.iter_mut().find(|r: &&mut Resource| r.id() == rhs.id()) {
                res.add_dependency(lhs.end_id.clone());
            }
            rhs
        });

        // Register 'include' function
        let m_path = module_path.clone();
        engine.register_fn("include", move |ctx: NativeCallContext, name: String| -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = CURRENT_EXEC_CTX.with(|ctx| ctx.borrow().clone().expect("No execution context"));
            
            let handle = ModuleHandle {
                name: name.clone(),
                start_id: format!("ModuleStart[{}]", name),
                end_id: format!("ModuleEnd[{}]", name),
            };

            // Idempotency: skip if already included
            let mut included = exec_ctx.included_modules.lock().unwrap();
            if included.contains(&name) {
                return Ok(handle);
            }
            included.insert(name.clone());
            drop(included);

            let base = m_path.lock().unwrap();
            if let Some(ref bp) = *base {
                let init_path = bp.join(&name).join("manifests").join("init.rhai");
                if init_path.exists() {
                    // Emit ModuleStart
                    {
                        let mut resources = exec_ctx.resources.lock().unwrap();
                        resources.push(Resource::Meta(MetaResource {
                            id: handle.start_id.clone(),
                            kind: MetaKind::ModuleStart,
                            dependencies: Vec::new(),
                        }));
                    }

                    // Push to stack
                    exec_ctx.module_stack.lock().unwrap().push(name.clone());

                    // Evaluate the included file
                    // We need to maintain the thread-local during this call as well, 
                    // and eval_file will run on the same thread.
                    let _ = ctx.engine().eval_file::<Dynamic>(init_path).map_err(|e| {
                        Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to include module '{}': {}", name, e).into(),
                            rhai::Position::NONE,
                        ))
                    })?;

                    // Pop stack
                    exec_ctx.module_stack.lock().unwrap().pop();

                    // Emit ModuleEnd
                    {
                        let mut resources = exec_ctx.resources.lock().unwrap();
                        resources.push(Resource::Meta(MetaResource {
                            id: handle.end_id.clone(),
                            kind: MetaKind::ModuleEnd,
                            dependencies: vec![handle.start_id.clone()], // End depends on Start
                        }));
                    }

                    return Ok(handle);
                }
            }
            Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Module {} not found", name).into(),
                rhai::Position::NONE,
            )))
        });

        Self { engine: Arc::new(engine), module_path }
    }

    pub fn set_module_path(&self, path: PathBuf) {
        *self.module_path.lock().unwrap() = Some(path);
    }

    pub fn run_manifest(&self, path: PathBuf, node_name: String, environment: String, facts: Facts) -> Result<Catalog> {
        let exec_ctx = ExecutionContext::new();
        let mut scope = Scope::new();
        
        // Inject facts into scope
        let mut facts_map = Map::new();
        for (k, v) in facts.values {
            facts_map.insert(k.into(), v.into());
        }
        scope.set_value("facts", facts_map);

        let ast = self.engine.compile_file(path)
            .map_err(|e| DomainError::Internal(format!("Rhai compilation error: {}", e)))?;
        
        let eval_res = CURRENT_EXEC_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(exec_ctx.clone());
            let r = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);
            *ctx.borrow_mut() = None;
            r
        });

        let _ = eval_res.map_err(|e| DomainError::Internal(format!("Rhai execution error: {}", e)))?;

        let resources = exec_ctx.resources.lock().unwrap().clone();
        let sorted_resources = self.sort_resources(resources)?;

        Ok(Catalog::new(node_name, environment, sorted_resources))
    }

    pub fn run_manifest_with_modules(&self, path: PathBuf, module_path: PathBuf, node_name: String, environment: String, facts: Facts) -> Result<Catalog> {
        self.set_module_path(module_path);
        self.run_manifest(path, node_name, environment, facts)
    }

    /// Performs topological sort of resources based on dependencies
    fn sort_resources(&self, resources: Vec<Resource>) -> Result<Vec<Resource>> {
        let mut sorted = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        let resource_map: HashMap<String, Resource> = resources.into_iter()
            .map(|r| (r.id().to_string(), r))
            .collect();

        fn visit(
            id: &str, 
            resource_map: &HashMap<String, Resource>,
            visited: &mut HashSet<String>,
            visiting: &mut HashSet<String>,
            sorted: &mut Vec<Resource>
        ) -> Result<()> {
            if visiting.contains(id) {
                return Err(DomainError::Internal(format!("Circular dependency detected involving: {}", id)));
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
