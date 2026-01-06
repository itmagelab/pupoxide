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
    },
    /// Apply configuration from an environment locally
    Apply {
        #[arg(short, long)]
        environment: String,
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
    },
}
