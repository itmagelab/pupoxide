use super::context::DslContext;
use crate::application::engine::ModuleHandle;
use crate::domain::resource::Resource;
use rhai::Engine;

pub fn register(engine: &mut Engine) {
    engine
        .register_custom_operator("->", 60)
        .expect("Failed to register custom operator");

    engine.register_fn("->", move |lhs: Resource, rhs: Resource| {
        DslContext::add_dependency_between_ids(lhs.id(), rhs.id());
        rhs
    });

    engine.register_fn("->", move |lhs: ModuleHandle, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(&lhs.end_id, &rhs.start_id);
        rhs
    });

    engine.register_fn("->", move |lhs: Resource, rhs: ModuleHandle| {
        DslContext::add_dependency_between_ids(lhs.id(), &rhs.start_id);
        rhs
    });

    engine.register_fn("->", move |lhs: ModuleHandle, rhs: Resource| {
        DslContext::add_dependency_between_ids(&lhs.end_id, rhs.id());
        rhs
    });

    engine.register_fn(
        "->",
        move |lhs: rhai::Shared<rhai::Module>, rhs: rhai::Shared<rhai::Module>| {
            let lhs_h = lhs
                .get_var("module_handle")
                .and_then(|v| v.try_cast::<ModuleHandle>());
            let rhs_h = rhs
                .get_var("module_handle")
                .and_then(|v| v.try_cast::<ModuleHandle>());

            if let (Some(lhs_h), Some(rhs_h)) = (lhs_h, rhs_h) {
                DslContext::add_dependency_between_ids(&lhs_h.end_id, &rhs_h.start_id);
            }
            rhs
        },
    );

    engine.register_fn(
        "->",
        move |lhs: rhai::Shared<rhai::Module>, rhs: Resource| {
            if let Some(lhs_h) = lhs
                .get_var("module_handle")
                .and_then(|v| v.try_cast::<ModuleHandle>())
            {
                DslContext::add_dependency_between_ids(&lhs_h.end_id, rhs.id());
            }
            rhs
        },
    );

    engine.register_fn(
        "->",
        move |lhs: Resource, rhs: rhai::Shared<rhai::Module>| {
            if let Some(rhs_h) = rhs
                .get_var("module_handle")
                .and_then(|v| v.try_cast::<ModuleHandle>())
            {
                DslContext::add_dependency_between_ids(lhs.id(), &rhs_h.start_id);
            }
            rhs
        },
    );
}
