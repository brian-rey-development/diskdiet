mod bulk;

use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::Metadata;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const BLOCK_BYTES: u64 = 512;
const UNIT_STEP: f64 = 1000.0;
const ROUND_UP_LIMIT: f64 = 999.5;
const ONE_DECIMAL_LIMIT: f64 = 100.0;
const HALF_TENTH: f64 = 0.05;
const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
const FIRST_DECIMAL_UNIT: usize = 2;

type InodeKey = (u64, u64);

enum Node {
    Dir(PathBuf),
    File {
        bytes: u64,
        shared: Option<InodeKey>,
    },
}

#[derive(Default)]
struct Tally {
    bytes: u64,
    shared: HashMap<InodeKey, u64>,
}

impl Tally {
    fn add(&mut self, bytes: u64, shared: Option<InodeKey>) {
        match shared {
            Some(key) => {
                self.shared.insert(key, bytes);
            }
            None => self.bytes += bytes,
        }
    }

    fn merge(mut self, other: Tally) -> Tally {
        self.bytes += other.bytes;
        self.shared.extend(other.shared);
        self
    }

    fn total(&self) -> u64 {
        self.bytes + self.shared.values().sum::<u64>()
    }
}

fn node_of(path: PathBuf, meta: &Metadata) -> Node {
    if meta.is_dir() {
        return Node::Dir(path);
    }
    let shared = (meta.nlink() > 1).then_some((meta.dev(), meta.ino()));
    Node::File {
        bytes: meta.blocks() * BLOCK_BYTES,
        shared,
    }
}

fn read_dir_nodes(dir: &Path) -> io::Result<Vec<Node>> {
    let nodes = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let meta = entry.path().symlink_metadata().ok()?;
            Some(node_of(entry.path(), &meta))
        })
        .collect();
    Ok(nodes)
}

fn children(dir: &Path) -> Vec<Node> {
    bulk::list(dir)
        .or_else(|_| read_dir_nodes(dir))
        .unwrap_or_default()
}

fn tally_dir(dir: &Path) -> Tally {
    let mut own = Tally::default();
    let mut subdirs = Vec::new();
    for node in children(dir) {
        match node {
            Node::Dir(path) => subdirs.push(path),
            Node::File { bytes, shared } => own.add(bytes, shared),
        }
    }
    subdirs
        .par_iter()
        .map(|sub| tally_dir(sub))
        .reduce(Tally::default, Tally::merge)
        .merge(own)
}

fn tally_root(path: &Path) -> Tally {
    let Ok(meta) = path.symlink_metadata() else {
        return Tally::default();
    };
    match node_of(path.to_path_buf(), &meta) {
        Node::Dir(dir) => tally_dir(&dir),
        Node::File { bytes, shared } => {
            let mut tally = Tally::default();
            tally.add(bytes, shared);
            tally
        }
    }
}

pub fn disk_usage(paths: &[PathBuf]) -> u64 {
    paths
        .par_iter()
        .map(|path| tally_root(path))
        .reduce(Tally::default, Tally::merge)
        .total()
}

fn scale(bytes: u64) -> (f64, usize) {
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= ROUND_UP_LIMIT && unit < UNITS.len() - 1 {
        value /= UNIT_STEP;
        unit += 1;
    }
    (value, unit)
}

pub fn format_bytes(bytes: u64) -> String {
    let (value, unit) = scale(bytes);
    if unit < FIRST_DECIMAL_UNIT || value >= ONE_DECIMAL_LIMIT - HALF_TENTH {
        return format!("{value:.0} {}", UNITS[unit]);
    }
    format!("{value:.1} {}", UNITS[unit])
}
