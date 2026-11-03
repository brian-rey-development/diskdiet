use super::ScanContext;
use super::caches::measured;
use crate::fmt::{parse_size, tilde_path};
use crate::model::{Action, Category, Item, Risk};
use crate::process::{DEFAULT_TIMEOUT, run_stdout};
use crate::tools;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

const HF_HUB_DIR: &str = ".cache/huggingface/hub";
const HF_MODEL_PREFIX: &str = "models--";
const HF_SEPARATOR: &str = "--";
const OLLAMA_HEADER: &str = "NAME";
const OLLAMA_MIN_COLUMNS: usize = 4;

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let mut items = ollama_items(ctx);
    items.extend(huggingface_items(ctx));
    items
}

pub fn parse_ollama_list(text: &str) -> Vec<(Vec<String>, u64)> {
    let mut groups: Vec<(String, Vec<String>, u64)> = Vec::new();
    for (name, id, bytes) in text.lines().filter_map(parse_ollama_row) {
        match groups.iter_mut().find(|(known, _, _)| *known == id) {
            Some((_, names, _)) => names.push(name),
            None => groups.push((id, vec![name], bytes)),
        }
    }
    groups
        .into_iter()
        .map(|(_, names, bytes)| (names, bytes))
        .collect()
}

pub fn hf_label(folder: &str) -> Option<String> {
    let rest = folder.strip_prefix(HF_MODEL_PREFIX)?;
    (!rest.is_empty()).then(|| rest.replace(HF_SEPARATOR, "/"))
}

fn parse_ollama_row(line: &str) -> Option<(String, String, u64)> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.len() < OLLAMA_MIN_COLUMNS || columns[0] == OLLAMA_HEADER {
        return None;
    }
    let bytes = parse_size(&format!("{}{}", columns[2], columns[3]))?;
    Some((columns[0].to_string(), columns[1].to_string(), bytes))
}

fn ollama_items(ctx: &ScanContext) -> Vec<Item> {
    let Some(ollama) = ctx.tools.get(tools::OLLAMA) else {
        return Vec::new();
    };
    let Some(listing) = run_stdout(ollama, &["list"], DEFAULT_TIMEOUT) else {
        return Vec::new();
    };
    parse_ollama_list(&listing)
        .into_iter()
        .map(|(names, bytes)| ollama_item(ollama, &names, bytes))
        .collect()
}

fn ollama_item(ollama: &Path, names: &[String], bytes: u64) -> Item {
    let mut args = vec!["rm".to_string()];
    args.extend(names.iter().cloned());
    let action = Action::RunCommand {
        program: ollama.to_path_buf(),
        args,
    };
    let name = names.join(", ");
    let first = names.first().map_or("", String::as_str);
    let detail = format!("Ollama model · re-download with `ollama pull {first}`");
    Item::new(
        Category::AiModels,
        name,
        detail,
        bytes,
        Risk::Review,
        action,
    )
}

fn huggingface_items(ctx: &ScanContext) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(ctx.home.join(HF_HUB_DIR)) else {
        return Vec::new();
    };
    let folders: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    folders
        .into_par_iter()
        .filter_map(|path| huggingface_item(ctx, path))
        .collect()
}

fn huggingface_item(ctx: &ScanContext, path: PathBuf) -> Option<Item> {
    let label = hf_label(&path.file_name()?.to_string_lossy())?;
    let bytes = measured(&path)?;
    let detail = format!("Hugging Face model · {}", tilde_path(&path, &ctx.home));
    let action = Action::DeletePath(path);
    Some(Item::new(
        Category::AiModels,
        label,
        detail,
        bytes,
        Risk::Review,
        action,
    ))
}
