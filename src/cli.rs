use crate::model::{Category, RunMode};
use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[allow(clippy::struct_excessive_bools)]
#[command(
    name = "diskdiet",
    version,
    about = "Your AI agents made your disk fat. Put it on a diet."
)]
pub struct Args {
    #[arg(long, help = "Show what would be freed without deleting anything")]
    pub dry_run: bool,

    #[arg(long, help = "Print a non-interactive report and exit")]
    pub report: bool,

    #[arg(
        long,
        requires = "report",
        help = "With --report, print JSON instead of a table"
    )]
    pub json: bool,

    #[arg(
        long = "root",
        value_name = "DIR",
        help = "Extra folder to scan for projects and worktrees (repeatable)"
    )]
    pub roots: Vec<PathBuf>,

    #[arg(
        long,
        value_name = "FILE",
        help = "Path to a config file (default: ~/.config/diskdiet/config.toml)"
    )]
    pub config: Option<PathBuf>,

    #[arg(
        long,
        value_name = "CATEGORY",
        value_delimiter = ',',
        help = "Only offer these categories, comma separated (for example build-artifacts,downloads)"
    )]
    pub only: Vec<Category>,

    #[arg(long, help = "Skip the share card at the end")]
    pub no_share: bool,
}

impl Args {
    pub(crate) fn run_mode(&self) -> RunMode {
        if self.dry_run {
            RunMode::DryRun
        } else {
            RunMode::Live
        }
    }
}
