#![deny(clippy::unwrap_used)]
use anyhow::Result;
use clap::Parser;
use pupoxide::application::{EnvironmentLoader, PupoxideEngine};

use pupoxide::interface::{Cli, Commands};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let state_dir = PathBuf::from("/tmp/pupoxide");
    let state_store = pupoxide::infrastructure::StateStore::new(state_dir.join("state"));

    // Initialize provider registry with default adapters
    let mut provider_registry = pupoxide::application::ProviderRegistry::new();
    provider_registry.register(std::sync::Arc::new(pupoxide::infrastructure::FsAdapter));
    provider_registry.register(std::sync::Arc::new(pupoxide::infrastructure::ExecAdapter));
    let provider = std::sync::Arc::new(provider_registry);

    match cli.command {
        Commands::Run {
            file,
            module_path,
            dry_run,
        } => {
            let engine = PupoxideEngine::new(None);

            // Smart module path resolution
            let resolved_module_path = if let Some(mp) = module_path {
                Some(mp)
            } else {
                file.parent().and_then(|p| {
                    let sibling_modules = if p.ends_with("manifests") {
                        p.parent().map(|parent| parent.join("modules"))
                    } else {
                        Some(p.join("modules"))
                    };
                    sibling_modules.filter(|path| path.exists())
                })
            };

            if let Some(mp) = resolved_module_path {
                engine.set_module_path(mp);
            }

            let facts = pupoxide::infrastructure::Facter::collect();
            let catalog =
                engine.run_manifest(file, "localhost".to_string(), "local".to_string(), facts)?;

            pupoxide::application::execute_transaction(
                catalog,
                &state_store,
                provider,
                dry_run,
            )
            .await?;
        }
        Commands::Apply {
            environment,
            dry_run,
        } => {
            let loader = EnvironmentLoader::new(cli.config);
            let manifest_path = loader.get_site_manifest(&environment)?;
            let modules_path = loader.get_modules_path(&environment);

            let mut stash = None;
            let env_path = loader
                .get_modules_path(&environment)
                .parent()
                .expect("Environment path must have a parent")
                .to_path_buf();
            match pupoxide::infrastructure::Stash::new(env_path) {
                Ok(s) => stash = s,
                Err(e) => tracing::warn!("Failed to load Stash: {}", e),
            }

            let engine = PupoxideEngine::new(stash);
            let facts = pupoxide::infrastructure::Facter::collect();
            let catalog = engine.run_manifest_with_modules(
                manifest_path,
                modules_path,
                "localhost".to_string(),
                environment,
                facts,
            )?;

            pupoxide::application::execute_transaction(
                catalog,
                &state_store,
                provider,
                dry_run,
            )
            .await?;
        }

        Commands::Master { port } => {
            let loader = EnvironmentLoader::new(cli.config);
            let engine = PupoxideEngine::new(None); // Master might need to load Hiera per request/environment later
            let state = pupoxide::interface::server::MasterState { engine, loader };
            pupoxide::interface::server::start_master(state, port).await?;
        }
        Commands::Agent {
            server,
            node,
            environment,
            dry_run,
        } => {
            let agent = pupoxide::interface::agent::PupoxideAgent::new(server, node, environment);
            agent.run(dry_run).await?;
        }
    }

    Ok(())
}
