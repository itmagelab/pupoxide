#![deny(clippy::unwrap_used)]
#![allow(deprecated, clippy::unnecessary_struct_initialization)]
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;

pub use domain::resource;
pub use infrastructure::FsAdapter;
