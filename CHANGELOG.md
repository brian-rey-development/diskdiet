# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The scan spinner now updates as each scanner finishes, showing how many have completed and the running total per category, biggest first and limited to the categories you asked for with `--only`. The scan exposes this through `scan_with_progress`, which calls back once per finished scanner and keeps `scan` independent of the UI, and `--report` stays silent.
- A review screen after the per-category picks lists every selected item grouped by category, with a subtotal for each group and a grand total. From it you can continue, cancel, or choose a category and edit it again with the current selection preselected. A dangerous item that is newly selected during an edit asks for its typed confirmation just as it does the first time.
- Categories with more than ten items open with type-to-filter enabled, and the title says so.

### Changed

- Git worktrees are listed grouped by repository and sorted by branch name, and each row shows its repository as `repo › branch`.
- The detail of the highlighted item moves to its own line under the row, and every detail, label, blurb, review row, spinner message and progress message is cut to the terminal width with an ellipsis instead of wrapping.
- The typed confirmation for a dangerous worktree now appears right after the category it belongs to, with the item's details in a warning above the prompt, instead of after every category has been answered.
- The summary table right-aligns the item counts and is followed by a one-line reminder of the keys.
- The execution progress bar shows the repository of a worktree and keeps the item name inside the terminal width.
- Startup is faster because tools are looked up on `PATH` first, and the login shell is only started when a tool is not found there.
- Fewer git calls per scan, because repositories without linked worktrees are skipped before git is asked about them.
- App usage is read with one batched `mdls` call instead of one per app.
- The scan thread pool is capped at eight threads, which keeps large machines from spawning more workers than the disk can feed.
- CI uses `actions/checkout` v7 and the minimum supported Rust version is now 1.88.
- The code is split into small single-purpose modules, `scan` and `actions` no longer depend on each other, and clippy's `pedantic` group is now enforced in CI alongside the default lints.

### Fixed

- The finish message printed the macOS log path on Linux, and now shows the real log location for each platform.
- Counts of one now read "1 item" instead of "1 items" in the scan summary ("Found 1.2 GB across 1 item") and the execution progress ("Processed 1 item").
- A worktree with an ignored `build/`, `dist/`, `target/`, `coverage/` or `.cache/` folder is no longer reported as Safe when that folder sits in a directory without a project manifest, because a folder such as `deploy/build/` can hold files like signing keys that exist nowhere else. These names are now excused only next to a `package.json`, `Cargo.toml`, `Makefile` or similar file.
- The worktree prune guard no longer reads git's English output. It lists worktrees with `git worktree list --porcelain` under a timeout and skips the prune when the listing fails or when any worktree of the repo is on a drive that is not mounted, where before a failed or translated git message let the prune go ahead.
- A mount point on Linux is now recognised by its device number, so an empty `/mnt/x` left behind after a drive is unplugged counts as not mounted instead of mounted.
- The delete guard now refuses `/run/media`, `/run/media/USER` and the drives mounted below them (`/media/USER/LABEL`, `/run/media/USER/LABEL`, `/mnt/x`, `/Volumes/X`), where before only direct children of `/media` and `/run/media` were protected.
- `--only` no longer hides risk. All scanners now run before the category filter, so a Safe cache folder that contains a worktree or an AI model is shown as Review or Danger even when you ask for a single category.
- The check made right before deleting an app cache now skips the item when `ps` cannot be run, instead of treating an empty process list as "the app is not running".
- Running external commands no longer waits forever when the exit watcher fails, and a command whose output pipe breaks is reported as failed instead of returning partial output as if it were complete.

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
