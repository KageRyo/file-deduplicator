use file_deduplicator::cli::{Cli, Commands};
use file_deduplicator::duplicate;
use file_deduplicator::reporter;
use file_deduplicator::scanner;
use file_deduplicator::cleanup;

use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, min_size, exclude, json } => {
            let duplicates = run_scan(path, min_size, exclude, json)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&duplicates)?);
            } else {
                reporter::report(&duplicates);
            }
        }
        Commands::Move { path, to, min_size, exclude } => {
            let duplicates = run_scan(path, min_size, exclude, false)?;
            if duplicates.is_empty() {
                println!("No duplicates found.");
            } else {
                cleanup::move_duplicates(duplicates, &to)?;
            }
        }
        Commands::Delete { path, dry_run, confirm, keep, min_size, exclude } => {
            if !dry_run && !confirm {
                anyhow::bail!("You must use --confirm to delete files, or use --dry-run to see what would happen.");
            }
            let duplicates = run_scan(path, min_size, exclude, false)?;
            if duplicates.is_empty() {
                println!("No duplicates found.");
            } else {
                cleanup::delete_duplicates(duplicates, keep, dry_run)?;
            }
        }
    }

    Ok(())
}

fn run_scan(path: PathBuf, min_size: Option<String>, exclude: Vec<String>, quiet: bool) -> anyhow::Result<Vec<duplicate::DuplicateGroup>> {
    let min_size_bytes = min_size.and_then(|s| scanner::parse_size(&s));
    
    if !quiet {
        println!("Scanning: {:?}", path);
    }

    let files = scanner::scan_dir(path, min_size_bytes, &exclude);
    
    if !quiet {
        println!("Found {} files.", files.len());
    }

    let pb = if !quiet {
        let pb = ProgressBar::new(0);
        pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
            .progress_chars("#>-"));
        Some(pb)
    } else {
        None
    };
    
    let duplicates = duplicate::find_duplicates(files, pb.as_ref());

    if let Some(pb) = pb {
        pb.finish_and_clear();
    }

    Ok(duplicates)
}
