use crate::domain::catalog::Catalog;
use crate::domain::error::{DomainError, Result};
use crate::domain::facts::Facts;
use crate::domain::resource::Resource;
use crate::infrastructure::hiera::Hiera;
use rhai::{Dynamic, Engine, Map, Scope};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::dsl;

thread_local! {
    pub static CURRENT_EXEC_CTX: RefCell<Option<ExecutionContext>> = RefCell::new(None);
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub resources: Arc<Mutex<Vec<Resource>>>,
    pub included_modules: Arc<Mutex<HashSet<String>>>,
    pub module_stack: Arc<Mutex<Vec<String>>>,
    pub facts: Arc<Facts>,
}

impl ExecutionContext {
    pub fn new(facts: Facts) -> Self {
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

    pub fn with_module_path(self, path: PathBuf) -> Self {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
        self
    }

    pub fn register_defaults(mut self) -> Self {
        let hiera_arc = Arc::new(self.hiera.clone());
        dsl::register_all(&mut self.engine, hiera_arc, self.module_path.clone());
        self
    }

    pub fn build(self) -> PupoxideEngine {
        PupoxideEngine {
            engine: Arc::new(self.engine),
            module_path: self.module_path,
            hiera: Arc::new(self.hiera),
        }
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
