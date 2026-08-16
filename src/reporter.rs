use crate::duplicate::DuplicateGroup;
use crate::duplicate::DuplicateResult;
use crate::scanner::ScanReport;

pub fn report(groups: &[DuplicateGroup]) {
    let mut total_saved_space = 0;

    for (i, group) in groups.iter().enumerate() {
        let saved_space = group.size * (group.files.len() as u64 - 1);
        total_saved_space += saved_space;

        println!("Duplicate group {}:", i + 1);
        println!("  Size: {}", format_size(group.size));
        println!("  Hash: {}", group.hash);
        println!("  Files:");
        for file in &group.files {
            println!("    {}", file.display());
        }
        println!();
    }

    println!("Found {} duplicate groups.", groups.len());
    println!("Potential space saved: {}", format_size(total_saved_space));
}

pub fn report_scan_summary(scan: &ScanReport, duplicates: &DuplicateResult) {
    let summary = scan.summary(
        duplicates.partial_hash_candidates,
        duplicates.candidates_hashed,
        duplicates.hash_errors.len(),
    );
    eprintln!(
        "Scan summary: {} files scanned, {} skipped, {} scan failures, {} partial-hash candidates, {} full-hash candidates, {} hash failures.",
        summary.files_scanned,
        summary.files_skipped,
        summary.scan_failures,
        summary.partial_hash_candidates,
        summary.hash_candidates,
        summary.hash_failures
    );

    for error in &scan.errors {
        let path = error
            .path
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<unknown path>".to_string());
        eprintln!(
            "Scan error [{}] {}: {}",
            error.kind_as_str(),
            path,
            error.message
        );
    }

    for error in &duplicates.hash_errors {
        eprintln!("Hash error {}: {}", error.path.display(), error.message);
    }
}

fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} bytes", size)
    }
}
