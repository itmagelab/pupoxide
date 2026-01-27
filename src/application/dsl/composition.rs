use crate::application::engine::{ExecutionContext, InclusionType, ModuleHandle};
use crate::domain::resource::{MetaKind, MetaResource, Resource};
use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::warn;

// Helper to lock a mutex safely or return an error
fn lock_or_err<'a, T>(
    mutex: &'a Arc<Mutex<T>>,
    label: &str,
) -> std::result::Result<std::sync::MutexGuard<'a, T>, Box<rhai::EvalAltResult>> {
    mutex.lock().map_err(|e| {
        warn!("Failed to lock {}: {}", label, e);
        Box::new(rhai::EvalAltResult::ErrorRuntime(
            format!("Failed to lock {}: {}", label, e).into(),
            rhai::Position::NONE,
        ))
    })
}

// Resolve the full path for an inclusion based on type and name
fn resolve_inclusion_path(
    inc_type: InclusionType,
    name: &str,
    current_path: &std::path::Path,
    base_path: Option<&std::path::Path>,
) -> std::result::Result<PathBuf, Box<rhai::EvalAltResult>> {
    let full_path = if name.starts_with(".") {
        // Relative path (e.g., import "./utils")
        let mut p = current_path.parent().unwrap_or(current_path).join(name);
        if p.extension().is_none() {
            p.set_extension("rhai");
        }
        p
    } else {
        // Module/Role/Profile path
        let bp = base_path.ok_or_else(|| {
            Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!(
                    "Base path not set, cannot include '{}' ({:?})",
                    name, inc_type
                )
                .into(),
                rhai::Position::NONE,
            ))
        })?;

        match inc_type {
            InclusionType::Module => bp.join(name).join("manifests").join("init.rhai"),
            InclusionType::Role => bp
                .parent()
                .unwrap_or(bp)
                .join("role")
                .join(format!("{}.rhai", name)),
            InclusionType::Profile => bp
                .parent()
                .unwrap_or(bp)
                .join("profile")
                .join(format!("{}.rhai", name)),
        }
    };

    Ok(full_path)
}

