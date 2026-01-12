use rhai::Engine;
use crate::domain::resource::{Ensure, Resource};
use crate::application::engine::ModuleHandle;

pub fn register(engine: &mut Engine) {
    engine.register_type_with_name::<Resource>("Resource");
    engine.register_type_with_name::<ModuleHandle>("ModuleHandle");
    engine
        .register_type_with_name::<Ensure>("Ensure")
        .register_fn("present", || Ensure::Present)
        .register_fn("absent", || Ensure::Absent);
}
