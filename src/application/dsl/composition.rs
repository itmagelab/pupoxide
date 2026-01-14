use super::context::DslContext;
use crate::application::engine::{InclusionType, ModuleHandle};
use crate::domain::resource::{MetaKind, MetaResource, Resource};
use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn register(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
    let m_path = module_path.clone();

    let create_include_fn = |inc_type: InclusionType| {
        let m_path = m_path.clone();
        move |ctx: NativeCallContext,
              name: String|
              -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();

            // Check constraints
            {
                let stack = exec_ctx
                    .inclusion_stack
                    .lock()
                    .expect("Failed to lock inclusion stack");
                if stack.last() == Some(&InclusionType::Role) && inc_type != InclusionType::Profile {
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        "Roles can ONLY include profiles. Technical modules or other roles are not allowed.".into(),
                        rhai::Position::NONE,
                    )));
                }
            }

            let handle = ModuleHandle {
                name: name.clone(),
                start_id: format!("ModuleStart[{}]", name),
                end_id: format!("ModuleEnd[{}]", name),
            };

            let mut included = exec_ctx
                .included_modules
                .lock()
                .expect("Failed to lock included modules");
            if included.contains(&handle.start_id) {
                return Ok(handle);
            }
            included.insert(handle.start_id.clone());
            drop(included);

            let mut current_p = exec_ctx
                .current_path
                .lock()
                .expect("Failed to lock current path");
            let parent_dir = current_p.parent().unwrap_or(&current_p);

            let full_path = if name.starts_with(".") {
                let mut p = parent_dir.join(&name);
                if p.extension().is_none() {
                    p.set_extension("rhai");
                }
                p
            } else {
                let base = m_path.lock().expect("Failed to lock module path");
                if let Some(ref bp) = *base {
                    match inc_type {
                        InclusionType::Module => bp.join(&name).join("manifests").join("init.rhai"),
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
                } else {
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Base path not set, cannot include '{}' ({:?})", name, inc_type)
                            .into(),
                        rhai::Position::NONE,
                    )));
                }
            };

            if full_path.exists() {
                let start_resource_count = exec_ctx
                    .resources
                    .lock()
                    .expect("Failed to lock resources")
                    .len();

                {
                    let mut resources = exec_ctx.resources.lock().expect("Failed to lock resources");
                    let mut dependencies = Vec::new();

                    // Add dependency on parent module if exists
                    let stack = exec_ctx.module_stack.lock().expect("Failed to lock module stack");
                    if let Some(parent) = stack.last() {
                        dependencies.push(format!("ModuleStart[{}]", parent));
                    }
                    drop(stack);

                    resources.push(Resource::Meta(MetaResource {
                        id: handle.start_id.clone(),
                        kind: MetaKind::ModuleStart,
                        dependencies,
                    }));
                }

                exec_ctx
                    .module_stack
                    .lock()
                    .expect("Failed to lock module stack")
                    .push(name.clone());

                // Track inclusion context
                exec_ctx
                    .inclusion_stack
                    .lock()
                    .expect("Failed to lock inclusion stack")
                    .push(inc_type);

                let old_path = std::mem::replace(&mut *current_p, full_path.clone());
                drop(current_p);

                let mut scope = rhai::Scope::new();
                let mut facts_map = rhai::Map::new();
                for (k, v) in exec_ctx.facts.values.clone() {
                    facts_map.insert(k.into(), v.into());
                }
                scope.set_value("facts", facts_map);

                let eval_res = ctx.engine().eval_file_with_scope::<Dynamic>(&mut scope, full_path).map_err(|e| {
                    Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Failed to include {:?} '{}': {}", inc_type, name, e).into(),
                        rhai::Position::NONE,
                    ))
                });

                // Restore inclusion context
                exec_ctx
                    .inclusion_stack
                    .lock()
                    .expect("Failed to lock inclusion stack")
                    .pop();

                let mut current_p = exec_ctx
                    .current_path
                    .lock()
                    .expect("Failed to lock current path");
                *current_p = old_path;

                exec_ctx
                    .module_stack
                    .lock()
                    .expect("Failed to lock module stack")
                    .pop();

                let _ = eval_res?;

                {
                    let mut resources = exec_ctx.resources.lock().expect("Failed to lock resources");
                    let mut end_deps = vec![handle.start_id.clone()];

                    for i in start_resource_count..resources.len() {
                        end_deps.push(resources[i].id().to_string());
                    }

                    resources.push(Resource::Meta(MetaResource {
                        id: handle.end_id.clone(),
                        kind: MetaKind::ModuleEnd,
                        dependencies: end_deps,
                    }));
                }

                return Ok(handle);
            }
            Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("{:?} {} not found at {}", inc_type, name, full_path.display()).into(),
                rhai::Position::NONE,
            )))
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
