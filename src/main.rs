mod cleanup;
mod cli;
mod duplicate;
mod hasher;
mod reporter;
mod scanner;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Commands};
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use std::path::PathBuf;
use std::process::ExitCode;

const EXIT_SUCCESS: u8 = 0;
const EXIT_GENERAL_ERROR: u8 = 1;
const EXIT_INCOMPLETE_SCAN: u8 = 2;
const EXIT_PARTIAL_CLEANUP: u8 = 3;

struct ScanOutcome {
    scan: scanner::ScanReport,
    duplicates: duplicate::DuplicateResult,
}

#[derive(Serialize)]
struct JsonScanOutput<'a> {
    summary: scanner::ScanSummary,
    duplicate_groups: &'a [duplicate::DuplicateGroup],
    scan_errors: &'a [scanner::ScanError],
    hash_errors: &'a [duplicate::HashError],
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::from(EXIT_GENERAL_ERROR)
        }
    }
}

fn run() -> Result<u8> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan {
            path,
            min_size,
            exclude,
            json,
        } => {
            let outcome = run_scan(path, min_size, exclude, json)?;
            if json {
                let output = JsonScanOutput {
                    summary: outcome.scan.summary(
                        outcome.duplicates.candidates_hashed,
                        outcome.duplicates.hash_errors.len(),
                    ),
                    duplicate_groups: &outcome.duplicates.groups,
                    scan_errors: &outcome.scan.errors,
                    hash_errors: &outcome.duplicates.hash_errors,
                };
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                reporter::report(&outcome.duplicates.groups);
            }

            Ok(
                if outcome.scan.is_complete() && outcome.duplicates.is_complete() {
                    EXIT_SUCCESS
                } else {
                    EXIT_INCOMPLETE_SCAN
                },
            )
        }
        Commands::Move {
            path,
            to,
            dry_run,
            keep,
            min_size,
            exclude,
        } => {
            let outcome = run_scan(path, min_size, exclude, false)?;
            if !ensure_complete(&outcome, "move") {
                return Ok(EXIT_INCOMPLETE_SCAN);
            }
            if outcome.duplicates.groups.is_empty() {
                println!("No duplicates found.");
            } else {
                let report =
                    cleanup::move_duplicates(outcome.duplicates.groups, &to, keep, dry_run)?;
                return Ok(finish_cleanup_report(report, "move"));
            }

            Ok(EXIT_SUCCESS)
        }
        Commands::Delete {
            path,
            dry_run,
            confirm,
            keep,
            min_size,
            exclude,
        } => {
            if !dry_run && !confirm {
                anyhow::bail!(
                    "You must use --confirm to delete files, or use --dry-run to see what would happen."
                );
            }
            let outcome = run_scan(path, min_size, exclude, false)?;
            if !ensure_complete(&outcome, "delete") {
                return Ok(EXIT_INCOMPLETE_SCAN);
            }
            if outcome.duplicates.groups.is_empty() {
                println!("No duplicates found.");
            } else {
                let report = cleanup::delete_duplicates(outcome.duplicates.groups, keep, dry_run)?;
                return Ok(finish_cleanup_report(report, "delete"));
            }

            Ok(EXIT_SUCCESS)
        }
    }
}

fn run_scan(
    path: PathBuf,
    min_size: Option<String>,
    exclude: Vec<String>,
    quiet: bool,
) -> Result<ScanOutcome> {
    let min_size_bytes = match min_size {
        Some(value) => Some(
            scanner::parse_size(&value)
                .ok_or_else(|| anyhow::anyhow!("Invalid size format: {value}"))?,
        ),
        None => None,
    };

    if !quiet {
        eprintln!("Scanning: {:?}", path);
    }

    let scan = scanner::scan_dir(path, min_size_bytes, &exclude)?;

    let progress = if quiet {
        None
    } else {
        let progress = ProgressBar::new(0);
        progress.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
                )?
                .progress_chars("#>-"),
        );
        Some(progress)
    };

    let duplicates = duplicate::find_duplicates(scan.files.clone(), progress.as_ref());

    if let Some(progress) = progress {
        progress.finish_and_clear();
    }

    if !quiet {
        reporter::report_scan_summary(&scan, &duplicates);
    }

    Ok(ScanOutcome { scan, duplicates })
}

fn ensure_complete(outcome: &ScanOutcome, operation: &str) -> bool {
    if outcome.scan.is_complete() && outcome.duplicates.is_complete() {
        return true;
    }

    eprintln!(
        "Refusing to {operation}: scan is incomplete ({} scan failures, {} hash failures); no destructive operation was started.",
        outcome.scan.errors.len(),
        outcome.duplicates.hash_errors.len()
    );
    false
}

fn finish_cleanup_report(report: cleanup::CleanupReport, operation: &str) -> u8 {
    if report.has_skipped_files() {
        eprintln!(
            "{operation} skipped {} file(s) after revalidation; changed files were not removed or moved.",
            report.skipped.len()
        );
        return EXIT_PARTIAL_CLEANUP;
    }
    EXIT_SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn partial_cleanup_uses_a_distinct_exit_code() {
        let report = cleanup::CleanupReport {
            completed: 1,
            skipped: vec![cleanup::SkippedOperation {
                path: PathBuf::from("changed.txt"),
                reason: "file changed since scan".to_string(),
            }],
        };

        assert_eq!(
            finish_cleanup_report(report, "delete"),
            EXIT_PARTIAL_CLEANUP
        );
    }
}
