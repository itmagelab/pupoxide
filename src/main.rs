use anyhow::Result;
use clap::Parser;
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

    match cli.command {
        Commands::Run { file, module_path } => {
            let engine = PupoxideEngine::new();
            
            // Smart module path resolution
            let resolved_module_path = if let Some(mp) = module_path {
                Some(mp)
            } else {
                // Try to find a sibling 'modules' directory relative to the file
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
            let adapter = FsAdapter;
            for resource in catalog.resources {
                adapter.apply(&resource).await?;
            }
        }
        Commands::Apply { environment } => {
            let loader = EnvironmentLoader::new(cli.config);
            let manifest_path = loader.get_site_manifest(&environment)?;
            let modules_path = loader.get_modules_path(&environment);
            
            let engine = PupoxideEngine::new();
            let facts = pupoxide::infrastructure::Facter::collect();
            let catalog = engine.run_manifest_with_modules(manifest_path, modules_path, "localhost".to_string(), environment, facts)?;
            
            let adapter = FsAdapter;
            for resource in catalog.resources {
                adapter.apply(&resource).await?;
            }
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
