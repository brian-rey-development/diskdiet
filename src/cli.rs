use crate::model::Category;
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "diskdiet",
    version,
    about = "Your AI agents made your disk fat. Put it on a diet."
)]
pub struct Args {
    /// Show what would be freed without deleting anything
    #[arg(long)]
    pub dry_run: bool,

    /// Print a non-interactive report and exit
    #[arg(long)]
    pub report: bool,

    /// With --report, print JSON instead of a table
    #[arg(long, requires = "report")]
    pub json: bool,

    /// Extra folder to scan for projects and worktrees (repeatable)
    #[arg(long = "root", value_name = "DIR")]
    pub roots: Vec<PathBuf>,

    /// Path to a config file (default: ~/.config/diskdiet/config.toml)
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Only offer these categories, comma separated (for example build-artifacts,downloads)
    #[arg(long, value_name = "CATEGORY", value_delimiter = ',')]
    pub only: Vec<Category>,

    /// Skip the share card at the end
    #[arg(long)]
    pub no_share: bool,
}
