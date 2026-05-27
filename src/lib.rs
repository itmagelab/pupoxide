//! # Pupoxide
//!
//! A high-performance, memory-safe, declarative configuration management tool inspired by Puppet.
//!
//! Pupoxide leverages the power and safety of Rust and integrates the flexible scripting capabilities of Rhai
//! to allow expressing complex system configurations cleanly, safely, and efficiently.
//!
//! ## Key Features
//!
//! - **Declarative Configuration DSL**: Describe files, directories, commands, and packages using a simple Rhai-based DSL.
//! - **Safe Concurrent Execution**: Resources are executed in parallel using a Kahn-based topological sort, respecting dependency relationships.
//! - **Resource Mutexes**: Package managers (like `apt` or `brew`) and other conflicting resources are automatically serialized using internal mutex locks.
//! - **Hexagonal Architecture**: Core domains and applications are decoupled from infrastructure stores via clean ports and adapters.
//! - **Hierarchical Configuration (Stash)**: Dynamically resolve system variables based on target host facts using a Multi-level Tera templated configuration storage.
//! - **Zero Warnings & High Safety**: Built under a strict `#![deny(clippy::unwrap_used)]` compiler warning policy.
//!
//! ## Architecture Overview
//!
//! Pupoxide follows a clean **Ports & Adapters (Hexagonal)** architectural pattern inside `src/`:
//!
//! 1. **Domain (`src/domain/`)**: Houses the primary business entities (like `Resource`, `Catalog`, `Transaction`, `Facts`, and `ResourceReport`) and abstract port traits (like `ResourceProvider`). It is highly pure and contains minimal external dependencies.
//! 2. **Application (`src/application/`)**: Defines orchestrations, including DSL registration, manifest evaluation via `PupoxideEngine`, and execution graphs via `execute_transaction`. It relies on abstract ports like `StashProvider` and `StateStore`.
//! 3. **Infrastructure (`src/infrastructure/`)**: Implements specific I/O adapters, such as concrete `Stash` filesystems and transaction log `StateStore` adapters.
//! 4. **Interface (`src/interface/`)**: Entry point for CLI configurations, REST handlers, or agent bindings.
//!
//! ## Quick Start Example
//!
//! The following example demonstrates how to configure the engine, compile a manifest, and apply it to a target system.
//!
//! ### 1. Write a Rhai Manifest (`site.rhai`)
//!
//! ```rhai
//! // Declare a package resource
//! pkg("git", #{
//!     ensure: "present",
//! });
//!
//! // Create a custom configuration file depending on git
//! file("/etc/myapp/config.yaml", #{
//!     ensure: "present",
//!     content: "port: 8080\nenvironment: production",
//!     require: ["package:git"],
//! });
//! ```
//!
//! ### 2. Execute via Rust API
//!
//! ```rust,ignore
//! use pupoxide::application::{PupoxideEngine, execute_transaction};
//! use pupoxide::domain::facts::Facts;
//! use pupoxide::infrastructure::{Stash, StateStore, FsAdapter};
//! use std::path::PathBuf;
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 1. Gather host facts
//!     let mut facts = Facts::new();
//!     facts.insert("os_family".to_string(), "Darwin".to_string());
//!     facts.insert("hostname".to_string(), "my-macbook".to_string());
//!
//!     // 2. Initialize Infrastructure Adapters
//!     let env_path = PathBuf::from("./examples/environments/production");
//!     let stash = Stash::new(env_path.clone())?.map(|s| Arc::new(s) as Arc<dyn pupoxide::application::StashProvider>);
//!     let state_store = StateStore::new(env_path.clone());
//!     let provider = Arc::new(FsAdapter::new());
//!
//!     // 3. Build & Run the Pupoxide Engine
//!     let engine = PupoxideEngine::builder()
//!         .with_stash(stash.expect("stash config missing"))
//!         .with_module_path(env_path.join("modules"))
//!         .register_defaults()
//!         .build();
//!
//!     let catalog = engine.run_manifest(
//!         env_path.join("manifests/site.rhai"),
//!         "my-macbook".to_string(),
//!         "production".to_string(),
//!         facts,
//!     )?;
//!
//!     // 4. Apply configuration via Parallel Execution Graph
//!     let reports = execute_transaction(
//!         catalog,
//!         &state_store,
//!         provider,
//!         false, // dry_run
//!         |report| {
//!             println!("Resource [{}]: {:?}", report.resource_id, report.status);
//!         }
//!     ).await?;
//!
//!     println!("Successfully synchronized system configuration!");
//!     Ok(())
//! }
//! ```

#![deny(clippy::unwrap_used)]

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interface;

pub use domain::resource;
pub use infrastructure::FsAdapter;
