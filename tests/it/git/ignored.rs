use diskdiet::git::ignored::is_build_output;

#[test]
fn build_output_is_recognised_by_any_path_component() {
    for known in [
        "target/",
        "apps/web/node_modules/",
        ".next/",
        ".venv/",
        "x/__pycache__/",
        "apps/web/next-env.d.ts",
        "apps/web/tsconfig.tsbuildinfo",
        ".husky/_/",
        ".husky/_/pre-commit",
        "coverage/",
        "apps/web/.tanstack/",
        "app/src-tauri/gen/",
    ] {
        assert!(is_build_output(known), "{known}");
    }
}

#[test]
fn personal_files_are_never_build_output() {
    for personal in [
        ".env",
        "config/.env.local",
        "secrets/key.pem",
        "targets/a",
        "notes.md",
        ".husky/pre-commit",
        ".vercel/project.json",
        "src/gen/schema.rs",
        "pkg/dist/a.js",
        "deploy/build/signing.pem",
        "target",
    ] {
        assert!(!is_build_output(personal), "{personal}");
    }
}
