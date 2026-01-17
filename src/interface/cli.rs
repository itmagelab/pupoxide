use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Path to the pupoxide configuration directory (default: /etc/pupoxide)
    #[arg(short, long, default_value = "/etc/pupoxide")]
    pub config: PathBuf,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Apply a single manifest file locally
    Run {
        #[arg(short, long)]
        file: PathBuf,

        /// Path to the modules directory (optional)
        #[arg(short, long)]
        module_path: Option<PathBuf>,

        /// Run in dry-run mode without making changes
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },
    /// Apply configuration from an environment locally
    Apply {
        #[arg(short, long)]
        environment: String,

        /// Run in dry-run mode without making changes
        #[arg(long, default_value = "false")]
        dry_run: bool,
    },
    /// Start the Pupoxide Master server
    Master {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Start the Pupoxide Agent
    Agent {
        #[arg(short, long)]
        server: String,
        #[arg(short, long)]
        node: String,
        #[arg(short, long)]
        environment: String,

        /// Bootstrap agent with the master (generate certificate and register)
        #[arg(long, default_value = "false")]
        bootstrap: bool,

        /// Bootstrap token (required when using --bootstrap)
        #[arg(long)]
        token: Option<String>,

        /// Run in dry-run mode without making changes
        #[arg(long, default_value = "false")]
        dry_run: bool,

        /// Optional certificate directory
        #[arg(short, long)]
        cert_dir: Option<PathBuf>,
    },
}
