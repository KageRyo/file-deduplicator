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
            // TODO: implement scanning
        }
    }

    Ok(())
}
