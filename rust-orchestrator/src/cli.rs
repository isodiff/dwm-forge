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
    Build(BuildArgs),
    Package(PackageArgs),
    SyncGentoo(SyncGentooArgs),
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum FetchSource {
    Git,
    Tarball,
}

#[derive(Args, Debug, Clone)]
pub struct BuildArgs {
    #[arg(long)]
    pub work_dir: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    pub skip_hash_check: bool,
    #[arg(long, default_value_t = false)]
    pub patch_only: bool,
    #[arg(long)]
    pub c_test_dir: Option<PathBuf>,
}

#[derive(Args, Debug, Clone)]
pub struct PackageArgs {
    #[arg(long)]
    pub staging_dir: PathBuf,
    #[arg(long, default_value = "dist")]
    pub out_dir: PathBuf,
    #[arg(long, default_value = "local")]
    pub sha: String,
}

#[derive(Args, Debug, Clone)]
pub struct SyncGentooArgs {
    #[arg(long, default_value = "isodiff/dwm-forge")]
    pub repo: String,
    #[arg(long, default_value = "https://api.github.com")]
    pub api_base: String,
    #[arg(long, default_value = "/etc/portage/savedconfig/x11-wm/dwm")]
    pub savedconfig_path: PathBuf,
    #[arg(long, default_value = "/var/cache/distfiles")]
    pub distdir: PathBuf,
    #[arg(
        long,
        default_value = "/var/db/repos/dwm-forge-local/x11-wm/dwm/dwm-6.5-r100.ebuild"
    )]
    pub ebuild_path: PathBuf,
    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Args, Debug)]
pub struct FetchArgs {
    #[arg(long, value_enum, default_value_t = FetchSource::Git)]
    pub source: FetchSource,
    #[arg(long, default_value_t = false)]
    pub update_lock: bool,
}
