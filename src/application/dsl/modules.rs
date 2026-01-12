use rhai::{Dynamic, Engine, NativeCallContext};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::domain::resource::{MetaKind, MetaResource, Resource};
use crate::application::engine::ModuleHandle;
use super::context::DslContext;

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

            let base = m_path.lock().expect("Failed to lock module path");
            if let Some(ref bp) = *base {
                let init_path = bp.join(&name).join("manifests").join("init.rhai");
                if init_path.exists() {
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

                    let _ = ctx.engine().eval_file::<Dynamic>(init_path).map_err(|e| {
                        Box::new(rhai::EvalAltResult::ErrorRuntime(
                            format!("Failed to include module '{}': {}", name, e).into(),
                            rhai::Position::NONE,
                        ))
                    })?;

                    exec_ctx
                        .module_stack
                        .lock()
                        .expect("Failed to lock module stack")
                        .pop();

                    {
                        let mut resources =
                            exec_ctx.resources.lock().expect("Failed to lock resources");
                        resources.push(Resource::Meta(MetaResource {
                            id: handle.end_id.clone(),
                            kind: MetaKind::ModuleEnd,
                            dependencies: vec![handle.start_id.clone()],
                        }));
                    }

                    return Ok(handle);
                }
            }
            Err(Box::new(rhai::EvalAltResult::ErrorRuntime(
                format!("Module {} not found", name).into(),
                rhai::Position::NONE,
            )))
        },
    );
}
