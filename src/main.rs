mod cli;
mod scanner;
mod hasher;
mod duplicate;
mod reporter;

use clap::Parser;
use cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Scan { path, min_size, exclude, json } => {
            let min_size_bytes = min_size.and_then(|s| scanner::parse_size(&s));
            
            if !json {
                println!("Scanning: {:?}", path);
            }

            let files = scanner::scan_dir(path, min_size_bytes, &exclude);
            
            if !json {
                println!("Found {} files.", files.len());
            }
            
            let duplicates = duplicate::find_duplicates(files);

            if json {
                println!("{}", serde_json::to_string_pretty(&duplicates)?);
            } else {
                reporter::report(&duplicates);
            }
        }
    }

    Ok(())
}
