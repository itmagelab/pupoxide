use crate::domain::catalog::Catalog;
use crate::domain::error::Result;
use crate::domain::facts::Facts;
use crate::domain::resource::Resource;

/// Trait (Port) abstracting value lookup from a configuration stash (hierarchical data store).
pub trait StashProvider: Send + Sync {
    /// Looks up a value by its key path, optionally matching target host facts.
    fn lookup(&self, key: &str, facts: &crate::domain::facts::Facts) -> Option<yaml_serde::Value>;
}
use rhai::{Dynamic, Engine, Map, Scope};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::dsl;

thread_local! {
    /// Thread-local storage holding the active execution context during Rhai evaluation.
    pub static CURRENT_EXEC_CTX: RefCell<Option<ExecutionContext>> = const { RefCell::new(None) };
}

/// Category of hierarchical inclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InclusionType {
    /// A generic manifest module (e.g. `import "nginx"`).
    Module,
    /// A top-level node definition role.
    Role,
    /// A configuration package profile.
    Profile,
}

/// Represents an active manifest inclusion during evaluation.
#[derive(Debug, Clone)]
pub struct ActiveInclusion {
    /// Category of the inclusion.
    pub inclusion_type: InclusionType,
    /// Logical name of the included resource/file.
    pub name: String,
}

/// The mutable state of manifest evaluation, shared within the execution context.
#[derive(Debug, Clone)]
pub struct ExecutionState {
    /// The catalog under construction.
    pub catalog: Catalog,
    /// Tracks modules that have already been included to prevent redundant evaluation.
    pub included_modules: HashSet<String>,
    /// Active hierarchical stack of inclusion paths.
    pub module_stack: Vec<ActiveInclusion>,
    /// Absolute path of the currently evaluating manifest.
    pub current_path: PathBuf,
    /// Maps Rhai source locations to topological ModuleStart nodes.
    pub source_map: HashMap<String, String>,
}

impl ExecutionState {
    /// Returns the currently active inclusion type, if any.
    pub fn current_inclusion_type(&self) -> Option<InclusionType> {
        self.module_stack.last().map(|inc| inc.inclusion_type)
    }

    /// Begins a new manifest inclusion (Module, Role, or Profile).
    ///
    /// This method:
    /// 1. Computes parent dependency links based on the active stack.
    /// 2. Creates and registers the start marker resource in the catalog.
    /// 3. Pushes the new inclusion onto the stack.
    /// 4. Switches the current path to the target manifest file.
    ///
    /// Returns the created `ModuleHandle`, the old path (to be restored later),
    /// and the catalog node count before this inclusion started.
    pub fn enter_inclusion(
        &mut self,
        inc_type: InclusionType,
        name: String,
        target_path: PathBuf,
    ) -> (ModuleHandle, PathBuf, usize) {
        let handle = ModuleHandle {
            name: name.clone(),
            start_id: format!("{:?}Start[{}]", inc_type, name),
            end_id: format!("{:?}End[{}]", inc_type, name),
        };

        // 1. Determine dependencies (the parent start marker, if any)
        let mut dependencies = Vec::new();
        if let Some(parent) = self.module_stack.last() {
            dependencies.push(format!("{:?}Start[{}]", parent.inclusion_type, parent.name));
        }

        // 2. Add start marker to catalog
        let resource = Resource::Meta(crate::domain::resource::MetaResource {
            id: handle.start_id.clone(),
            kind: crate::domain::resource::MetaKind::ModuleStart,
            dependencies: dependencies.clone(),
        });
        self.catalog.add_resource(resource);

        for dep in dependencies {
            let _ = self.catalog.add_dependency(&dep, &handle.start_id);
        }

        // 3. Register in source map (especially useful for error/context mapping in standard imports)
        self.source_map
            .insert(name.clone(), handle.start_id.clone());

        // 4. Record current node count and push to stack
        let start_node_count = self.catalog.graph.node_count();
        self.module_stack.push(ActiveInclusion {
            inclusion_type: inc_type,
            name,
        });

        // 5. Update path
        let old_path = std::mem::replace(&mut self.current_path, target_path);

        (handle, old_path, start_node_count)
    }

