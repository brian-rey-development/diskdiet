use std::fmt;
use std::path::{Path, PathBuf};

pub const DELETE_CONFIRMATION: &str = "delete";
pub const FSTRIM: &str = "fstrim";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, clap::ValueEnum)]
pub enum Category {
    AgentWorktrees,
    BuildArtifacts,
    PythonEnvs,
    PackageCaches,
    AppCaches,
    Docker,
    AiModels,
    UnusedApps,
    Downloads,
    Trash,
}

impl Category {
    pub const ALL: [Self; 10] = [
        Self::AgentWorktrees,
        Self::BuildArtifacts,
        Self::PythonEnvs,
        Self::PackageCaches,
        Self::AppCaches,
        Self::Docker,
        Self::AiModels,
        Self::UnusedApps,
        Self::Downloads,
        Self::Trash,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Self::AgentWorktrees => "Git worktrees",
            Self::BuildArtifacts => "Build artifacts",
            Self::PythonEnvs => "Python virtualenvs",
            Self::PackageCaches => "Package manager caches",
            Self::AppCaches => "App caches",
            Self::Docker => "Docker",
            Self::AiModels => "AI models",
            Self::UnusedApps => "Unused apps",
            Self::Downloads => "Downloads",
            Self::Trash => "Trash",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Self::AgentWorktrees => "Worktrees left behind by you or your coding agents",
            Self::BuildArtifacts => {
                "Rust target/, .next/, .turbo/ and stale node_modules. Rebuilt on demand"
            }
            Self::PythonEnvs => "Project .venv folders. Run `uv sync` to recreate them",
            Self::PackageCaches => {
                "uv, pnpm, npm, cargo, Homebrew and friends. Re-downloaded on demand"
            }
            Self::AppCaches => "Regenerable caches inside ~/Library/Caches",
            Self::Docker => {
                "Unused images and build cache. Containers and volumes are never touched"
            }
            Self::AiModels => "Local Ollama and Hugging Face models. Re-downloaded on demand",
            Self::UnusedApps => "Apps you have not opened in a long time. Moved to the Trash",
            Self::Downloads => "Installers and old files in ~/Downloads. Moved to the Trash",
            Self::Trash => "Everything already in your Trash. Permanent",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Safe,
    Review,
    Danger,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Review => "review",
            Self::Danger => "danger",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Live,
    DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Rechecked,
    Forced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    DeletePath(PathBuf),
    MoveToTrash(PathBuf),
    RunCommand {
        program: PathBuf,
        args: Vec<String>,
    },
    RemoveWorktree {
        repo: PathBuf,
        worktree: PathBuf,
        removal: Removal,
    },
    PruneWorktrees {
        repo: PathBuf,
    },
    EmptyTrash,
}

impl Action {
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::DeletePath(path) | Self::MoveToTrash(path) => Some(path),
            Self::RemoveWorktree { worktree, .. } => Some(worktree),
            Self::PruneWorktrees { repo } => Some(repo),
            Self::RunCommand { .. } | Self::EmptyTrash => None,
        }
    }

    pub fn deletes_path(&self) -> bool {
        matches!(self, Self::DeletePath(_) | Self::MoveToTrash(_))
    }

    pub fn is_vm_trim(&self) -> bool {
        matches!(self, Self::RunCommand { args, .. } if args.iter().any(|arg| arg == FSTRIM))
    }

    pub fn ignores_size_floor(&self) -> bool {
        match self {
            Self::PruneWorktrees { .. } | Self::RemoveWorktree { .. } | Self::EmptyTrash => true,
            Self::RunCommand { .. } => self.is_vm_trim(),
            Self::DeletePath(_) | Self::MoveToTrash(_) => false,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DeletePath(path) => write!(f, "delete {}", path.display()),
            Self::MoveToTrash(path) => write!(f, "trash {}", path.display()),
            Self::EmptyTrash => f.write_str("empty trash"),
            Self::RunCommand { program, args } => {
                write!(f, "run {} {}", program.display(), args.join(" "))
            }
            Self::RemoveWorktree { repo, worktree, .. } => write!(
                f,
                "remove worktree {} (repo {})",
                worktree.display(),
                repo.display()
            ),
            Self::PruneWorktrees { repo } => {
                write!(f, "prune worktrees (repo {})", repo.display())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub category: Category,
    pub label: String,
    pub detail: String,
    pub bytes: u64,
    pub risk: Risk,
    pub action: Action,
    pub preselected: bool,
    pub typed_confirmation: Option<String>,
}

impl Item {
    pub fn new(
        category: Category,
        label: impl Into<String>,
        detail: impl Into<String>,
        bytes: u64,
        risk: Risk,
        action: Action,
    ) -> Self {
        Self {
            category,
            label: label.into(),
            detail: detail.into(),
            bytes,
            risk,
            action,
            preselected: risk == Risk::Safe,
            typed_confirmation: None,
        }
    }

    #[must_use]
    pub fn with_typed_delete(mut self) -> Self {
        self.typed_confirmation = Some(DELETE_CONFIRMATION.to_owned());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    DryRun,
    Skipped(String),
    Failed(String),
    NeedsFullDiskAccess,
    Stopped,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Done => f.write_str("done"),
            Self::DryRun => f.write_str("dry-run"),
            Self::Skipped(why) => write!(f, "skipped: {why}"),
            Self::Failed(why) => write!(f, "failed: {why}"),
            Self::NeedsFullDiskAccess => f.write_str("needs full disk access"),
            Self::Stopped => f.write_str("stopped"),
        }
    }
}
