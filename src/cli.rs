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

    /// Move duplicate files to a specific directory
    Move {
        /// The path to scan
        path: PathBuf,

        /// Destination directory for duplicates
        #[arg(short, long)]
        to: PathBuf,

        /// Minimum file size to consider
        #[arg(long)]
        min_size: Option<String>,

        /// Paths to exclude from scanning
        #[arg(long)]
        exclude: Vec<String>,
    },

    /// Delete duplicate files
    Delete {
        /// The path to scan
        path: PathBuf,

        /// Perform a dry run (don't actually delete)
        #[arg(long)]
        dry_run: bool,

        /// Confirm deletion (required if not dry run)
        #[arg(long)]
        confirm: bool,

        /// Keep policy
        #[arg(long, value_enum, default_value_t = KeepPolicy::First)]
        keep: KeepPolicy,

        /// Minimum file size to consider
        #[arg(long)]
        min_size: Option<String>,

        /// Paths to exclude from scanning
        #[arg(long)]
        exclude: Vec<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum KeepPolicy {
    First,
    Newest,
    Oldest,
}
