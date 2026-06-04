use crate::application::engine::{ExecutionContext, InclusionType, ModuleHandle};
use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
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

enum InclusionTarget {
    Relative(String),
    Named(String),
}

impl FromStr for InclusionTarget {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let path = Path::new(s);
        match path.components().next() {
            Some(Component::CurDir) | Some(Component::ParentDir) => {
                Ok(Self::Relative(s.to_string()))
            }
            _ => Ok(Self::Named(s.to_string())),
        }
    }
}

impl InclusionType {
    /// Resolves the file path for the given inclusion type and name relative to the base path.
    pub fn resolve_path(&self, base_path: &std::path::Path, name: &str) -> PathBuf {
        match self {
            InclusionType::Module => base_path.join(name).join("manifests").join("init.rhai"),
            InclusionType::Role => base_path
                .parent()
                .unwrap_or(base_path)
                .join("role")
                .join(format!("{}.rhai", name)),
            InclusionType::Profile => base_path
                .parent()
                .unwrap_or(base_path)
                .join("profile")
                .join(format!("{}.rhai", name)),
        }
    }
}

// Resolve the full path for an inclusion based on type and name
fn resolve_inclusion_path(
    inc_type: InclusionType,
    name: &str,
    current_path: &std::path::Path,
    base_path: Option<&std::path::Path>,
) -> std::result::Result<PathBuf, Box<rhai::EvalAltResult>> {
    match name.parse::<InclusionTarget>().unwrap() {
        InclusionTarget::Relative(rel_path) => {
            let mut p = current_path.parent().unwrap_or(current_path).join(rel_path);
            if p.extension().is_none() {
                p.set_extension("rhai");
            }
            Ok(p)
        }
        InclusionTarget::Named(named_name) => {
            let bp = base_path.ok_or_else(|| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    format!(
                        "Base path not set, cannot include '{}' ({:?})",
                        named_name, inc_type
                    )
                    .into(),
                    rhai::Position::NONE,
                ))
            })?;

            Ok(inc_type.resolve_path(bp, &named_name))
        }
    }
}

pub fn register(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
    let m_path = module_path.clone();

    let create_include_fn = |inc_type: InclusionType| {
        let m_path = m_path.clone();
        move |ctx: NativeCallContext,
              name: String|
              -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = ExecutionContext::get_current();

            let mut state = lock_or_err(&exec_ctx.state, "state")?;

            // Check constraints: Roles can only include Profiles
            if state.current_inclusion_type() == Some(InclusionType::Role)
                && inc_type != InclusionType::Profile
            {
                return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                    "Roles can ONLY include profiles. Technical modules or other roles are not allowed.".into(),
                    rhai::Position::NONE,
                )));
            }

            let start_id = format!("{:?}Start[{}]", inc_type, name);

            // Check if already included
            if state.included_modules.contains(&start_id) {
                return Ok(ModuleHandle {
                    name: name.clone(),
                    start_id: start_id.clone(),
                    end_id: format!("{:?}End[{}]", inc_type, name),
                });
            }
            state.included_modules.insert(start_id);

            // Get current path and base path for resolution
            let base = lock_or_err(&m_path, "module_path")?;
            let full_path = resolve_inclusion_path(
                inc_type,
                &name,
                &state.current_path,
                base.as_ref().map(|p| p.as_path()),
            )?;
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

            let (handle, old_path, start_node_count) =
                state.enter_inclusion(inc_type, name.clone(), full_path.clone());

            // Release lock before evaluating file to avoid deadlocks on nested calls
            drop(state);

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

            // Re-acquire lock to restore context, stack, and current path, and add end marker
            let mut state = lock_or_err(&exec_ctx.state, "state")?;
            state.exit_inclusion(&handle, old_path, start_node_count);

            // Check if evaluation succeeded
            let _ = eval_res?;

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
