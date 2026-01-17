use super::context::DslContext;
use crate::application::engine::{InclusionType, ModuleHandle};
use crate::domain::resource::{MetaKind, MetaResource, Resource};
use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::warn;

pub fn register(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
    let m_path = module_path.clone();

    let create_include_fn = |inc_type: InclusionType| {
        let m_path = m_path.clone();
        move |ctx: NativeCallContext,
              name: String|
              -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();

            // Check constraints: Roles can only include Profiles
            {
                let current_type = match exec_ctx.current_inclusion_type.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        warn!("Failed to lock current inclusion type: {}", e);
                        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to lock current inclusion type: {}", e).into(),
                            rhai::Position::NONE,
                        )));
                    }
                };
                if *current_type == Some(InclusionType::Role) && inc_type != InclusionType::Profile {
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

            let mut included = match exec_ctx.included_modules.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    warn!("Failed to lock included modules: {}", e);
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Failed to lock included modules: {}", e).into(),
                        rhai::Position::NONE,
                    )));
                }
            };
            if included.contains(&handle.start_id) {
                return Ok(handle);
            }
            included.insert(handle.start_id.clone());
            drop(included);

            let mut current_p = match exec_ctx.current_path.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    warn!("Failed to lock current path: {}", e);
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Failed to lock current path: {}", e).into(),
                        rhai::Position::NONE,
                    )));
                }
            };
            let parent_dir = current_p.parent().unwrap_or(&current_p);

            let full_path = if name.starts_with(".") {
                let mut p = parent_dir.join(&name);
                if p.extension().is_none() {
                    p.set_extension("rhai");
                }
                p
            } else {
                let base = match m_path.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        warn!("Failed to lock module path: {}", e);
                        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to lock module path: {}", e).into(),
                            rhai::Position::NONE,
                        )));
                    }
                };
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
                let start_resource_count = match exec_ctx.resources.lock() {
                    Ok(guard) => guard.len(),
                    Err(e) => {
                        warn!("Failed to lock resources: {}", e);
                        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to lock resources: {}", e).into(),
                            rhai::Position::NONE,
                        )));
                    }
                };

                {
                    let mut resources = match exec_ctx.resources.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            warn!("Failed to lock resources: {}", e);
                            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                                format!("Failed to lock resources: {}", e).into(),
                                rhai::Position::NONE,
                            )));
                        }
                    };
                    let mut dependencies = Vec::new();

                    // Add dependency on parent module if exists
                    if let Ok(stack) = exec_ctx.module_stack.lock() {
                        if let Some(parent) = stack.last() {
                            dependencies.push(format!("ModuleStart[{}]", parent));
                        }
                    }
                    
                    resources.push(Resource::Meta(MetaResource {
                        id: handle.start_id.clone(),
                        kind: MetaKind::ModuleStart,
                        dependencies,
                    }));
                }

                let module_stack_result = exec_ctx.module_stack.lock();
                if let Ok(mut stack) = module_stack_result {
                    stack.push(name.clone());
                } else {
                    warn!("Failed to lock module stack");
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        "Failed to lock module stack".into(),
                        rhai::Position::NONE,
                    )));
                }

                // Save old inclusion context and set new one
                let old_inclusion_type = {
                    let mut current = match exec_ctx.current_inclusion_type.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            warn!("Failed to lock current inclusion type: {}", e);
                            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                                "Failed to lock current inclusion type".into(),
                                rhai::Position::NONE,
                            )));
                        }
                    };
                    std::mem::replace(&mut *current, Some(inc_type))
                };

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
                {
                    let mut current = match exec_ctx.current_inclusion_type.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            warn!("Failed to lock current inclusion type: {}", e);
                            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                                "Failed to lock current inclusion type".into(),
                                rhai::Position::NONE,
                            )));
                        }
                    };
                    *current = old_inclusion_type;
                }

                let current_path_result = exec_ctx.current_path.lock();
                let mut current_p = match current_path_result {
                    Ok(guard) => guard,
                    Err(e) => {
                        warn!("Failed to lock current path: {}", e);
                        return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to lock current path: {}", e).into(),
                            rhai::Position::NONE,
                        )));
                    }
                };
                *current_p = old_path;

                let module_stack_result = exec_ctx.module_stack.lock();
                if let Ok(mut stack) = module_stack_result {
                    stack.pop();
                } else {
                    warn!("Failed to lock module stack");
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        "Failed to lock module stack".into(),
                        rhai::Position::NONE,
                    )));
                }

                let _ = eval_res?;

                {
                    let mut resources = match exec_ctx.resources.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            warn!("Failed to lock resources: {}", e);
                            return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                                format!("Failed to lock resources: {}", e).into(),
                                rhai::Position::NONE,
                            )));
                        }
                    };
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
