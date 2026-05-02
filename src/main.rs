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
        Commands::Scan { path } => {
            println!("Scanning: {:?}", path);
            let files = scanner::scan_dir(path);
            println!("Found {} files.", files.len());
            
            let duplicates = duplicate::find_duplicates(files);
            reporter::report(&duplicates);
        }
    }

    Ok(())
}
