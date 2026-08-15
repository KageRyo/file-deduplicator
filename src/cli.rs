use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "dedup")]
#[command(version)]
#[command(about = "Find and safely manage duplicate files", long_about = None)]
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

        /// Show planned moves without changing the filesystem
        #[arg(long)]
        dry_run: bool,

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

#[derive(clap::ValueEnum, Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeepPolicy {
    /// Keep the lexicographically smallest path in each duplicate group
    First,
    /// Keep the file with the newest modification time
    Newest,
    /// Keep the file with the oldest modification time
    Oldest,
}
