use super::ScanContext;
use super::caches::{make_item, run_stdout};
use crate::model::{Action, Category, Item, Risk};
use std::path::Path;

const KIND_IMAGES: &str = "Images";
const KIND_BUILD_CACHE: &str = "Build Cache";
const VM_TRIM_LABEL: &str = "Reclaim VM disk space";
const VM_TRIM_DETAIL: &str = "returns freed space inside the VM to macOS";
const FILTER_FLAG: &str = "--filter";
const VM_TRIM_ARGS: [&str; 5] = ["ssh", "--", "sudo", "fstrim", "-a"];
const LOCAL_SOCKET_SCHEME: &str = "unix://";
const DOCKER_HOST_VAR: &str = "DOCKER_HOST";
const CONTEXT_HOST_ARGS: [&str; 4] = [
    "context",
    "inspect",
    "--format",
    "{{.Endpoints.docker.Host}}",
];

const UNITS: [(&str, f64); 5] = [
    ("b", 1.0),
    ("kb", 1e3),
    ("mb", 1e6),
    ("gb", 1e9),
    ("tb", 1e12),
];

struct PruneSpec {
    kind: &'static str,
    label: &'static str,
    args: &'static [&'static str],
    risk: Risk,
    note: &'static str,
}

const PRUNES: [PruneSpec; 2] = [
    PruneSpec {
        kind: KIND_IMAGES,
        label: "Unused Docker images",
        args: &["image", "prune", "-af"],
        risk: Risk::Review,
        note: "Images no container uses, created more than",
    },
    PruneSpec {
        kind: KIND_BUILD_CACHE,
        label: "Docker build cache",
        args: &["builder", "prune", "-af"],
        risk: Risk::Safe,
        note: "Build cache older than",
    },
];

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Some(docker) = ctx.tools.get("docker") else {
        return Vec::new();
    };
    if !is_local_daemon(docker) || run_stdout(docker, &["info"]).is_none() {
        return Vec::new();
    }
    let mut items = prune_items(ctx, docker);
    items.extend(vm_trim_item(ctx));
    items
}

pub fn prune_risk(kind: &str) -> Option<Risk> {
    PRUNES
        .iter()
        .find(|spec| spec.kind == kind)
        .map(|spec| spec.risk)
}

pub fn is_local_endpoint(host: &str) -> bool {
    host.trim().starts_with(LOCAL_SOCKET_SCHEME)
}

fn is_local_daemon(docker: &Path) -> bool {
    if let Ok(host) = std::env::var(DOCKER_HOST_VAR) {
        return is_local_endpoint(&host);
    }
    run_stdout(docker, &CONTEXT_HOST_ARGS).is_some_and(|host| is_local_endpoint(&host))
}

pub fn parse_docker_size(text: &str) -> Option<u64> {
    let token = text.split_whitespace().next()?;
    let split = token.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let number: f64 = token[..split].parse().ok()?;
    let unit = token[split..].to_ascii_lowercase();
    let (_, factor) = UNITS.iter().find(|(name, _)| *name == unit)?;
    Some((number * factor).round() as u64)
}

pub fn parse_docker_df(text: &str) -> Vec<(String, u64)> {
    text.lines().filter_map(parse_df_line).collect()
}

fn parse_df_line(line: &str) -> Option<(String, u64)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let kind = value.get("Type")?.as_str()?.to_string();
    let bytes = parse_docker_size(value.get("Reclaimable")?.as_str()?)?;
    Some((kind, bytes))
}

fn prune_items(ctx: &ScanContext, docker: &Path) -> Vec<Item> {
    let Some(df) = run_stdout(docker, &["system", "df", "--format", "{{json .}}"]) else {
        return Vec::new();
    };
    let usage = parse_docker_df(&df);
    PRUNES
        .iter()
        .filter_map(|spec| {
            let bytes = usage.iter().find(|(kind, _)| kind == spec.kind)?.1;
            (bytes > 0).then(|| prune_item(ctx, docker, spec, bytes))
        })
        .collect()
}

fn prune_item(ctx: &ScanContext, docker: &Path, spec: &PruneSpec, bytes: u64) -> Item {
    let hours = ctx.config.docker_prune_until_hours;
    let mut args: Vec<String> = spec.args.iter().map(ToString::to_string).collect();
    args.extend([FILTER_FLAG.to_string(), format!("until={hours}h")]);
    let action = Action::RunCommand {
        program: docker.to_path_buf(),
        args,
    };
    let detail = format!("{} {hours} hours ago, up to this size", spec.note);
    make_item(
        Category::Docker,
        spec.label.to_string(),
        detail,
        bytes,
        spec.risk,
        action,
    )
}

fn vm_trim_item(ctx: &ScanContext) -> Option<Item> {
    let colima = ctx.tools.get("colima")?;
    run_stdout(colima, &["status"])?;
    let action = Action::RunCommand {
        program: colima.to_path_buf(),
        args: VM_TRIM_ARGS.iter().map(ToString::to_string).collect(),
    };
    Some(make_item(
        Category::Docker,
        VM_TRIM_LABEL.to_string(),
        VM_TRIM_DETAIL.to_string(),
        0,
        Risk::Safe,
        action,
    ))
}
