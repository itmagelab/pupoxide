#![deny(clippy::unwrap_used)]
pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod interface;

pub use domain::resource;
pub use infrastructure::FsAdapter;
