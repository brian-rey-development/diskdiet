use anyhow::Context;
use diskdiet::{
    config,
    platform::{NO_HOME, home_dir},
    scan::{SCANNERS, ScanContext},
    tools::Tools,
};
use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let home = home_dir().context(NO_HOME)?;
    let started = Instant::now();
    let tools = Tools::discover();
    println!("tools {:?}", started.elapsed());
    let ctx = ScanContext {
        config: config::load(None, &home)?,
        home,
        tools,
    };
    for scanner in SCANNERS {
        let started = Instant::now();
        let item_count = (scanner.scan)(&ctx).len();
        println!(
            "{:10} {:>8.2?} items={item_count}",
            scanner.name,
            started.elapsed()
        );
    }
    Ok(())
}
