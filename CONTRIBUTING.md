# Contributing

Thanks for helping. diskdiet deletes files on people's machines, so the bar for changes is higher than usual, and the process below exists to keep that bar visible.

## Setup

You need macOS and a stable Rust toolchain (the repo pins `stable` with `rustfmt` and `clippy` in `rust-toolchain.toml`).

```sh
git clone https://github.com/brian-rey-development/diskdiet.git
cd diskdiet
./scripts/dev.sh        # cargo run -- --dry-run
./scripts/report.sh     # cargo run --release -- --report
```

Always develop with `--dry-run`. `scripts/dev.sh` does that for you, and passes any extra flags through, for example `./scripts/dev.sh --root ~/some/folder`.

To see where scan time goes, run `cargo run --release --example profile`. It runs each scanner on its own and prints how long it took and how many items it found, which is how the worktree scanner was found to spend about a second per large worktree in `git status --ignored=traditional` (the `matching` mode it uses now takes 0.06 seconds).

## Before you open a pull request

```sh
./scripts/check.sh
```

It runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. All three must pass.

## Code standards

- Rust 2024, no `unwrap()` outside tests, `unsafe` denied by the lint config everywhere except `src/size/bulk.rs` (the one `getattrlistbulk` call, which parses its buffer through bounds-checked slices), `anyhow` for errors at the edges.
- Functions of 20 lines or fewer (clippy `too_many_lines` enforces it), named constants instead of magic numbers and strings, early returns, immutability by default.
- No comments unless the why is non-obvious (a hidden constraint, a subtle invariant, a workaround). No doc comments except on `cli.rs` arguments, because those become `--help`.
- Scanners never panic. On any error they skip the item.
- Commits follow Conventional Commits, in English.

## The one hard rule

**Every safety rule needs a test.** The rules are listed in [docs/SAFETY.md](docs/SAFETY.md). If you add a rule, add the test in `tests/` in the same change. If you change behavior that a rule covers, the test must still prove the rule holds. A pull request that touches deletion logic without a test will not be merged.

Tests should use fixtures from `tests/common/mod.rs` (temporary git repos and fake projects), never real folders on your machine.

## Adding a scanner

The steps are in [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md#how-to-add-a-scanner). In short, write `scan(ctx: &ScanContext) -> Vec<Item>`, register it in `SCANNERS`, measure with `size::disk_usage`, assign risk conservatively, expose the decision logic as a `pub fn` and test it.

When you are unsure whether something is safe to preselect, leave it unselected. A wrongly unticked box costs the user two seconds, and a wrongly ticked one can cost them their work.

## Reporting bugs

Include your macOS version, the command you ran, and the relevant lines from `~/Library/Logs/diskdiet.log`. For anything where diskdiet offered or removed something it should not have, see the end of [docs/SAFETY.md](docs/SAFETY.md).

## License

By contributing you agree that your contributions are licensed under the [MIT License](LICENSE).
