use clap::Parser;
use diskdiet::{cli::Args, ui};

fn main() -> anyhow::Result<()> {
    ui::run(Args::parse())
}
