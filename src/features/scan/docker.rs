use super::ScanContext;
use crate::domain::model::{Action, Category, FSTRIM, Item, Risk};
use crate::infra::sys::process::{DEFAULT_TIMEOUT, run_stdout};
use crate::infra::sys::tools;
use crate::infra::util::fmt::parse_size;
use std::path::Path;

const VM_TRIM_LABEL: &str = "Reclaim VM disk space";
const VM_TRIM_DETAIL: &str = "returns freed space inside the VM to macOS";
const FILTER_FLAG: &str = "--filter";
const VM_TRIM_ARGS: [&str; 5] = ["ssh", "--", "sudo", FSTRIM, "-a"];
const LOCAL_SOCKET_SCHEME: &str = "unix://";
const DOCKER_HOST_VAR: &str = "DOCKER_HOST";
const DF_ARGS: [&str; 4] = ["system", "df", "--format", "{{json .}}"];
const CONTEXT_HOST_ARGS: [&str; 4] = [
    "context",
    "inspect",
    "--format",
    "{{.Endpoints.docker.Host}}",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum DockerKind {
    Images,
    BuildCache,
}

impl DockerKind {
    const fn df_type(self) -> &'static str {
        match self {
            Self::Images => "Images",
            Self::BuildCache => "Build Cache",
        }
    }
}

struct PruneSpec {
    kind: DockerKind,
    label: &'static str,
    args: &'static [&'static str],
    risk: Risk,
    note: &'static str,
}

const PRUNES: [PruneSpec; 2] = [
    PruneSpec {
        kind: DockerKind::Images,
        label: "Unused Docker images",
        args: &["image", "prune", "-af"],
        risk: Risk::Review,
        note: "Images no container uses, created more than",
    },
    PruneSpec {
        kind: DockerKind::BuildCache,
        label: "Docker build cache",
        args: &["builder", "prune", "-af"],
        risk: Risk::Safe,
        note: "Build cache older than",
    },
];

pub fn scan(ctx: &ScanContext) -> Vec<Item> {
    let Some(docker) = ctx.tools.get(tools::DOCKER) else {
        return Vec::new();
    };
    if !is_local_daemon(docker) {
        return Vec::new();
    }
    let (daemon_running, (mut items, vm_trim)) = rayon::join(
        || run_stdout(docker, &["info"], DEFAULT_TIMEOUT).is_some(),
        || rayon::join(|| prune_items(ctx, docker), || vm_trim_item(ctx)),
    );
    if !daemon_running {
        return Vec::new();
    }
    items.extend(vm_trim);
    items
}

pub fn prune_risk(kind: &str) -> Option<Risk> {
    PRUNES
        .iter()
        .find(|spec| spec.kind.df_type() == kind)
        .map(|spec| spec.risk)
}

pub fn is_local_endpoint(host: &str) -> bool {
    host.trim().starts_with(LOCAL_SOCKET_SCHEME)
}

fn is_local_daemon(docker: &Path) -> bool {
    if let Ok(host) = std::env::var(DOCKER_HOST_VAR) {
        return is_local_endpoint(&host);
    }
    run_stdout(docker, &CONTEXT_HOST_ARGS, DEFAULT_TIMEOUT)
        .is_some_and(|host| is_local_endpoint(&host))
}

pub fn parse_docker_df(text: &str) -> Vec<(String, u64)> {
    text.lines().filter_map(parse_df_line).collect()
}

fn parse_df_line(line: &str) -> Option<(String, u64)> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let kind = value.get("Type")?.as_str()?.to_owned();
    let bytes = parse_size(value.get("Reclaimable")?.as_str()?)?;
    Some((kind, bytes))
}

fn prune_items(ctx: &ScanContext, docker: &Path) -> Vec<Item> {
    let Some(df) = run_stdout(docker, &DF_ARGS, DEFAULT_TIMEOUT) else {
        return Vec::new();
    };
    let usage = parse_docker_df(&df);
    PRUNES
        .iter()
        .filter_map(|spec| {
            let bytes = usage
                .iter()
                .find(|(kind, _)| kind == spec.kind.df_type())?
                .1;
            (bytes > 0).then(|| prune_item(ctx, docker, spec, bytes))
        })
        .collect()
}

fn prune_item(ctx: &ScanContext, docker: &Path, spec: &PruneSpec, bytes: u64) -> Item {
    let hours = ctx.config.docker_prune_until_hours;
    let mut args: Vec<String> = spec.args.iter().map(|arg| (*arg).to_owned()).collect();
    args.extend([FILTER_FLAG.to_owned(), format!("until={hours}h")]);
    let action = Action::RunCommand {
        program: docker.to_path_buf(),
        args,
    };
    let detail = format!("{} {hours} hours ago, up to this size", spec.note);
    Item::new(
        Category::Docker,
        spec.label,
        detail,
        bytes,
        spec.risk,
        action,
    )
}

fn vm_trim_item(ctx: &ScanContext) -> Option<Item> {
    let colima = ctx.tools.get(tools::COLIMA)?;
    run_stdout(colima, &["status"], DEFAULT_TIMEOUT)?;
    let action = Action::RunCommand {
        program: colima.to_path_buf(),
        args: VM_TRIM_ARGS.iter().map(|arg| (*arg).to_owned()).collect(),
    };
    Some(Item::new(
        Category::Docker,
        VM_TRIM_LABEL,
        VM_TRIM_DETAIL,
        0,
        Risk::Safe,
        action,
    ))
}
