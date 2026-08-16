use clap::{Parser, Subcommand};
use clap_complete::Shell;
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

        /// Number of hashing worker threads (must be at least 1)
        #[arg(long, value_parser = parse_positive_usize)]
        threads: Option<usize>,
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

        /// Keep policy
        #[arg(long, value_enum, default_value_t = KeepPolicy::First)]
        keep: KeepPolicy,

        /// Minimum file size to consider
        #[arg(long)]
        min_size: Option<String>,

        /// Paths to exclude from scanning
        #[arg(long)]
        exclude: Vec<String>,

        /// Number of hashing worker threads (must be at least 1)
        #[arg(long, value_parser = parse_positive_usize)]
        threads: Option<usize>,
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

        /// Number of hashing worker threads (must be at least 1)
        #[arg(long, value_parser = parse_positive_usize)]
        threads: Option<usize>,
    },

    /// Move duplicate files to the operating system trash or recycle bin
    Trash {
        /// The path to scan
        path: PathBuf,

        /// Perform a dry run (don't move files to the trash)
        #[arg(long)]
        dry_run: bool,

        /// Confirm moving files to the trash (required if not dry run)
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

        /// Number of hashing worker threads (must be at least 1)
        #[arg(long, value_parser = parse_positive_usize)]
        threads: Option<usize>,
    },

    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
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

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let value = value
        .parse::<usize>()
        .map_err(|_| "must be a positive integer".to_string())?;
    if value == 0 {
        return Err("must be at least 1".to_string());
    }
    Ok(value)
}
