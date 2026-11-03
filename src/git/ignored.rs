use std::path::Path;

const UNAMBIGUOUS_BUILD_DIRS: [&str; 12] = [
    "node_modules",
    ".next",
    ".turbo",
    "__pycache__",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
    ".svelte-kit",
    ".nuxt",
    ".tanstack",
    ".parcel-cache",
    ".venv",
];
const AMBIGUOUS_BUILD_DIRS: [&str; 5] = ["target", "dist", "build", "coverage", ".cache"];
const PROJECT_MANIFESTS: [&str; 15] = [
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "setup.py",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "CMakeLists.txt",
    "Makefile",
    "composer.json",
    "Gemfile",
    "mix.exs",
    "deno.json",
    "bun.lockb",
];
const IGNORABLE_FILES: [&str; 3] = [".DS_Store", "next-env.d.ts", ".eslintcache"];
const IGNORABLE_EXTENSIONS: [&str; 2] = [".tsbuildinfo", ".pyc"];
const GENERATED_DIR_PAIRS: [[&str; 2]; 2] = [[".husky", "_"], ["src-tauri", "gen"]];

fn is_generated_file(name: &str) -> bool {
    IGNORABLE_FILES.contains(&name) || IGNORABLE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

fn has_project_manifest(dir: &Path) -> bool {
    PROJECT_MANIFESTS
        .iter()
        .any(|manifest| dir.join(manifest).is_file())
}

fn is_build_dir(parts: &[&str], worktree: &Path) -> bool {
    let Some((last, parents)) = parts.split_last() else {
        return false;
    };
    if UNAMBIGUOUS_BUILD_DIRS.contains(last) {
        return true;
    }
    AMBIGUOUS_BUILD_DIRS.contains(last) && has_project_manifest(&worktree.join(parents.join("/")))
}

pub fn is_build_output(path: &str, worktree: &Path) -> bool {
    let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
    let Some(last) = parts.last() else {
        return false;
    };
    let in_generated_dir = parts
        .windows(2)
        .any(|pair| GENERATED_DIR_PAIRS.iter().any(|known| known == pair));
    if in_generated_dir {
        return true;
    }
    if !path.ends_with('/') {
        return is_generated_file(last);
    }
    is_build_dir(&parts, worktree)
}
