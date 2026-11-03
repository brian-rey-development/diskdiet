use crate::process::{DEFAULT_TIMEOUT, run_stdout, stdout_of};
use crate::time::parse_timestamp;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

const BUNDLE_ID_ATTRIBUTE: &str = "kMDItemCFBundleIdentifier";
const LAST_USED_ATTRIBUTE: &str = "kMDItemLastUsedDate";
const MDLS_RAW_FLAG: &str = "-raw";
const MDLS_NAME_FLAG: &str = "-name";
const MDLS_NULL: &str = "(null)";
const MDLS_FIELD_SEPARATOR: char = '\0';
const MDLS_FIELDS_PER_APP: usize = 2;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Spotlight {
    pub bundle_id: Option<String>,
    pub last_used: Option<i64>,
}

pub fn parse_mdls_batch(raw: &str, app_count: usize) -> Option<Vec<Spotlight>> {
    let fields: Vec<&str> = raw.split(MDLS_FIELD_SEPARATOR).collect();
    if fields.len() != app_count * MDLS_FIELDS_PER_APP {
        return None;
    }
    let (pairs, _) = fields.as_chunks::<MDLS_FIELDS_PER_APP>();
    Some(pairs.iter().map(spotlight_of).collect())
}

pub(super) fn spotlight_records(mdls: &Path, apps: &[PathBuf]) -> Vec<Spotlight> {
    batched_spotlight(mdls, apps).unwrap_or_else(|| {
        apps.par_iter()
            .map(|app| single_spotlight(mdls, app))
            .collect()
    })
}

fn spotlight_of(pair: &[&str; MDLS_FIELDS_PER_APP]) -> Spotlight {
    let [bundle_id, last_used] = pair;
    Spotlight {
        bundle_id: mdls_text(bundle_id),
        last_used: mdls_text(last_used).and_then(|raw| parse_timestamp(&raw)),
    }
}

fn mdls_text(raw: &str) -> Option<String> {
    let value = raw.trim();
    (!value.is_empty() && value != MDLS_NULL).then(|| value.to_owned())
}

fn batched_spotlight(mdls: &Path, apps: &[PathBuf]) -> Option<Vec<Spotlight>> {
    if apps.is_empty() {
        return Some(Vec::new());
    }
    let paths = apps
        .iter()
        .map(|app| app.to_str())
        .collect::<Option<Vec<&str>>>()?;
    let mut command = Command::new(mdls);
    command
        .args([MDLS_RAW_FLAG, MDLS_NAME_FLAG, BUNDLE_ID_ATTRIBUTE])
        .args([MDLS_NAME_FLAG, LAST_USED_ATTRIBUTE])
        .args(paths);
    parse_mdls_batch(&stdout_of(&mut command, DEFAULT_TIMEOUT)?, apps.len())
}

fn single_spotlight(mdls: &Path, app: &Path) -> Spotlight {
    Spotlight {
        bundle_id: mdls_value(mdls, BUNDLE_ID_ATTRIBUTE, app),
        last_used: mdls_value(mdls, LAST_USED_ATTRIBUTE, app).and_then(|raw| parse_timestamp(&raw)),
    }
}

fn mdls_value(mdls: &Path, attribute: &str, path: &Path) -> Option<String> {
    let args = [MDLS_RAW_FLAG, MDLS_NAME_FLAG, attribute, path.to_str()?];
    mdls_text(&run_stdout(mdls, &args, DEFAULT_TIMEOUT)?)
}
