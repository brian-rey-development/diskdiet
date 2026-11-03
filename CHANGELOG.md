# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - Unreleased

### Added

- Interactive flow with a summary, one multiselect per category, typed confirmation for dangerous items and a final confirm.
- Parallel scanners for git worktrees, build artifacts, Python virtualenvs, package manager caches, app caches, Docker, AI models, unused apps, Downloads and Trash.
- Worktree risk assessment from commits ahead, dirty files and ignored files that are not known build output.
- Discovery of worktrees outside the dev roots by following `git worktree list`.
- Real disk usage measurement (allocated bytes, inode dedupe, no symlink following) through `getattrlistbulk`, with a `read_dir` fallback, and a freed-space figure measured with `statvfs` right before and after deleting.
- `--dry-run`, `--report`, `--report --json`, `--root`, `--config`, `--only` and `--no-share` flags.
- Safety guards for deletion targets, worktree rechecks before removal, risk inheritance for nested items, and graceful Ctrl-C during cleanup (the current step finishes, the rest is left untouched).
- TOML configuration in `~/.config/diskdiet/config.toml` with a documented example in `config/diskdiet.example.toml`.
- Append-only action log in `~/Library/Logs/diskdiet.log`.
- Share card and X intent link after a cleanup.
- `setup.sh` installer with `--uninstall`, plus `scripts/check.sh`, `scripts/dev.sh`, `scripts/report.sh`, `scripts/demo-sandbox.sh` and a `vhs` tape for the demo GIF.

[Unreleased]: https://github.com/brian-rey-development/diskdiet/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/brian-rey-development/diskdiet/releases/tag/v0.1.0
