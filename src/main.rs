use clap::Parser;
use diskdiet::app::{cli::Args, session, ui};

fn main() -> anyhow::Result<()> {
    session::init_thread_pool();
    ui::run(&Args::parse())
}
