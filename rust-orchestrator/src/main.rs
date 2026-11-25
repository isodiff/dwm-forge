use clap::Parser;
use dwm_forge::cli::{Cli, Commands};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch(_) => {
            // Stub implementation
            println!("Fetching dwm...");
            Ok(())
        }
        Commands::Build => {
            // Stub implementation
            println!("Building dwm...");
            Ok(())
        }
        Commands::Package => {
            // Stub implementation
            println!("Packaging dwm...");
            Ok(())
        }
        Commands::SyncGentoo => {
            // Stub implementation
            println!("Syncing with Gentoo...");
            Ok(())
        }
    }
}