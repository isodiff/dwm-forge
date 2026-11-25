use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Cli {
    #[arg(long, default_value = ".")]
    pub repo_root: PathBuf,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Fetch(FetchArgs),
    Build,
    Package,
    SyncGentoo,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum FetchSource {
    Git,
    Tarball,
}

#[derive(Args, Debug)]
pub struct FetchArgs {
    #[arg(long, value_enum, default_value_t = FetchSource::Git)]
    pub source: FetchSource,
    #[arg(long, default_value_t = false)]
    pub update_lock: bool,
}