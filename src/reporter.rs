use crate::duplicate::DuplicateGroup;

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
