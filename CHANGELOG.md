# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The scan spinner shows how many scanners are done and the running total per category, biggest first.
- A review screen after the per-category picks lists every selected item with subtotals and a grand total. You can continue, cancel, or edit a category again.
- Categories with more than ten items open with type-to-filter enabled.

### Changed

- The library is organized into four layers, `app`, `features`, `domain` and `infra`, so its public paths move. For example `diskdiet::model` is now `diskdiet::domain::model` and `diskdiet::scan` is now `diskdiet::features::scan`. A test fails when a lower layer imports a higher one.
- Git worktrees are grouped by repository, sorted by branch name, and shown as `repo › branch`.
- The detail of the highlighted item sits on its own line under the row, and long lines are cut with an ellipsis instead of wrapping.
- The typed confirmation for a dangerous worktree comes right after its category, with the item's details above the prompt.
- The summary table right-aligns item counts and ends with a reminder of the keys.
- The progress bar shows the repository of a worktree.
- Startup is faster, scans make fewer git calls, and app usage is read with one `mdls` call instead of one per app.
- The scan thread pool is capped at eight threads.
- CI uses `actions/checkout` v7, and the minimum supported Rust version is now 1.88.
- The code is split into smaller modules, and clippy's `pedantic` group is now enforced in CI.

### Fixed

- The finish message showed the macOS log path on Linux.
- Counts of one read "1 item" instead of "1 items" in the scan summary and progress.
- A worktree with an ignored `build/`, `dist/`, `target/`, `coverage/` or `.cache/` folder is no longer Safe when that folder has no project manifest beside it, because something like `deploy/build/` can hold signing keys that exist nowhere else.
- The worktree prune is skipped when git's worktree listing fails or any worktree of the repo is on an unmounted drive. Before, a failed or translated git message let the prune go ahead.
- On Linux, an empty `/mnt/x` left after unplugging a drive now counts as not mounted.
- The delete guard now refuses `/run/media`, `/run/media/USER` and every drive mounted below them, such as `/media/USER/LABEL` or `/Volumes/X`. Before, only direct children of `/media` and `/run/media` were protected.
- `--only` no longer hides risk. A Safe cache folder that contains a worktree or an AI model now shows as Review or Danger even when you ask for one category.
- Deleting an app cache is skipped when `ps` cannot be run, instead of assuming the app is not running.
- External commands no longer hang when their exit watcher fails, and a broken output pipe is reported as a failure instead of partial output.

## [0.1.0] - Unreleased

### Added

- Interactive flow with a summary, one multiselect per category, typed confirmation for dangerous items and a final confirm.
- Parallel scanners for git worktrees, build artifacts, Python virtualenvs, package manager caches, app caches, Docker, AI models, unused apps, Downloads and Trash.
- Worktree risk assessment from commits ahead, dirty files and ignored files that are not known build output.
- Discovery of worktrees outside the dev roots by following `git worktree list`.
- Real disk usage measurement (allocated bytes, no double counting, no symlink following) and a freed-space figure read from free space before and after deleting.
- `--dry-run`, `--report`, `--report --json`, `--root`, `--config`, `--only` and `--no-share` flags.
- Safety guards for deletion targets, worktree rechecks before removal, risk inheritance for nested items and graceful Ctrl-C during cleanup.
- TOML configuration in `~/.config/diskdiet/config.toml`, with an example in `config/diskdiet.example.toml`.
- Append-only action log in `~/Library/Logs/diskdiet.log` on macOS and `~/.local/state/diskdiet/diskdiet.log` on Linux.
- Linux support, including freedesktop.org Trash, `~/.cache` app caches, Linux installer formats and Linux mount points.
- Share card and X intent link after a cleanup.
- `setup.sh` installer with `--uninstall`, helper scripts in `scripts/` and a `vhs` tape for the demo GIF.

[Unreleased]: https://github.com/brian-rey-development/diskdiet/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/brian-rey-development/diskdiet/releases/tag/v0.1.0
