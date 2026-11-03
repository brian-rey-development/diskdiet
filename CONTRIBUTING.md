# Contributing

Thanks for helping. diskdiet deletes files on people's machines, so the bar for changes is higher than usual, and the process below exists to keep that bar visible.

## Setup

You need macOS or Linux and a stable Rust toolchain (the repo pins `stable` with `rustfmt` and `clippy` in `rust-toolchain.toml`).

```sh
git clone https://github.com/brian-rey-development/diskdiet.git
cd diskdiet
./scripts/dev.sh        # cargo run -- --dry-run
./scripts/report.sh     # cargo run --release -- --report
```

Always develop with `--dry-run`. `scripts/dev.sh` does that for you, and passes any extra flags through, for example `./scripts/dev.sh --root ~/some/folder`.

To see where scan time goes, run `cargo run --release --example profile`. It runs each scanner on its own and prints how long it took and how many items it found, which is how the worktree scanner was found to spend about a second per large worktree in `git status --ignored=traditional` (the `matching` mode it uses now takes 0.06 seconds).

## Recording the demo

The GIF in the README is recorded with [vhs](https://github.com/charmbracelet/vhs). Build a release binary, then run `vhs scripts/demo.tape` from the repo root. The tape runs inside a fake home folder built by `scripts/demo-sandbox.sh`, so no real project names or paths show up in the recording.

## Before you open a pull request

```sh
./scripts/check.sh
```

It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. All three must pass, and CI runs them on both macOS and Ubuntu. Clippy runs with the `all` and `pedantic` groups denied in `Cargo.toml`, plus a few nursery lints, so a warning anywhere fails the build. The only pedantic lints allowed crate-wide are `must_use_candidate` and `missing_errors_doc`, and the few local `#[allow]` attributes sit on the smallest item that needs them, such as the one function that converts a float to a byte count after checking its range. To lint the other platform from your machine, add its target with `rustup target add` and run `cargo clippy --target <triple> --all-targets`.

## Code standards

- Rust 2024, no `unwrap()` outside tests, `unsafe` denied by the lint config everywhere except `src/size/bulk.rs` (the one `getattrlistbulk` call, which parses its buffer through bounds-checked slices), `anyhow` for errors at the edges.
- Functions of 20 lines or fewer (clippy `too_many_lines` enforces it), named constants instead of magic numbers and strings, early returns, immutability by default.
- No comments. Names carry the intent, and a reason that would need a comment becomes a well-named function or constant instead. The single exception is the `// SAFETY:` note on the one `unsafe` block, which is the Rust convention for stating why it is sound. Help text for `--help` goes through `#[arg(help = "...")]` in `cli.rs`, not doc comments.
- Scanners never panic. On any error they skip the item.
- Commits follow Conventional Commits, in English.

## The one hard rule

**Every safety rule needs a test.** The rules are listed in [docs/SAFETY.md](docs/SAFETY.md). If you add a rule, add the test in `tests/it/`, in the module that mirrors the code you touched, in the same change. If you change behavior that a rule covers, the test must still prove the rule holds. A pull request that touches deletion logic without a test will not be merged.

All integration tests build into one binary, `tests/it/main.rs`, and the module tree under `tests/it/` mirrors `src/`. Shared helpers and constants live in `tests/it/support/` (temporary git repos, item and config builders), so reuse them instead of redefining your own. Every test returns `anyhow::Result<()>` and uses `?` for fallible setup. Tests must use temporary folders, never real folders on your machine. Run `cargo test` for the whole suite or `cargo test <module>::` to run one part, for example `cargo test scan::worktrees::`.

## Adding a scanner

The steps are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#how-to-add-a-scanner). In short, write `scan(ctx: &ScanContext) -> Vec<Item>`, register it in `SCANNERS`, measure with `size::disk_usage`, assign risk conservatively, expose the decision logic as a `pub fn` and test it.

When you are unsure whether something is safe to preselect, leave it unselected. A wrongly unticked box costs the user two seconds, and a wrongly ticked one can cost them their work.

## Reporting bugs

Include your OS and version, the command you ran, and the relevant lines from the log (`~/Library/Logs/diskdiet.log` on macOS, `~/.local/state/diskdiet/diskdiet.log` on Linux). For anything where diskdiet offered or removed something it should not have, see the end of [docs/SAFETY.md](docs/SAFETY.md).

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