pub fn register(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
    let m_path = module_path.clone();

    let create_include_fn = |inc_type: InclusionType| {
        let m_path = m_path.clone();
        move |ctx: NativeCallContext,
              name: String|
              -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();

            // Check constraints: Roles can only include Profiles
            {
                let current_type: std::sync::MutexGuard<Option<InclusionType>> =
                    lock_or_err(&exec_ctx.current_inclusion_type, "current_inclusion_type")?;
                if *current_type == Some(InclusionType::Role) && inc_type != InclusionType::Profile
                {
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        "Roles can ONLY include profiles. Technical modules or other roles are not allowed.".into(),
                        rhai::Position::NONE,
                    )));
                }
            }

            let handle = ModuleHandle {
                name: name.clone(),
                start_id: format!("{:?}Start[{}]", inc_type, name),
                end_id: format!("{:?}End[{}]", inc_type, name),
            };

            // Check if already included
            let mut included: std::sync::MutexGuard<std::collections::HashSet<String>> =
                lock_or_err(&exec_ctx.included_modules, "included_modules")?;
            if included.contains(&handle.start_id) {
                return Ok(handle);
            }
            included.insert(handle.start_id.clone());
            drop(included);

            // Get current path and base path for resolution
            let current_p: std::sync::MutexGuard<PathBuf> =
                lock_or_err(&exec_ctx.current_path, "current_path")?;
            let base = lock_or_err(&m_path, "module_path")?;
            let full_path = resolve_inclusion_path(
                inc_type,
                &name,
                &current_p,
                base.as_ref().map(|p| p.as_path()),
            )?;
            drop(current_p);
            drop(base);

            if !full_path.exists() {
                return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!(
                        "{:?} {} not found at {}",
                        inc_type,
                        name,
                        full_path.display()
                    )
                    .into(),
                    rhai::Position::NONE,
                )));
            }

            // Track resource count before inclusion
            let start_node_count = {
                let catalog: std::sync::MutexGuard<crate::domain::catalog::Catalog> =
                    lock_or_err(&exec_ctx.catalog, "catalog")?;
                catalog.graph.node_count()
            };

            // Add module start marker
            {
                let mut catalog = lock_or_err(&exec_ctx.catalog, "catalog")?;
                let mut dependencies = Vec::new();
                if let Ok(stack) = exec_ctx.module_stack.lock()
                    && let Some((parent_type, parent_name)) = stack.last()
                {
                    let dep = format!("{:?}Start[{}]", parent_type, parent_name);
                    dependencies.push(dep);
                }
                catalog.add_resource(Resource::Meta(MetaResource {
                    id: handle.start_id.clone(),
                    kind: MetaKind::ModuleStart,
                    dependencies: dependencies.clone(),
                }));
                // Add edges from dependencies
                for dep in dependencies {
                    let _ = catalog.add_dependency(&dep, &handle.start_id);
                }
            }

            // Push module to stack
            {
                let mut stack = lock_or_err(&exec_ctx.module_stack, "module_stack")?;
                stack.push((inc_type, name.clone()));
            }

            // Save and update inclusion context
            let old_inclusion_type = {
                let mut current =
                    lock_or_err(&exec_ctx.current_inclusion_type, "current_inclusion_type")?;
                (*current).replace(inc_type)
            };

            // Save and update current path
            let old_path = {
                let mut current = lock_or_err(&exec_ctx.current_path, "current_path")?;
                std::mem::replace(&mut *current, full_path.clone())
            };

            // Prepare and execute the included file
            let eval_res = {
                let mut scope = rhai::Scope::new();
                let mut facts_map = rhai::Map::new();
                for (k, v) in exec_ctx.facts.values.clone() {
                    facts_map.insert(k.into(), v.into());
                }
                scope.set_value("facts", facts_map);

                ctx.engine()
                    .eval_file_with_scope::<Dynamic>(&mut scope, full_path)
                    .map_err(|e| {
                        Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to include {:?} '{}': {}", inc_type, name, e).into(),
                            rhai::Position::NONE,
                        ))
                    })
            };

            // Restore inclusion context
            {
                let mut current =
                    lock_or_err(&exec_ctx.current_inclusion_type, "current_inclusion_type")?;
                *current = old_inclusion_type;
            }

            // Restore current path
            {
                let mut current = lock_or_err(&exec_ctx.current_path, "current_path")?;
                *current = old_path;
            }

            // Pop module from stack
            {
                let mut stack = lock_or_err(&exec_ctx.module_stack, "module_stack")?;
                stack.pop();
            }

            // Check if evaluation succeeded
            let _ = eval_res?;

            // Add module end marker
            {
                let mut catalog = lock_or_err(&exec_ctx.catalog, "catalog")?;

                let mut end_deps = Vec::new();

                let current_count = catalog.graph.node_count();
                // Collect IDs of resources added between start_node_count and current_count
                for i in start_node_count..current_count {
                    let idx = petgraph::graph::NodeIndex::new(i);
                    if let Some(res) = catalog.graph.node_weight(idx) {
                        // Skip if it is the start marker (handle.start_id)
                        if res.id() != handle.start_id {
                            end_deps.push(res.id().to_string());
                        }
                    }
                }

                if end_deps.is_empty() {
                    end_deps.push(handle.start_id.clone());
                }

                catalog.add_resource(Resource::Meta(MetaResource {
                    id: handle.end_id.clone(),
                    kind: MetaKind::ModuleEnd,
                    dependencies: end_deps.clone(),
                }));

                for dep in end_deps {
                    let _ = catalog.add_dependency(&dep, &handle.end_id);
                }
            }

            Ok(handle)
        }
    };

    let include_fn = create_include_fn(InclusionType::Module);
    engine.register_fn("include", include_fn.clone());
    engine.register_fn("get$include", include_fn);

    let role_fn = create_include_fn(InclusionType::Role);
    engine.register_fn("role", role_fn.clone());
    engine.register_fn("get$role", role_fn);

    let profile_fn = create_include_fn(InclusionType::Profile);
    engine.register_fn("profile", profile_fn.clone());
    engine.register_fn("get$profile", profile_fn);
}
