# Contributing

diskdiet deletes files on people's machines, so changes get a stricter review than usual.

## Setup

You need macOS or Linux and the stable Rust toolchain, which `rust-toolchain.toml` pins with `rustfmt` and `clippy`.

```sh
git clone https://github.com/brian-rey-development/diskdiet.git
cd diskdiet
./scripts/dev.sh        # cargo run -- --dry-run
./scripts/report.sh     # cargo run --release -- --report
```

Always develop with `--dry-run`. `scripts/dev.sh` adds it and passes extra flags through, such as `--root ~/some/folder`.

To see where scan time goes, run `cargo run --release --example profile`. It times each scanner on its own.

## Recording the demo

The README GIF is recorded with [vhs](https://github.com/charmbracelet/vhs). Build a release binary and run `vhs scripts/demo.tape` from the repo root. The tape runs in a fake home folder built by `scripts/demo-sandbox.sh`, so no real paths show up.

## Before you open a pull request

```sh
./scripts/check.sh
```

This runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. All three must pass, and CI runs them on macOS and Ubuntu. The `all` and `pedantic` clippy groups are denied in `Cargo.toml`, with `must_use_candidate` and `missing_errors_doc` allowed crate-wide. Any local `#[allow]` goes on the smallest item that needs it. On macOS, `rustup target add x86_64-unknown-linux-gnu` and `cargo clippy --all-targets --target x86_64-unknown-linux-gnu -- -D warnings` catch Linux-only code before CI does.

## Code standards

- Rust 2024. No `unwrap()` outside tests. `unsafe` is denied except in `src/infra/fs/size/bulk.rs`. Use `anyhow` for errors at the edges.
- Functions of 20 lines or fewer, enforced by clippy `too_many_lines`. Use named constants, early returns and immutable bindings.
- No comments. If a reason needs a comment, extract a well-named function or constant instead. The one exception is the `// SAFETY:` note in `src/infra/fs/size/bulk.rs`. Help text for `--help` goes in `#[arg(help = "...")]` in `src/app/cli.rs`.
- Code lives in four layers, `app`, `features`, `domain` and `infra`, and imports only point downward (see the [module map](docs/ARCHITECTURE.md#module-map)). Write imports as full `use crate::layer::...` paths, because `tests/it/architecture.rs` checks the direction by reading them.
- Scanners never panic. On any error they skip the item.
- Commits follow Conventional Commits, in English.

## The one hard rule

Every safety rule needs a test. The rules are in [docs/SAFETY.md](docs/SAFETY.md). If you add or change a rule, add or update its test in the same change. A pull request that touches deletion logic without a test will not be merged.

Integration tests build into one binary, `tests/it/main.rs`, and the module tree under `tests/it/` mirrors `src/`. Shared helpers live in `tests/it/support/`. Tests return `anyhow::Result<()>` and use temporary folders, never real ones on your machine. Run `cargo test <module>::` for one part, for example `cargo test features::scan::worktrees::`.

## Adding a scanner

The steps are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#how-to-add-a-scanner). In short, write `scan(ctx: &ScanContext) -> Vec<Item>`, register it in `SCANNERS`, measure with `size::disk_usage` and test the decision logic. When you are unsure whether something is safe to preselect, leave it unticked. A wrongly unticked box costs the user two seconds, and a wrongly ticked one can cost them their work.

## Reporting bugs

Include your OS and version, the command you ran and the relevant log lines (`~/Library/Logs/diskdiet.log` on macOS, `~/.local/state/diskdiet/diskdiet.log` on Linux). If diskdiet offered or removed something it should not have, open a safety report instead.

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
