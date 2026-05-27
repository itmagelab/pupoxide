use crate::application::StashProvider;
use rhai::Engine;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub mod composition;
pub mod operators;
pub mod resources;
pub mod stash;
pub mod stdlib;
pub mod types;
pub mod utils;

pub fn register_all(
    engine: &mut Engine,
    stash: Option<Arc<dyn StashProvider>>,
    module_path: Arc<Mutex<Option<PathBuf>>>,
) {
    types::register(engine);
    stash::register(engine, stash);
    operators::register(engine);
    composition::register(engine, module_path);
    resources::register(engine);
    stdlib::register(engine);
}