    /// Finishes the current manifest inclusion.
    ///
    /// This method:
    /// 1. Restores the previous file path.
    /// 2. Pops the inclusion from the stack.
    /// 3. Computes dependencies for the end marker (all resources declared during evaluation).
    /// 4. Creates and registers the end marker resource in the catalog.
    pub fn exit_inclusion(
        &mut self,
        handle: &ModuleHandle,
        old_path: PathBuf,
        start_node_count: usize,
    ) {
        // 1. Restore path and pop stack
        self.current_path = old_path;
        self.module_stack.pop();

        // 2. Gather all resources added during this inclusion's evaluation
        let current_count = self.catalog.graph.node_count();
        let mut end_deps = Vec::new();
        for i in start_node_count..current_count {
            let idx = petgraph::graph::NodeIndex::new(i);
            if let Some(res) = self.catalog.graph.node_weight(idx)
                && res.id() != handle.start_id
            {
                end_deps.push(res.id().to_string());
            }
        }

        // Fallback to start marker if no resources were added
        if end_deps.is_empty() {
            end_deps.push(handle.start_id.clone());
        }

        // 3. Add end marker to catalog
        let resource = Resource::Meta(crate::domain::resource::MetaResource {
            id: handle.end_id.clone(),
            kind: crate::domain::resource::MetaKind::ModuleEnd,
            dependencies: end_deps.clone(),
        });
        self.catalog.add_resource(resource);

        for dep in end_deps {
            let _ = self.catalog.add_dependency(&dep, &handle.end_id);
        }
    }
}

/// The runtime context holding current catalog state, module call stacks, and collected facts.
///
/// This context is dynamically accessed by DSL helper functions during Rhai execution.
#[derive(Clone, Debug)]
pub struct ExecutionContext {
    /// The shared mutable state of evaluation.
    pub state: Arc<Mutex<ExecutionState>>,
    /// Collected system facts.
    pub facts: Arc<Facts>,
}

impl ExecutionContext {
    /// Creates a new `ExecutionContext` for the given node and environment.
    pub fn new(facts: Facts, path: PathBuf, node_name: String, environment: String) -> Self {
        Self {
            state: Arc::new(Mutex::new(ExecutionState {
                catalog: Catalog::new(node_name, environment),
                included_modules: HashSet::new(),
                module_stack: Vec::new(),
                current_path: path,
                source_map: HashMap::new(),
            })),
            facts: Arc::new(facts),
        }
    }

    /// Safely retrieves the active execution context from thread-local storage.
    ///
    /// # Panics
    ///
    /// Panics if called outside the context of Rhai manifest evaluation.
    pub fn get_current() -> Self {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow()
                .clone()
                // SAFETY: Execution context must be set during Rhai evaluation.
                // This is ensured by the engine's run_manifest method.
                .expect("Execution context must be set during Rhai evaluation")
        })
    }

    /// Computes and returns the logical hierarchical location context (role, profile, module path).
    pub fn get_source_context(&self) -> Option<crate::domain::resource::SourceContext> {
        let state = self.state.lock().ok()?;
        let mut context = crate::domain::resource::SourceContext::default();
        let mut modules = Vec::new();

        for inc in state.module_stack.iter() {
            let mut normalized_name = inc.name.strip_prefix("./").unwrap_or(&inc.name);
            normalized_name = normalized_name
                .strip_suffix(".rhai")
                .unwrap_or(normalized_name);

            match inc.inclusion_type {
                InclusionType::Role => context.role = Some(normalized_name.to_string()),
                InclusionType::Profile => context.profile = Some(normalized_name.to_string()),
                InclusionType::Module => modules.push(normalized_name.to_string()),
            }
        }

        if !modules.is_empty() {
            context.module = Some(modules.join("::"));
        }

        if context.role.is_none() && context.profile.is_none() && context.module.is_none() {
            None
        } else {
            Some(context)
        }
    }

    /// Adds a compiled resource to the local catalog.
    ///
    /// Ensures architectural purity rules (e.g. preventing direct resources inside Roles).
    pub fn add_resource(&self, resource: Resource) -> Result<Resource> {
        let mut state = self
            .state
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock state: {}", e))?;

        // Enforce role constraints: Resources cannot be added directly in Roles
        if state.current_inclusion_type() == Some(InclusionType::Role)
            && !matches!(resource, Resource::Meta(_))
        {
            return Err(anyhow::anyhow!(
                "Technical resources like '{}' are NOT allowed directly in Roles. Roles must ONLY include Profiles.",
                resource.id()
            ));
        }

        state.catalog.add_resource(resource.clone());
        Ok(resource)
    }

    /// Registers a dependency between two resources.
    pub fn add_dependency_between_ids(&self, lhs_id: &str, rhs_id: &str) {
        if let Ok(mut state) = self.state.lock() {
            let _ = state.catalog.add_dependency(lhs_id, rhs_id);
        }
    }

    /// Selects the default package provider based on system OS facts.
    pub fn get_default_provider(&self) -> String {
        let os_family = self
            .facts
            .get("os_family")
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        match os_family {
            "Darwin" | "macOS" => "brew".to_string(),
            "Ubuntu" | "Debian" => "apt".to_string(),
            "Linux" => "apt".to_string(),
            "RedHat" | "CentOS" | "Fedora" | "AlmaLinux" | "Rocky" => "yum".to_string(),
            _ => "brew".to_string(),
        }
    }
}

