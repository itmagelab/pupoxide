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
    let engine = PupoxideEngine::new();
    let adapter = FsAdapter;

    match cli.command {
        Commands::Apply { environment } => {
            tracing::info!(env = %environment, "Applying environment...");
            let loader = EnvironmentLoader::new(cli.config);
            let manifest_path = loader.get_site_manifest(&environment)?;
            let resources = engine.run_manifest(manifest_path)?;
            
            for resource in resources {
                adapter.apply(&resource).await?;
            }
        }
        Commands::Run { file } => {
            tracing::info!(file = %file.display(), "Running manifest...");
            let resources = engine.run_manifest(file)?;
            for resource in resources {
                adapter.apply(&resource).await?;
            }
        }
    }

    Ok(())
}
