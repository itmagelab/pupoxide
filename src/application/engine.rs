use crate::domain::catalog::Catalog;
use crate::domain::error::Result;
use crate::domain::facts::Facts;
use crate::domain::resource::Resource;
use crate::infrastructure::stash::Stash;
use rhai::{Dynamic, Engine, Map, Scope};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::dsl;

thread_local! {
    pub static CURRENT_EXEC_CTX: RefCell<Option<ExecutionContext>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InclusionType {
    Module,
    Role,
    Profile,
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub resources: Arc<Mutex<Vec<Resource>>>,
    pub included_modules: Arc<Mutex<HashSet<String>>>,
    pub module_stack: Arc<Mutex<Vec<String>>>,
    pub inclusion_stack: Arc<Mutex<Vec<InclusionType>>>,
    pub facts: Arc<Facts>,
    pub current_path: Arc<Mutex<PathBuf>>,
}

impl ExecutionContext {
    pub fn new(facts: Facts, path: PathBuf) -> Self {
        Self {
            resources: Arc::new(Mutex::new(Vec::new())),
            included_modules: Arc::new(Mutex::new(HashSet::new())),
            module_stack: Arc::new(Mutex::new(Vec::new())),
            inclusion_stack: Arc::new(Mutex::new(Vec::new())),
            facts: Arc::new(facts),
            current_path: Arc::new(Mutex::new(path)),
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
    _stash: Arc<Option<Stash>>,
}

pub struct PupoxideEngineBuilder {
    engine: Engine,
    stash: Option<Stash>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl Default for PupoxideEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PupoxideEngineBuilder {
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            stash: None,
            module_path: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_stash(mut self, stash: Stash) -> Self {
        self.stash = Some(stash);
        self
    }

    pub fn with_module_path(self, path: PathBuf) -> Self {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
        self
    }

    pub fn register_defaults(mut self) -> Self {
        let stash_arc = Arc::new(self.stash.clone());
        dsl::register_all(&mut self.engine, stash_arc, self.module_path.clone());
        self
    }

    pub fn build(self) -> PupoxideEngine {
        let mut engine = self.engine;
        engine.set_module_resolver(PupoxideModuleResolver {
            module_path: self.module_path.clone(),
        });
        PupoxideEngine {
            engine: Arc::new(engine),
            module_path: self.module_path,
            _stash: Arc::new(self.stash),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PupoxideModuleResolver {
    pub module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl PupoxideModuleResolver {
    /// Получает текущий контекст выполнения из локального хранилища потока.
    fn get_context(
        &self,
        pos: rhai::Position,
    ) -> std::result::Result<ExecutionContext, Box<rhai::EvalAltResult>> {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow().clone().ok_or_else(|| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    "Контекст выполнения не найден. Убедитесь, что движок запущен через run_manifest.".into(),
                    pos,
                ))
            })
        })
    }

    /// Определяет полный путь к файлу манифеста или модуля.
    fn resolve_full_path(
        &self,
        path: &str,
        exec_ctx: &ExecutionContext,
        pos: rhai::Position,
    ) -> std::result::Result<PathBuf, Box<rhai::EvalAltResult>> {
        let current_p = exec_ctx
            .current_path
            .lock()
            .expect("Не удалось заблокировать текущий путь");
        let parent_dir = current_p.parent().unwrap_or(&current_p);

        let full_path = if path.starts_with(".") {
            // Относительный путь (например, import "./utils")
            let mut p = parent_dir.join(path);
            if p.extension().is_none() {
                p.set_extension("rhai");
            }
            p
        } else {
            // Путь к модулю (например, import "nginx")
            let base = self
                .module_path
                .lock()
                .expect("Не удалось заблокировать путь к модулям");
            if let Some(ref bp) = *base {
                bp.join(path).join("manifests").join("init.rhai")
            } else {
                return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Путь к модулям не установлен, невозможно найти '{}'", path).into(),
                    pos,
                )));
            }
        };

        if !full_path.exists() {
            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Файл не найден: {}", full_path.display()).into(),
                pos,
            )));
        }

        Ok(full_path)
    }
}

