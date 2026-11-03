const BUILD_OUTPUT_DIRS: [&str; 17] = [
    "target",
    "node_modules",
    ".next",
    ".turbo",
    "dist",
    "build",
    ".venv",
    "__pycache__",
    "coverage",
    ".cache",
    ".parcel-cache",
    ".pytest_cache",
    ".ruff_cache",
    ".mypy_cache",
    ".svelte-kit",
    ".nuxt",
    ".tanstack",
];
const IGNORABLE_FILES: [&str; 3] = [".DS_Store", "next-env.d.ts", ".eslintcache"];
const IGNORABLE_EXTENSIONS: [&str; 2] = [".tsbuildinfo", ".pyc"];
const GENERATED_DIR_PAIRS: [[&str; 2]; 2] = [[".husky", "_"], ["src-tauri", "gen"]];

fn is_generated_file(name: &str) -> bool {
    IGNORABLE_FILES.contains(&name) || IGNORABLE_EXTENSIONS.iter().any(|ext| name.ends_with(ext))
}

pub fn is_build_output(path: &str) -> bool {
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
    BUILD_OUTPUT_DIRS.contains(last)
}
