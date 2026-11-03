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
    pub const ALL: [Category; 10] = [
        Category::AgentWorktrees,
        Category::BuildArtifacts,
        Category::PythonEnvs,
        Category::PackageCaches,
        Category::AppCaches,
        Category::Docker,
        Category::AiModels,
        Category::UnusedApps,
        Category::Downloads,
        Category::Trash,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Category::AgentWorktrees => "Git worktrees",
            Category::BuildArtifacts => "Build artifacts",
            Category::PythonEnvs => "Python virtualenvs",
            Category::PackageCaches => "Package manager caches",
            Category::AppCaches => "App caches",
            Category::Docker => "Docker",
            Category::AiModels => "AI models",
            Category::UnusedApps => "Unused apps",
            Category::Downloads => "Downloads",
            Category::Trash => "Trash",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Category::AgentWorktrees => "Worktrees left behind by you or your coding agents",
            Category::BuildArtifacts => {
                "Rust target/, .next/, .turbo/ and stale node_modules. Rebuilt on demand"
            }
            Category::PythonEnvs => "Project .venv folders. Run `uv sync` to recreate them",
            Category::PackageCaches => {
                "uv, pnpm, npm, cargo, Homebrew and friends. Re-downloaded on demand"
            }
            Category::AppCaches => "Regenerable caches inside ~/Library/Caches",
            Category::Docker => {
                "Unused images and build cache. Containers and volumes are never touched"
            }
            Category::AiModels => "Local Ollama and Hugging Face models. Re-downloaded on demand",
            Category::UnusedApps => "Apps you have not opened in a long time. Moved to the Trash",
            Category::Downloads => "Installers and old files in ~/Downloads. Moved to the Trash",
            Category::Trash => "Everything already in your Trash. Permanent",
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
            Risk::Safe => "safe",
            Risk::Review => "review",
            Risk::Danger => "danger",
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
            Action::DeletePath(path) | Action::MoveToTrash(path) => Some(path),
            Action::RemoveWorktree { worktree, .. } => Some(worktree),
            Action::PruneWorktrees { repo } => Some(repo),
            Action::RunCommand { .. } | Action::EmptyTrash => None,
        }
    }

    pub fn deletes_path(&self) -> bool {
        matches!(self, Action::DeletePath(_) | Action::MoveToTrash(_))
    }

    pub fn is_vm_trim(&self) -> bool {
        matches!(self, Action::RunCommand { args, .. } if args.iter().any(|arg| arg == FSTRIM))
    }

    pub fn ignores_size_floor(&self) -> bool {
        match self {
            Action::PruneWorktrees { .. } | Action::RemoveWorktree { .. } | Action::EmptyTrash => {
                true
            }
            Action::RunCommand { .. } => self.is_vm_trim(),
            Action::DeletePath(_) | Action::MoveToTrash(_) => false,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::DeletePath(path) => write!(f, "delete {}", path.display()),
            Action::MoveToTrash(path) => write!(f, "trash {}", path.display()),
            Action::EmptyTrash => f.write_str("empty trash"),
            Action::RunCommand { program, args } => {
                write!(f, "run {} {}", program.display(), args.join(" "))
            }
            Action::RemoveWorktree { repo, worktree, .. } => write!(
                f,
                "remove worktree {} (repo {})",
                worktree.display(),
                repo.display()
            ),
            Action::PruneWorktrees { repo } => {
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
            Outcome::Done => f.write_str("done"),
            Outcome::DryRun => f.write_str("dry-run"),
            Outcome::Skipped(why) => write!(f, "skipped: {why}"),
            Outcome::Failed(why) => write!(f, "failed: {why}"),
            Outcome::NeedsFullDiskAccess => f.write_str("needs full disk access"),
            Outcome::Stopped => f.write_str("stopped"),
        }
    }
}
