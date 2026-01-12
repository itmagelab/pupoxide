use crate::infrastructure::stash::Stash;
use rhai::Engine;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod context;
pub mod modules;
pub mod operators;
pub mod resources;
pub mod stash;
pub mod types;

pub fn register_all(
    engine: &mut Engine,
    stash: Arc<Option<Stash>>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
) {
    types::register(engine);
    stash::register(engine, stash);
    operators::register(engine);
    modules::register(engine, module_path);
    resources::register(engine);
}
