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
            
            // Initialize CA certificate
            let ca_cert_path = std::path::PathBuf::from("/etc/pupoxide/ca.pem");
            let ca_key_path = std::path::PathBuf::from("/etc/pupoxide/ca.key");
            let ca = pupoxide::infrastructure::CertificateAuthority::new_or_load(&ca_cert_path, &ca_key_path)?;
            
            // Save CA if not existed
            ca.save(&ca_cert_path, &ca_key_path)?;
            
            let bootstrap_manager = pupoxide::infrastructure::BootstrapTokenManager::new();
            let agent_registry = pupoxide::infrastructure::AgentRegistry::new();
            
            let state = pupoxide::interface::server::MasterState {
                engine,
                loader,
                ca,
                bootstrap_manager,
                agent_registry,
            };
            pupoxide::interface::server::start_master(state, port).await?;
        }
        Commands::Agent {
            server,
            node,
            environment,
            bootstrap,
            token,
            dry_run,
            cert_dir,
        } => {
            let agent = pupoxide::interface::agent::PupoxideAgent::new(server, node, environment, cert_dir);
            
            if bootstrap {
                // Phase 1: Bootstrap mode
                let bootstrap_token = token.ok_or_else(|| {
                    anyhow::anyhow!("--token is required when using --bootstrap flag")
                })?;
                agent.bootstrap(bootstrap_token).await?;
            } else {
                // Phase 2: Regular agent mode
                agent.run(dry_run).await?;
            }
        }
    }

    Ok(())
}