impl rhai::ModuleResolver for PupoxideModuleResolver {
    fn resolve(
        &self,
        engine: &rhai::Engine,
        _source_path: Option<&str>,
        path: &str,
        pos: rhai::Position,
    ) -> std::result::Result<Arc<rhai::Module>, Box<rhai::EvalAltResult>> {
        // 1. Получаем контекст и определяем путь к файлу
        let exec_ctx = self.get_context(pos)?;
        let full_path = self.resolve_full_path(path, &exec_ctx, pos)?;

        let module_name = path.to_string();
        let handle = ModuleHandle {
            name: module_name.clone(),
            start_id: format!("ModuleStart[{}]", module_name),
            end_id: format!("ModuleEnd[{}]", module_name),
        };

        // 2. Создаем начальный мета-ресурс для отслеживания зависимостей модуля
        let start_resource_count = {
            let mut resources = exec_ctx
                .resources
                .lock()
                .expect("Не удалось заблокировать ресурсы");
            let count = resources.len();
            resources.push(Resource::Meta(crate::domain::resource::MetaResource {
                id: handle.start_id.clone(),
                kind: crate::domain::resource::MetaKind::ModuleStart,
                dependencies: Vec::new(),
            }));
            count
        };

        // 3. Подготавливаем среду для выполнения модуля
        exec_ctx
            .module_stack
            .lock()
            .expect("Не удалось заблокировать стек модулей")
            .push(module_name.clone());

        let old_path = {
            let mut current_p = exec_ctx
                .current_path
                .lock()
                .expect("Не удалось заблокировать текущий путь");
            std::mem::replace(&mut *current_p, full_path.clone())
        };

        // 4. Компилируем и выполняем AST
        let mut ast = engine.compile_file(full_path).map_err(|e| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Ошибка компиляции импорта '{}': {}", path, e).into(),
                pos,
            ))
        })?;
        ast.set_source(path);

        let mut scope = rhai::Scope::new();
        // Пробрасываем "facts" внутрь модуля
        let mut facts_map = Map::new();
        for (k, v) in exec_ctx.facts.values.clone() {
            facts_map.insert(k.into(), v.into());
        }
        scope.set_value("facts", facts_map);

        let eval_res = engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);

        // 5. Восстанавливаем состояние после выполнения
        {
            let mut current_p = exec_ctx
                .current_path
                .lock()
                .expect("Не удалось заблокировать текущий путь");
            *current_p = old_path;
        }
        exec_ctx
            .module_stack
            .lock()
            .expect("Не удалось заблокировать стек модулей")
            .pop();

        let _ = eval_res.map_err(|e| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Ошибка выполнения импорта '{}': {}", path, e).into(),
                pos,
            ))
        })?;

        // 6. Создаем финальный мета-ресурс, который зависит от всего, что было внутри модуля
        {
            let mut resources = exec_ctx
                .resources
                .lock()
                .expect("Не удалось заблокировать ресурсы");
            let mut end_deps = vec![handle.start_id.clone()];

            // Модуль считается завершенным только после выполнения всех его ресурсов
            for i in start_resource_count..resources.len() {
                end_deps.push(resources[i].id().to_string());
            }

            resources.push(Resource::Meta(crate::domain::resource::MetaResource {
                id: handle.end_id.clone(),
                kind: crate::domain::resource::MetaKind::ModuleEnd,
                dependencies: end_deps,
            }));
        }

        // 7. Формируем Rhai-модуль для возврата
        let mut module = rhai::Module::eval_ast_as_new(scope, &ast, engine)?;
        module.set_var("module_handle", handle);

        Ok(Arc::new(module))
    }
}

impl PupoxideEngine {
    pub fn new(stash: Option<Stash>) -> Self {
        let mut builder = PupoxideEngineBuilder::new();
        if let Some(s) = stash {
            builder = builder.with_stash(s);
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
        let exec_ctx = ExecutionContext::new(facts.clone(), path.clone());
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
            .map_err(|e| anyhow::anyhow!("Rhai compilation error: {}", e))?;

        let eval_res = CURRENT_EXEC_CTX.with(|ctx| {
            *ctx.borrow_mut() = Some(exec_ctx.clone());
            let r = self.engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);
            *ctx.borrow_mut() = None;
            r
        });

        let _ =
            eval_res.map_err(|e| anyhow::anyhow!("Rhai execution error: {}", e))?;

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
                return Err(anyhow::anyhow!(
                    "Circular dependency detected involving: {}",
                    id
                ));
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
