use super::ScanContext;
use super::support::{file_label, located_detail, measured};
use crate::domain::model::{Action, Category, Item, Risk};
use crate::infra::sys::process::{DEFAULT_TIMEOUT, run_stdout};
use crate::infra::sys::tools;
use crate::infra::util::fmt::parse_size;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

const HF_HUB_DIR: &str = ".cache/huggingface/hub";
const HF_MODEL_PREFIX: &str = "models--";
const HF_SEPARATOR: &str = "--";
const OLLAMA_HEADER: &str = "NAME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OllamaModel {
    pub tags: Vec<String>,
    pub bytes: u64,
}

struct OllamaRow {
    tag: String,
    id: String,
    bytes: u64,
}

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let mut items = ollama_items(ctx);
    items.extend(huggingface_items(ctx));
    items
}

pub fn parse_ollama_list(text: &str) -> Vec<OllamaModel> {
    let mut grouped: Vec<(String, OllamaModel)> = Vec::new();
    for row in text.lines().filter_map(parse_ollama_row) {
        match grouped.iter_mut().find(|(known_id, _)| *known_id == row.id) {
            Some((_, model)) => model.tags.push(row.tag),
            None => grouped.push((
                row.id,
                OllamaModel {
                    tags: vec![row.tag],
                    bytes: row.bytes,
                },
            )),
        }
    }
    grouped.into_iter().map(|(_, model)| model).collect()
}

pub fn hf_label(folder: &str) -> Option<String> {
    let rest = folder.strip_prefix(HF_MODEL_PREFIX)?;
    (!rest.is_empty()).then(|| rest.replace(HF_SEPARATOR, "/"))
}

fn parse_ollama_row(line: &str) -> Option<OllamaRow> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    let [tag, id, amount, unit, ..] = columns.as_slice() else {
        return None;
    };
    if *tag == OLLAMA_HEADER {
        return None;
    }
    Some(OllamaRow {
        tag: (*tag).to_owned(),
        id: (*id).to_owned(),
        bytes: parse_size(&format!("{amount}{unit}"))?,
    })
}

fn ollama_items(ctx: &ScanContext) -> Vec<Item> {
    let Some(ollama) = ctx.tools.get(tools::OLLAMA) else {
        return Vec::new();
    };
    let Some(listing) = run_stdout(ollama, &["list"], DEFAULT_TIMEOUT) else {
        return Vec::new();
    };
    parse_ollama_list(&listing)
        .iter()
        .map(|model| ollama_item(ollama, model))
        .collect()
}

fn ollama_item(ollama: &Path, model: &OllamaModel) -> Item {
    let mut args = vec!["rm".to_owned()];
    args.extend(model.tags.iter().cloned());
    let action = Action::RunCommand {
        program: ollama.to_path_buf(),
        args,
    };
    let first_tag = model.tags.first().map_or("", String::as_str);
    let detail = format!("Ollama model · re-download with `ollama pull {first_tag}`");
    Item::new(
        Category::AiModels,
        model.tags.join(", "),
        detail,
        model.bytes,
        Risk::Review,
        action,
    )
}

fn huggingface_items(ctx: &ScanContext) -> Vec<Item> {
    let Ok(entries) = fs::read_dir(ctx.home.join(HF_HUB_DIR)) else {
        return Vec::new();
    };
    let folders: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    folders
        .into_par_iter()
        .filter_map(|path| huggingface_item(ctx, path))
        .collect()
}

fn huggingface_item(ctx: &ScanContext, path: PathBuf) -> Option<Item> {
    let label = hf_label(&file_label(&path)?)?;
    let bytes = measured(&path)?;
    let detail = located_detail("Hugging Face model", &path, &ctx.home);
    Some(Item::new(
        Category::AiModels,
        label,
        detail,
        bytes,
        Risk::Review,
        Action::DeletePath(path),
    ))
}
