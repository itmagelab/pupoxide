use rhai::{Engine, Scope, Dynamic, Map};
use std::path::PathBuf;
use std::cell::RefCell;
use std::rc::Rc;
use std::collections::{HashMap, HashSet};
use crate::domain::resource::{Resource, FileResource, Ensure};
use crate::domain::error::{Result, DomainError};

#[derive(Clone)]
struct SharedCollector {
    resources: Rc<RefCell<Vec<Resource>>>,
}

pub struct PupoxideEngine {
    engine: Engine,
    collector: SharedCollector,
}

impl PupoxideEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let collector = SharedCollector {
            resources: Rc::new(RefCell::new(Vec::new())),
        };

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

            c.resources.borrow_mut().push(resource.clone());
            resource
        });

        let c2 = collector.clone();
        // Register the -> operator
        // We register it as a custom operator name "->". 
        // Rhai allows this via register_custom_operator + register_fn.
        engine.register_custom_operator("->", 60).unwrap();
        engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
            let mut resources = c2.resources.borrow_mut();
            // Find rhs in collected resources and add lhs as dependency
            if let Some(res) = resources.iter_mut().find(|r| r.id() == rhs.id()) {
                res.add_dependency(lhs.id().to_string());
            }
            rhs
        });

        Self { engine, collector }
    }

    pub fn run_manifest(&self, path: PathBuf) -> Result<Vec<Resource>> {
        // Clear previous run
        self.collector.resources.borrow_mut().clear();

        let mut scope = Scope::new();
        let ast = self.engine.compile_file(path)
            .map_err(|e| DomainError::Internal(format!("Rhai compilation error: {}", e)))?;
        
        let _ = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast)
            .map_err(|e| DomainError::Internal(format!("Rhai execution error: {}", e)))?;

        let resources = self.collector.resources.borrow().clone();
        self.sort_resources(resources)
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
