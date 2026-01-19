#![deny(clippy::unwrap_used)]
use anyhow::Result;
use clap::Parser;
use pupoxide::application::{EnvironmentLoader, PupoxideEngine};

use pupoxide::interface::{Cli, Commands, MasterAction};
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

            pupoxide::application::execute_transaction(catalog, &state_store, provider, dry_run)
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

            pupoxide::application::execute_transaction(catalog, &state_store, provider, dry_run)
                .await?;
        }

        Commands::Master { action, config } => {
            let config_dir = config.unwrap_or_else(|| cli.config.clone());

            // Create config directory and subdirectories if they don't exist
            tokio::fs::create_dir_all(&config_dir).await?;

            let loader = EnvironmentLoader::new(config_dir.clone());
            let engine = PupoxideEngine::new(None);

            // Create certs subdirectory for all certificate-related files
            let certs_dir = config_dir.join("certs");
            tokio::fs::create_dir_all(&certs_dir).await?;

            // Initialize CA certificate
            let ca_cert_path = certs_dir.join("ca.pem");
            let ca_key_path = certs_dir.join("ca.key");
            let ca = pupoxide::infrastructure::CertificateAuthority::new_or_load(
                &ca_cert_path,
                &ca_key_path,
            )?;

            // Save CA if not existed
            ca.save(&ca_cert_path, &ca_key_path)?;

            // Initialize bootstrap request manager and agent registry
            let bootstrap_requests_dir = certs_dir.join("bootstrap_requests");
            let agents_dir = certs_dir.join("agents");

            // Create subdirectories
            tokio::fs::create_dir_all(&bootstrap_requests_dir).await?;
            tokio::fs::create_dir_all(&agents_dir).await?;

            let bootstrap_manager =
                pupoxide::infrastructure::BootstrapRequestManager::new(bootstrap_requests_dir);
            let agent_registry = pupoxide::infrastructure::AgentRegistryFs::new(agents_dir);

            let state = pupoxide::interface::server::MasterState {
                engine,
                loader,
                ca,
                bootstrap_manager,
                agent_registry,
            };

            match action {
                MasterAction::Start { port } => {
                    pupoxide::interface::server::start_master(state, port).await?;
                }
                MasterAction::Sign { node } => {
                    // Approve the bootstrap request and sign the certificate
                    let request = state.bootstrap_manager.get_request(&node).await?;

                    if !request.is_pending() {
                        tracing::error!(
                            "Request for node {} is not pending (status: {})",
                            node,
                            request.status
                        );
                        anyhow::bail!(
                            "Request for node {} cannot be signed (status: {})",
                            node,
                            request.status
                        );
                    }

                    // Sign the CSR (365 days validity)
                    let cert_pem = state.ca.sign_csr(&node, 365)?;

                    // Approve request
                    state.bootstrap_manager.approve_request(&node).await?;

                    // Register the agent
                    state
                        .agent_registry
                        .register(&node, &node, cert_pem)
                        .await?;

                    tracing::info!("Successfully signed and registered node: {}", node);
                    println!("✓ Node '{}' has been approved and registered", node);
                }
                MasterAction::Reject { node } => {
                    // Reject the bootstrap request
                    let request = state.bootstrap_manager.get_request(&node).await?;

                    if !request.is_pending() {
                        tracing::error!(
                            "Request for node {} is not pending (status: {})",
                            node,
                            request.status
                        );
                        anyhow::bail!(
                            "Request for node {} cannot be rejected (status: {})",
                            node,
                            request.status
                        );
                    }

                    state.bootstrap_manager.reject_request(&node).await?;

                    tracing::info!("Rejected bootstrap request for node: {}", node);
                    println!("✓ Request for node '{}' has been rejected", node);
                }
                MasterAction::List => {
                    // List all pending bootstrap requests
                    let requests = state.bootstrap_manager.list_pending_requests().await?;

                    if requests.is_empty() {
                        println!("No pending bootstrap requests");
                    } else {
                        println!("\nPending Bootstrap Requests:");
                        println!("{:-<60}", "");
                        println!("{:<20} {:<20} {:<10}", "Node ID", "Requested At", "Status");
                        println!("{:-<60}", "");

                        for req in requests {
                            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(
                                req.requested_at,
                                0,
                            )
                            .unwrap_or_default();
                            println!(
                                "{:<20} {:<20} {:<10}",
                                req.node_id,
                                dt.format("%Y-%m-%d %H:%M:%S"),
                                req.status
                            );
                        }
                    }
                }
            }
        }
        Commands::Agent {
            server,
            node,
            environment,
            bootstrap,
            check,
            check_timeout,
            dry_run,
            cert_dir,
        } => {
            let agent =
                pupoxide::interface::agent::PupoxideAgent::new(server, node, environment, cert_dir);

            if check {
                // Check bootstrap approval status (can be used with or without --bootstrap flag)
                agent.check_bootstrap_status(check_timeout).await?;
            } else if bootstrap {
                // Phase 1: Submit bootstrap request
                agent.bootstrap().await?;
            } else {
                // Phase 2: Regular agent mode
                agent.run(dry_run).await?;
            }
        }
        Commands::Graph {
            file,
            module_path,
            filter,
            max_depth,
            style,
        } => {
            let engine = PupoxideEngine::new(None);

            // Smart module path resolution (same as Run command)
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

            let style = match style.as_str() {
                "mermaid" => pupoxide::interface::graph::GraphStyle::Mermaid,
                _ => pupoxide::interface::graph::GraphStyle::Ascii,
            };

            pupoxide::interface::graph::display_graph(
                &catalog,
                filter.as_deref(),
                max_depth,
                style,
            )?;
        }
    }

    Ok(())
}
