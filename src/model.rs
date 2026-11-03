use std::path::PathBuf;

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
        force: bool,
    },
    PruneWorktrees {
        repo: PathBuf,
    },
    EmptyTrash,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Done,
    DryRun,
    Skipped(String),
    Failed(String),
    NeedsFullDiskAccess,
}
