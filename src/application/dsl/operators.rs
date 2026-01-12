use rhai::Engine;
use crate::domain::resource::Resource;
use crate::application::engine::ModuleHandle;
use super::context::DslContext;

pub fn register(engine: &mut Engine) {
    engine
        .register_custom_operator("->", 60)
        .expect("Failed to register custom operator");

    engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
        DslContext::add_dependency_between_ids(&lhs.id().to_string(), &rhs.id().to_string());
        rhs
    });

    engine.register_fn("->", move |lhs: ModuleHandle, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(&lhs.end_id, &rhs.start_id);
        rhs
    });

    engine.register_fn("->", move |lhs: Resource, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(&lhs.id().to_string(), &rhs.start_id);
        rhs
    });

    engine.register_fn("->", move |lhs: ModuleHandle, rhs: Resource| {
        DslContext::add_dependency_between_ids(&lhs.end_id, &rhs.id().to_string());
        rhs
    });
}
