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
    /// Applies a manifest from a specific environment
    Apply {
        /// The environment to use (e.g., production, staging)
        #[arg(short, long)]
        environment: String,
    },
    /// Executes a single rhai manifest file
    Run {
        /// Path to the rhai script
        file: PathBuf,
    },
}
