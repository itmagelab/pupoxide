#![deny(clippy::unwrap_used)]
use anyhow::Result;
use clap::Parser;

use pupoxide::interface::{Cli, Commands};
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

    match cli.command {
        Commands::Run {
            file,
            module_path,
            dry_run,
        } => {
            pupoxide::interface::handlers::handle_run(file, module_path, dry_run).await?;
        }
        Commands::Apply {
            environment,
            dry_run,
        } => {
            pupoxide::interface::handlers::handle_apply(environment, dry_run, cli.config).await?;
        }
        Commands::Master { action, config } => {
            pupoxide::interface::handlers::handle_master(action, config, cli.config).await?;
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
            pupoxide::interface::handlers::handle_agent(
                pupoxide::interface::handlers::AgentOptions {
                    server,
                    node,
                    environment,
                    bootstrap,
                    check,
                    check_timeout,
                    dry_run,
                    cert_dir,
                },
            )
            .await?;
        }
        Commands::Graph {
            file,
            module_path,
            filter,
            max_depth,
            style,
        } => {
            pupoxide::interface::handlers::handle_graph(
                file,
                module_path,
                filter,
                max_depth,
                style,
            )
            .await?;
        }
    }

    Ok(())
}
