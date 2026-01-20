use super::context::DslContext;
use crate::application::engine::ModuleHandle;
use crate::domain::resource::Resource;
use rhai::Engine;

// Helper to extract ModuleHandle from a Rhai module
fn get_module_handle(module: &rhai::Shared<rhai::Module>) -> Option<ModuleHandle> {
    module
        .get_var("module_handle")
        .and_then(|v| v.try_cast::<ModuleHandle>())
}

pub fn register(engine: &mut Engine) {
    // SAFETY: Registered at startup, failure should never happen with valid name.
    engine
        .register_custom_operator("->", 60)
        .expect("Failed to register custom operator");

    // Resource -> Resource
    engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
        DslContext::add_dependency_between_ids(lhs.id(), rhs.id());
        rhs
    });

    // ModuleHandle -> ModuleHandle
    engine.register_fn("->", move |lhs: ModuleHandle, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(&lhs.end_id, &rhs.start_id);
        rhs
    });

    // Resource -> ModuleHandle
    engine.register_fn("->", move |lhs: Resource, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(lhs.id(), &rhs.start_id);
        rhs
    });

    // ModuleHandle -> Resource
    engine.register_fn("->", move |lhs: ModuleHandle, rhs: Resource| {
        DslContext::add_dependency_between_ids(&lhs.end_id, rhs.id());
        rhs
    });

    // Module -> Module
    engine.register_fn(
        "->",
        move |lhs: rhai::Shared<rhai::Module>, rhs: rhai::Shared<rhai::Module>| {
            if let (Some(lhs_h), Some(rhs_h)) = (get_module_handle(&lhs), get_module_handle(&rhs)) {
                DslContext::add_dependency_between_ids(&lhs_h.end_id, &rhs_h.start_id);
            }
            rhs
        },
    );

    // Module -> Resource
    engine.register_fn(
        "->",
        move |lhs: rhai::Shared<rhai::Module>, rhs: Resource| {
            if let Some(lhs_h) = get_module_handle(&lhs) {
                DslContext::add_dependency_between_ids(&lhs_h.end_id, rhs.id());
            }
            rhs
        },
    );

    // Resource -> Module
    engine.register_fn(
        "->",
        move |lhs: Resource, rhs: rhai::Shared<rhai::Module>| {
            if let Some(rhs_h) = get_module_handle(&rhs) {
                DslContext::add_dependency_between_ids(lhs.id(), &rhs_h.start_id);
            }
            rhs
        },
    );
}
