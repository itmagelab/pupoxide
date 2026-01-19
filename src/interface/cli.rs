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
    /// Manage the Pupoxide Master server
    Master {
        #[command(subcommand)]
        action: MasterAction,

        /// Path to the configuration directory
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    /// Start the Pupoxide Agent
    Agent {
        #[arg(short, long)]
        server: String,
        #[arg(short, long)]
        node: String,
        #[arg(short, long)]
        environment: String,

        /// Bootstrap agent with the master (submit CSR request)
        #[arg(long, default_value = "false")]
        bootstrap: bool,

        /// Check bootstrap approval status (polls until approved)
        #[arg(long, default_value = "false")]
        check: bool,

        /// Timeout for --check in seconds
        #[arg(long, default_value = "600")]
        check_timeout: u64,

        /// Run in dry-run mode without making changes
        #[arg(long, default_value = "false")]
        dry_run: bool,

        /// Optional certificate directory
        #[arg(short, long)]
        cert_dir: Option<PathBuf>,
    },
    /// Visualize resource dependency graph for debugging
    Graph {
        /// Path to the manifest file
        #[arg(short, long)]
        file: PathBuf,

        /// Path to modules directory
        #[arg(short, long)]
        module_path: Option<PathBuf>,

        /// Show only specific resource types (file, directory, exec, meta)
        #[arg(long, value_delimiter = ',')]
        filter: Option<Vec<String>>,

        /// Maximum depth to display
        #[arg(long, default_value = "10")]
        max_depth: usize,

        /// Output style (ascii, mermaid)
        #[arg(short, long, default_value = "ascii")]
        style: String,
    },
}

#[derive(Subcommand)]
pub enum MasterAction {
    /// Start the Master server
    Start {
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
    /// Sign certificate for a pending bootstrap request
    Sign {
        /// Node ID to approve
        #[arg(short, long)]
        node: String,
    },
    /// Reject a pending bootstrap request
    Reject {
        /// Node ID to reject
        #[arg(short, long)]
        node: String,
    },
    /// List pending bootstrap requests
    List,
}
