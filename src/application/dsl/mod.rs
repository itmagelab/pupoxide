use rhai::Engine;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crate::infrastructure::hiera::Hiera;

pub mod context;
pub mod types;
pub mod hiera;
pub mod resources;
pub mod operators;
pub mod modules;

pub fn register_all(
    engine: &mut Engine,
    hiera: Arc<Option<Hiera>>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
) {
    types::register(engine);
    hiera::register(engine, hiera);
    operators::register(engine);
    modules::register(engine, module_path);
    resources::register(engine);
}
