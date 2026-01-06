use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use pupoxide::interface::{Cli, Commands};
use pupoxide::application::{PupoxideEngine, EnvironmentLoader};
use pupoxide::infrastructure::FsAdapter;
use pupoxide::domain::resource::ResourceProvider;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    let state_dir = PathBuf::from("/tmp/pupoxide");
    let backup_store = pupoxide::infrastructure::BackupStore::new(state_dir.join("backups"));
    let state_store = pupoxide::infrastructure::StateStore::new(state_dir.join("state"));

    match cli.command {
        Commands::Run { file, module_path } => {
            let engine = PupoxideEngine::new();
            
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
            let catalog = engine.run_manifest(file, "localhost".to_string(), "local".to_string(), facts)?;
            
            execute_catalog_with_transaction(catalog, &backup_store, &state_store).await?;
        }
        Commands::Apply { environment } => {
            let loader = EnvironmentLoader::new(cli.config);
            let manifest_path = loader.get_site_manifest(&environment)?;
            let modules_path = loader.get_modules_path(&environment);
            
            let engine = PupoxideEngine::new();
            let facts = pupoxide::infrastructure::Facter::collect();
            let catalog = engine.run_manifest_with_modules(manifest_path, modules_path, "localhost".to_string(), environment, facts)?;
            
            execute_catalog_with_transaction(catalog, &backup_store, &state_store).await?;
        }
        Commands::Rollback { transaction_id } => {
            let transaction = if let Some(id) = transaction_id {
                state_store.load_transaction(&id)?
            } else {
                state_store.load_latest_transaction()?
            };

            tracing::info!(id = %transaction.id, "Starting rollback");
            
            let rollback_engine = pupoxide::application::RollbackEngine::new(backup_store);
            let rollback_catalog = rollback_engine.generate_rollback_catalog(&transaction);
            
            let adapter = FsAdapter;
            for resource in rollback_catalog.resources {
                tracing::info!(id = %resource.id(), "Rolling back resource");
                adapter.apply(&resource).await?;
            }
            
            tracing::info!("Rollback completed successfully");
        }
        Commands::Master { port } => {
            let loader = EnvironmentLoader::new(cli.config);
            let engine = PupoxideEngine::new();
            let state = pupoxide::interface::server::MasterState { engine, loader };
            pupoxide::interface::server::start_master(state, port).await?;
        }
        Commands::Agent { server, node, environment } => {
            let agent = pupoxide::interface::agent::PupoxideAgent::new(server, node, environment);
            agent.run().await?;
        }
    }

    Ok(())
}

async fn execute_catalog_with_transaction(
    catalog: pupoxide::domain::Catalog,
    backup_store: &pupoxide::infrastructure::BackupStore,
    state_store: &pupoxide::infrastructure::StateStore,
) -> Result<()> {
    let adapter = FsAdapter;
    let transaction_id = format!("tx_{}", chrono::Utc::now().timestamp());
    let mut transaction = pupoxide::domain::Transaction::new(transaction_id.clone(), catalog.clone());

    tracing::info!(id = %transaction_id, "Starting transaction");

    for resource in &catalog.resources {
        // 1. Snapshot original state
        let backup_needed = match resource {
            pupoxide::domain::Resource::File(f) => f.backup,
            pupoxide::domain::Resource::Directory(d) => d.backup,
            _ => false,
        };

        let state = adapter.get_state(resource, backup_needed).await?;
        transaction.original_states.insert(resource.id().to_string(), state.clone());

        // 2. Store backup if it's a file with content
        if let pupoxide::domain::ResourceState::Full { content: Some(bytes), .. } = state {
            let hash = backup_store.store(&bytes)?;
            transaction.backups.insert(resource.id().to_string(), hash);
        }

        // 3. Apply changes
        match adapter.apply(resource).await {
            Ok(_) => {
                transaction.resource_statuses.insert(resource.id().to_string(), pupoxide::domain::RollbackStatus::Success);
            }
            Err(e) => {
                transaction.resource_statuses.insert(resource.id().to_string(), pupoxide::domain::RollbackStatus::Failed(e.to_string()));
                state_store.save_transaction(&transaction)?;
                return Err(e.into());
            }
        }
    }

    state_store.save_transaction(&transaction)?;
    tracing::info!(id = %transaction_id, "Transaction completed and saved");
    Ok(())
}
