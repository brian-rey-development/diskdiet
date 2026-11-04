use std::fs;
use std::path::{Path, PathBuf};

const LAYER_RULES: [(&str, &[&str]); 4] = [
    ("infra", &["crate::domain", "crate::features", "crate::app"]),
    ("domain", &["crate::features", "crate::app"]),
    (
        "features/scan",
        &["crate::features::actions", "super::actions", "crate::app"],
    ),
    (
        "features/actions",
        &["crate::features::scan", "super::scan", "crate::app"],
    ),
];
const ESCAPES: [&str; 5] = [
    "crate::{",
    "crate::features::{",
    "crate::features;",
    "crate::features as",
    "super::super",
];

fn source_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    entries
        .flatten()
        .map(|entry| entry.path())
        .flat_map(|path| match path.is_dir() {
            true => rust_files(&path),
            false if path.extension().is_some_and(|ext| ext == "rs") => vec![path],
            false => Vec::new(),
        })
        .collect()
}

fn offenders(dir: &Path, forbidden: &[&str]) -> Vec<String> {
    rust_files(dir)
        .into_iter()
        .flat_map(|file| {
            let source = fs::read_to_string(&file).unwrap_or_default();
            forbidden
                .iter()
                .filter(|pattern| source.contains(**pattern))
                .map(|pattern| format!("{} uses {pattern}", file.display()))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn lower_layers_never_import_higher_ones() {
    let violations: Vec<String> = LAYER_RULES
        .iter()
        .flat_map(|(layer, forbidden)| offenders(&source_root().join(layer), forbidden))
        .collect();
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn imports_name_full_paths_so_the_layer_check_sees_them() {
    let violations = offenders(&source_root(), &ESCAPES);
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn every_layer_folder_exists() {
    let missing: Vec<&str> = LAYER_RULES
        .iter()
        .map(|(layer, _)| *layer)
        .filter(|layer| !source_root().join(layer).is_dir())
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}
