use super::context::DslContext;
use crate::application::engine::ModuleHandle;
use crate::domain::resource::{MetaKind, MetaResource, Resource};
use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn register(engine: &mut Engine, module_path: Arc<Mutex<Option<PathBuf>>>) {
    let m_path = module_path.clone();
    engine.register_fn(
        "include",
        move |ctx: NativeCallContext,
              name: String|
              -> std::result::Result<ModuleHandle, Box<rhai::EvalAltResult>> {
            let exec_ctx = DslContext::get_exec_ctx();

            let handle = ModuleHandle {
                name: name.clone(),
                start_id: format!("ModuleStart[{}]", name),
                end_id: format!("ModuleEnd[{}]", name),
            };

            let mut included = exec_ctx
                .included_modules
                .lock()
                .expect("Failed to lock included modules");
            if included.contains(&name) {
                return Ok(handle);
            }
            included.insert(name.clone());
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
                    bp.join(&name).join("manifests").join("init.rhai")
                } else {
                    return Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Module path not set, cannot include '{}'", name).into(),
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
                    let mut resources =
                        exec_ctx.resources.lock().expect("Failed to lock resources");
                    resources.push(Resource::Meta(MetaResource {
                        id: handle.start_id.clone(),
                        kind: MetaKind::ModuleStart,
                        dependencies: Vec::new(),
                    }));
                }

                exec_ctx
                    .module_stack
                    .lock()
                    .expect("Failed to lock module stack")
                    .push(name.clone());

                let old_path = std::mem::replace(&mut *current_p, full_path.clone());
                drop(current_p);

                let _ = ctx.engine().eval_file::<Dynamic>(full_path).map_err(|e| {
                    Box::new(rhai::EvalAltResult::ErrorRuntime(
                        format!("Failed to include module '{}': {}", name, e).into(),
                        rhai::Position::NONE,
                    ))
                })?;

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

                {
                    let mut resources =
                        exec_ctx.resources.lock().expect("Failed to lock resources");
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
                format!("Module {} not found", name).into(),
                rhai::Position::NONE,
            )))
        },
    );
}
