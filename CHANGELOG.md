# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Startup is faster because tools are looked up on `PATH` first, and the login shell is only started when a tool is not found there.
- Fewer git calls per scan, because repositories without linked worktrees are skipped before git is asked about them.
- App usage is read with one batched `mdls` call instead of one per app.
- The scan thread pool is capped at eight threads, which keeps large machines from spawning more workers than the disk can feed.
- CI uses `actions/checkout` v7 and the minimum supported Rust version is now 1.88.

### Fixed

- The finish message printed the macOS log path on Linux, and now shows the real log location for each platform.
- Counts of one now read "1 item" instead of "1 items" in the scan summary ("Found 1.2 GB across 1 item") and the execution progress ("Processed 1 item").

## [0.1.0] - Unreleased

### Added

- Interactive flow with a summary, one multiselect per category, typed confirmation for dangerous items and a final confirm.
- Parallel scanners for git worktrees, build artifacts, Python virtualenvs, package manager caches, app caches, Docker, AI models, unused apps, Downloads and Trash.
- Worktree risk assessment from commits ahead, dirty files and ignored files that are not known build output.
- Discovery of worktrees outside the dev roots by following `git worktree list`.
- Real disk usage measurement (allocated bytes, inode dedupe, no symlink following) through `getattrlistbulk` on macOS and parallel `readdir` on Linux, and a freed-space figure measured with `statvfs` right before and after deleting.
- `--dry-run`, `--report`, `--report --json`, `--root`, `--config`, `--only` and `--no-share` flags.
- Safety guards for deletion targets, worktree rechecks before removal, risk inheritance for nested items, and graceful Ctrl-C during cleanup (the current step finishes, the rest is left untouched).
- TOML configuration in `~/.config/diskdiet/config.toml` with a documented example in `config/diskdiet.example.toml`.
- Append-only action log in `~/Library/Logs/diskdiet.log` on macOS and `~/.local/state/diskdiet/diskdiet.log` on Linux.
- Linux support, including freedesktop.org Trash, `~/.cache` app caches, Linux installer formats and Linux mount points.
- Share card and X intent link after a cleanup.
- `setup.sh` installer with `--uninstall`, plus `scripts/check.sh`, `scripts/dev.sh`, `scripts/report.sh`, `scripts/demo-sandbox.sh` and a `vhs` tape for the demo GIF.

[Unreleased]: https://github.com/brian-rey-development/diskdiet/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/brian-rey-development/diskdiet/releases/tag/v0.1.0
