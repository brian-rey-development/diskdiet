use diskdiet::{
    config,
    scan::{SCANNERS, ScanContext},
    tools::Tools,
};
use std::{path::PathBuf, time::Instant};

fn main() -> anyhow::Result<()> {
    let home = PathBuf::from(std::env::var("HOME")?);
    let t = Instant::now();
    let tools = Tools::discover();
    println!("tools {:?}", t.elapsed());
    let ctx = ScanContext {
        config: config::load(None, &home)?,
        home,
        tools,
    };
    for (name, scan) in SCANNERS {
        let t = Instant::now();
        let n = scan(&ctx).len();
        println!("{name:10} {:>8.2?} items={n}", t.elapsed());
    }
    Ok(())
}
