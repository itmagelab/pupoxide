use super::utils::DslUtils;
use crate::application::engine::ExecutionContext;
use crate::domain::resource::{PackageResource, Resource};
use rhai::{Engine, Map, Module, NativeCallContext};

fn map_to_json(map: &Map) -> Option<serde_json::Value> {
    let mut clean_map = map.clone();
    clean_map.remove("ensure");
    clean_map.remove("provider");
    clean_map.remove("dependencies");
    clean_map.remove("mutex");
    clean_map.remove("require");

    if clean_map.is_empty() {
        return None;
    }

    let dynamic = rhai::Dynamic::from(clean_map);
    rhai::serde::from_dynamic::<serde_json::Value>(&dynamic).ok()
}

pub fn register(engine: &mut Engine) {
    let mut module = Module::new();

    module.set_native_fn("version", || Ok(env!("CARGO_PKG_VERSION").to_string()));

    // 'pkg' function
    module.set_native_fn(
        "pkg",
        move |ctx: NativeCallContext, name: String, params: Map| {
            let exec_ctx = ExecutionContext::get_current();
            let ensure = DslUtils::extract_ensure(&params);
            let dependencies =
                DslUtils::extract_dependencies(&params, &exec_ctx, ctx.call_source());
            let provider = DslUtils::extract_string(&params, "provider")
                .unwrap_or_else(|| exec_ctx.get_default_provider());

            // Automatic mutex based on provider
            let mutex =
                DslUtils::extract_string(&params, "mutex").unwrap_or_else(|| provider.clone());

            let custom_params = map_to_json(&params);

            let resource = Resource::Package(PackageResource {
                id: format!("Package[{}]", name),
                name,
                ensure,
                provider,
                dependencies,
                mutex: Some(mutex),
                source_context: exec_ctx.get_source_context(),
                params: custom_params,
            });

            exec_ctx.add_resource(resource).map_err(|e| {
                Box::new(rhai::EvalAltResult::ErrorRuntime(
                    e.to_string().into(),
                    rhai::Position::NONE,
                ))
            })
        },
    );

    let shared_module: std::sync::Arc<Module> = module.into();
    engine.register_static_module("stdlib", shared_module.clone());
    engine.register_static_module("std", shared_module);
}
