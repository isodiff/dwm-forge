use clap::Parser;
use dwm_forge::build;
use dwm_forge::cli::{Cli, Commands};
use dwm_forge::fetch;
use dwm_forge::package;
use dwm_forge::sync_gentoo;

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Fetch(args) => fetch::run_fetch(&cli.repo_root, &args),
        Commands::Build(args) => build::run_build(&cli.repo_root, &args),
        Commands::Package(args) => package::run_package(&cli.repo_root, &args),
        Commands::SyncGentoo(args) => sync_gentoo::run_sync_gentoo(&args),
    }
}