/// Tracks bounds and handles for evaluating standard modules.
#[derive(Clone, Debug)]
pub struct ModuleHandle {
    /// The name of the module.
    pub name: String,
    /// Identifier of the topological starting node.
    pub start_id: String,
    /// Identifier of the topological ending node.
    pub end_id: String,
}

/// The core Pupoxide engine for running and executing infrastructure-as-code manifests.
#[derive(Clone)]
pub struct PupoxideEngine {
    engine: Arc<Engine>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
    _stash: Option<Arc<dyn StashProvider>>,
}

/// A builder helper to initialize and configure a custom `PupoxideEngine`.
pub struct PupoxideEngineBuilder {
    engine: Engine,
    stash: Option<Arc<dyn StashProvider>>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl Default for PupoxideEngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PupoxideEngineBuilder {
    /// Creates a new `PupoxideEngineBuilder` with empty default configuration.
    pub fn new() -> Self {
        Self {
            engine: Engine::new(),
            stash: None,
            module_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Associates a hierarchical configuration stash with the engine.
    pub fn with_stash(mut self, stash: Arc<dyn StashProvider>) -> Self {
        self.stash = Some(stash);
        self
    }

    /// Configures the base directory used to resolve imported manifests and modules.
    pub fn with_module_path(self, path: PathBuf) -> Self {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
        self
    }

    /// Registers standard Pupoxide DSL functions, facts, and helpers.
    pub fn register_defaults(mut self) -> Self {
        let stash = self.stash.clone();
        dsl::register_all(&mut self.engine, stash, self.module_path.clone());
        self
    }

    /// Builds and returns the configured `PupoxideEngine`.
    pub fn build(self) -> PupoxideEngine {
        let mut engine = self.engine;
        engine.set_module_resolver(PupoxideModuleResolver {
            module_path: self.module_path.clone(),
        });
        PupoxideEngine {
            engine: Arc::new(engine),
            module_path: self.module_path,
            _stash: self.stash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PupoxideModuleResolver {
    pub module_path: Arc<Mutex<Option<PathBuf>>>,
}

impl PupoxideModuleResolver {
    /// Gets the current execution context from thread-local storage.
    fn get_context(
        &self,
        pos: rhai::Position,
    ) -> std::result::Result<ExecutionContext, Box<rhai::EvalAltResult>> {
        CURRENT_EXEC_CTX.with(|ctx| {
            ctx.borrow().clone().ok_or_else(|| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    "Execution context not found. Ensure the engine is running via run_manifest."
                        .into(),
                    pos,
                ))
            })
        })
    }

    /// Resolves the full path to a manifest or module file.
    fn resolve_full_path(
        &self,
        path: &str,
        exec_ctx: &ExecutionContext,
        pos: rhai::Position,
    ) -> std::result::Result<PathBuf, Box<rhai::EvalAltResult>> {
        let state = exec_ctx.state.lock().expect("Failed to lock state");
        let current_p = &state.current_path;
        let parent_dir = current_p.parent().unwrap_or(current_p);

        let full_path = if path.starts_with(".") {
            // Relative path (e.g., import "./utils")
            let mut p = parent_dir.join(path);
            if p.extension().is_none() {
                p.set_extension("rhai");
            }
            p
        } else {
            // Module path (e.g., import "nginx")
            let base = self.module_path.lock().expect("Failed to lock module path");
            if let Some(ref bp) = *base {
                bp.join(path).join("manifests").join("init.rhai")
            } else {
                return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Module path not set, cannot resolve '{}'", path).into(),
                    pos,
                )));
            }
        };

        if !full_path.exists() {
            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("File not found: {}", full_path.display()).into(),
                pos,
            )));
        }

        Ok(full_path)
    }
}

