use rhai::{Engine, Scope, Dynamic, Map, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::collections::{HashMap, HashSet};
use crate::domain::resource::{Resource, FileResource, Ensure};
use crate::domain::catalog::Catalog;
use crate::domain::error::{Result, DomainError};
use crate::domain::facts::Facts;

#[derive(Clone)]
struct SharedCollector {
    resources: Arc<Mutex<Vec<Resource>>>,
}

#[derive(Clone)]
pub struct PupoxideEngine {
    engine: Arc<Engine>,
    collector: SharedCollector,
    module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl PupoxideEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let collector = SharedCollector {
            resources: Arc::new(Mutex::new(Vec::new())),
        };
        let module_path: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));

        // Register Resource type
        engine.register_type_with_name::<Resource>("Resource");

        // Register Ensure enum
        engine.register_type_with_name::<Ensure>("Ensure")
              .register_fn("present", || Ensure::Present)
              .register_fn("absent", || Ensure::Absent);

        // Map strings to Ensure
        engine.register_fn("to_ensure", |s: String| match s.as_str() {
            "absent" => Ensure::Absent,
            _ => Ensure::Present,
        });

        let c = collector.clone();
        // The main 'file' function using Map
        engine.register_fn("file", move |path: String, params: Map| {
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
            if let Some(req) = params.get("require") {
                if let Some(dep_res) = req.clone().try_cast::<Resource>() {
                    dependencies.push(dep_res.id().to_string());
                } else if let Some(dep_id) = req.clone().try_cast::<String>() {
                    dependencies.push(dep_id);
                } else if let Some(arr) = req.clone().try_cast::<rhai::Array>() {
                    for item in arr {
                        if let Some(r) = item.clone().try_cast::<Resource>() {
                            dependencies.push(r.id().to_string());
                        } else if let Some(s) = item.try_cast::<String>() {
                            dependencies.push(s);
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

            c.resources.lock().unwrap().push(resource.clone());
            resource
        });

        let c2 = collector.clone();
        engine.register_custom_operator("->", 60).unwrap();
        engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
            let mut resources = c2.resources.lock().unwrap();
            if let Some(res) = resources.iter_mut().find(|r| r.id() == rhs.id()) {
                res.add_dependency(lhs.id().to_string());
            }
            rhs
        });

        // Register 'include' function
        let m_path = module_path.clone();
        engine.register_fn("include", move |ctx: NativeCallContext, name: String| -> std::result::Result<Dynamic, Box<rhai::EvalAltResult>> {
            let base = m_path.lock().unwrap();
            if let Some(ref bp) = *base {
                let init_path = bp.join(&name).join("manifests").join("init.rhai");
                if init_path.exists() {
                    // Evaluate the included file in the same engine context
                    let _ = ctx.engine().eval_file::<Dynamic>(init_path).map_err(|e| {
                        Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to include module '{}': {}", name, e).into(),
                            rhai::Position::NONE,
                        ))
                    })?;
                    return Ok(Dynamic::TRUE);
                }
            }
            Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Module {} not found", name).into(),
                rhai::Position::NONE,
            )))
        });

        Self { engine: Arc::new(engine), collector, module_path }
    }

    pub fn set_module_path(&self, path: PathBuf) {
        *self.module_path.lock().unwrap() = Some(path);
    }

    pub fn run_manifest(&self, path: PathBuf, node_name: String, environment: String, facts: Facts) -> Result<Catalog> {
        self.collector.resources.lock().unwrap().clear();

        let mut scope = Scope::new();
        
        // Inject facts into scope
        let mut facts_map = Map::new();
        for (k, v) in facts.values {
            facts_map.insert(k.into(), v.into());
        }
        scope.set_value("facts", facts_map);

        let ast = self.engine.compile_file(path)
            .map_err(|e| DomainError::Internal(format!("Rhai compilation error: {}", e)))?;
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|e| DomainError::Internal(format!("Rhai execution error: {}", e)))?;

        let resources = self.collector.resources.lock().unwrap().clone();
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
