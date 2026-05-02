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
    },
}