impl rhai::ModuleResolver for PupoxideModuleResolver {
    /// Resolves and compiles an imported manifest module (e.g., `import "nginx"`).
    ///
    /// The method performs the following steps for module isolation and tracking:
    /// 1. Locates the absolute path to the module file (relative to the current path or inside `modules/`).
    /// 2. Generates unique metadata resource markers for module start (`ModuleStart[...]`) and end (`ModuleEnd[...]`).
    /// 3. Adds the start marker to the catalog and links it to the parent module (to maintain dependency ordering).
    /// 4. Pushes the module name onto the execution stack and temporarily switches the engine's working directory.
    /// 5. Compiles and evaluates the Rhai module's AST inside an isolated `Scope`, passing the system facts.
    /// 6. Restores the working directory and pops the module from the stack after execution.
    /// 7. Creates the `ModuleEnd` marker, which depends on the start marker and all resources declared within the module.
    fn resolve(
        &self,
        engine: &rhai::Engine,
        _source_path: Option<&str>,
        path: &str,
        pos: rhai::Position,
    ) -> std::result::Result<Arc<rhai::Module>, Box<rhai::EvalAltResult>> {
        // 1. Get context and resolve file path
        let exec_ctx = self.get_context(pos)?;
        let full_path = self.resolve_full_path(path, &exec_ctx, pos)?;

        let module_name = path.to_string();

        // 2. Prepare environment for module execution under a single lock
        let (handle, old_path, start_node_count) = {
            let mut state = exec_ctx.state.lock().map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!("Failed to lock state: {}", e).into(),
                    pos,
                ))
            })?;
            state.enter_inclusion(InclusionType::Module, module_name, full_path.clone())
        };

        // 3. Compile and execute AST (lock is released)
        let mut ast = engine.compile_file(full_path).map_err(|e| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Import compilation error '{}': {}", path, e).into(),
                pos,
            ))
        })?;
        ast.set_source(path);

        let mut scope = rhai::Scope::new();
        // Pass "facts" into module scope
        let mut facts_map = Map::new();
        for (k, v) in exec_ctx.facts.values.clone() {
            facts_map.insert(k.into(), v.into());
        }
        scope.set_value("facts", facts_map);

        let eval_res = CURRENT_EXEC_CTX.with(|ctx| {
            let old_ctx = ctx.borrow().clone();
            *ctx.borrow_mut() = Some(exec_ctx.clone());
            let r = engine.eval_ast_with_scope::<Dynamic>(&mut scope, &ast);
            *ctx.borrow_mut() = old_ctx;
            r
        });

        // 4. Restore state after execution and add end marker under a single lock
        let mut state = exec_ctx.state.lock().map_err(|e| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Failed to lock state: {}", e).into(),
                pos,
            ))
        })?;
        state.exit_inclusion(&handle, old_path, start_node_count);
        drop(state); // explicitly drop to release lock

        let _ = eval_res.map_err(|e| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Import execution error '{}': {}", path, e).into(),
                pos,
            ))
        })?;

        // 5. Create Rhai module to return
        let mut module = rhai::Module::eval_ast_as_new(scope, &ast, engine)?;
        module.set_var("module_handle", handle);

        Ok(Arc::new(module))
    }
}

impl PupoxideEngine {
    /// Creates a default `PupoxideEngine` with an optional data stash provider.
    pub fn new(stash: Option<Arc<dyn StashProvider>>) -> Self {
        let mut builder = PupoxideEngineBuilder::new();
        if let Some(s) = stash {
            builder = builder.with_stash(s);
        }
        builder.register_defaults().build()
    }

    /// Returns a new `PupoxideEngineBuilder` to customize the engine setup.
    pub fn builder() -> PupoxideEngineBuilder {
        PupoxideEngineBuilder::new()
    }

    /// Sets the base path for finding external modules.
    pub fn set_module_path(&self, path: PathBuf) {
        *self.module_path.lock().expect("Failed to lock module path") = Some(path);
    }

    /// Compiles and evaluates the root manifest, building and returning a dependency-ordered `Catalog`.
    ///
    /// # Errors
    ///
    /// Returns an error if Rhai compilation, evaluation, or final catalog topological sorting fails.
    pub fn run_manifest(
        &self,
        path: PathBuf,
        node_name: String,
        environment: String,
        facts: Facts,
    ) -> Result<Catalog> {
        let exec_ctx = ExecutionContext::new(facts.clone(), path.clone(), node_name, environment);

        // Root manifest source mapping
        {
            let mut state = exec_ctx.state.lock().expect("Failed to lock state");
            // Common root names in Rhai
            state.source_map.insert("".to_string(), "Root".to_string());
            state
                .source_map
                .insert("site".to_string(), "Root".to_string());
        }

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
            *ctx.borrow_mut() = Some(exec_ctx.clone());
            r
        });

        let _ = eval_res.map_err(|e| anyhow::anyhow!("Rhai execution error: {}", e))?;

        let mut catalog = exec_ctx
            .state
            .lock()
            .expect("Failed to lock state")
            .catalog
            .clone();

        // Ensure ID map is ready (though it should be for freshly built catalog)
        catalog.rebuild_id_map();

        // Ensure all edges from resource struct dependencies are present in graph
        catalog.build_edges();

        // Final topological sort to ensure evaluation order is correct
        let _ = catalog.topological_sort()?;

        Ok(catalog)
    }

    /// A helper method that sets the module search path and immediately evaluates the root manifest.
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
}
