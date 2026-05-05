use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "selector4nix", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
    #[arg(long = "config-file", value_name = "PATH")]
    pub config_file: Option<PathBuf>,
    #[arg(long = "log-level", value_name = "LEVEL")]
    pub log_level: Option<String>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start HTTP server (default)
    Serve,
    /// Validate configuration file
    Check,
}
