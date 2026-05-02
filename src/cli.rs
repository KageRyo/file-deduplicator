use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dedup")]
#[command(about = "A safe and fast Rust CLI tool for finding duplicate files", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Scan a directory for duplicate files
    Scan {
        /// The path to scan
        path: PathBuf,

        /// Minimum file size to consider (e.g., 1MB, 500KB)
        #[arg(long)]
        min_size: Option<String>,

        /// Paths to exclude from scanning
        #[arg(long)]
        exclude: Vec<String>,

        /// Output results in JSON format
        #[arg(long)]
        json: bool,
    },
}
