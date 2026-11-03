# diskdiet: build plan

> Your AI agents made your disk fat. Put it on a diet.

diskdiet is an interactive, developer-aware disk cleaner for macOS. It finds the junk that generic cleaners miss (git worktrees left behind by coding agents, Rust `target/` dirs, Turborepo caches, uv and pnpm stores, Docker images inside Colima) and lets the user tick, item by item, what to remove.

The origin story, which is also the launch tweet: one real Mac went from 642 GB used to 319 GB used. 105 GB were Rust `target/` folders, 60 GB of them inside ten agent worktrees of a single repo.

## Scope of the MVP

1. Scan, in parallel, every category below and measure real disk usage.
2. Show a summary ("You can reclaim 287 GB"), then one multiselect per category. Safe items come preselected, risky ones come unselected and flagged.
3. Execute the selection with the safety rules below, then print the real amount freed (measured with `statvfs`, not estimated) and a share link for X.

Out of scope until the MVP works: cargo-dist, Homebrew tap, npm wrapper, demo GIF, background scheduling.

## Safety rules (each one comes from a real incident)

These are requirements. Every rule marked (test) needs a test in `tests/`.

1. **Never use `CACHEDIR.TAG` alone as a delete signal.** uv puts it in `.venv` and some model downloaders put it in model folders. A build dir is recognised by name plus a sibling manifest: `target/` next to `Cargo.toml`, `.next/` next to `package.json`, `.turbo/cache` inside a folder with `package.json` or `turbo.json`. (test)
2. **`.venv` is its own category (`PythonEnvs`), never preselected**, with the hint "run `uv sync` to recreate". (test)
3. **"Merged" does not mean safe.** A worktree whose branch has 0 commits ahead of base can still hold uncommitted work. Risk is computed from three facts: commits ahead, dirty tracked/untracked files, and ignored files that are not known build output. (test)
4. **Ignored files count.** `git status --porcelain` hides ignored files, so a worktree with only a `.env` looks clean. Use `git status --porcelain --ignored`, drop entries under known build dirs (`target/`, `node_modules/`, `.next/`, `.turbo/`, `dist/`, `build/`, `.venv/`, `__pycache__/`), and if anything else remains the worktree is `Danger`. (test)
5. **Dangerous worktrees need typed confirmation.** `Item.typed_confirmation = Some("delete")`. The UI asks the user to type the word. Never preselected.
6. **Worktrees whose folder no longer exists** become a single `PruneWorktrees` item per repo (0 bytes, Safe).
7. **Worktrees may live outside the dev roots** (for example `~/orca/workspaces/...`). Discover repos inside the roots, then follow `git worktree list` wherever it points.
8. **Never start Docker or Colima.** Only inspect when `docker info` already succeeds. Otherwise emit nothing and let the UI print "Docker is not running, skipped".
9. **Permission errors are expected.** Deleting inside `~/.Trash` needs Full Disk Access, and root-owned apps such as iMovie cannot be moved with plain file APIs. Moving to Trash and emptying the Trash go through Finder via `osascript`. If an error is EPERM, return `Outcome::NeedsFullDiskAccess` so the UI can explain the fix.
10. **Regenerable data is deleted directly or by the tool's own command** (`uv cache prune`, `pnpm store prune`, `docker image prune`, `brew cleanup`), so the freed space is real immediately. Only apps and downloads go to the Trash.
11. **Resolve tools the way the user's shell does.** pnpm may come from mise and not be on the PATH a binary inherits. Look up `PATH` first, then the login shell's `PATH` (one `$SHELL -lic` call), then `mise which <tool>`.
12. **The headline number must be true.** Disk usage is `st_blocks * 512` (real allocation, sparse files count correctly), hardlinks are counted once per `(dev, ino)` across the whole measurement, symlinks are never followed. The final "freed" figure is the `statvfs` difference before and after.
13. **Dry run deletes nothing** and returns `Outcome::DryRun` for every item.
14. **Every executed action is appended to `~/Library/Logs/diskdiet.log`** with a timestamp, the action and the outcome.

## Architecture

```
src/
  main.rs          parse Args, call ui::run
  lib.rs           module list
  cli.rs           clap Args (done)
  model.rs         Category, Risk, Action, Item, Outcome (done, shared contract)
  config.rs        Config with defaults, TOML load from ~/.config/diskdiet/config.toml
  size.rs          disk_usage (parallel, blocks*512, inode dedupe), format_bytes
  disk.rs          free_bytes via rustix::fs::statvfs (f_bavail * f_frsize)
  tools.rs         Tools::discover resolves KNOWN_TOOLS once, in parallel
  log.rs           append-only action log
  scan/mod.rs      ScanContext, SCANNERS, scan_all (rayon over scanners)
  scan/worktrees.rs
  scan/builds.rs   BuildArtifacts + PythonEnvs
  scan/caches.rs   PackageCaches + AppCaches
  scan/docker.rs
  scan/models.rs   Ollama + Hugging Face
  scan/apps.rs
  scan/downloads.rs
  scan/trash.rs
  actions.rs       execute(item, tools, dry_run) -> Outcome
  ui/mod.rs        interactive flow and --report
  ui/share.rs      share card + X intent URL
tests/
  common/mod.rs    fixture helpers (temp git repos, fake projects)
```

All scanners have the signature `pub fn scan(ctx: &ScanContext) -> Vec<Item>` and must never panic: on any error they skip the item.

## Module specs

### Core (`size.rs`, `disk.rs`, `tools.rs`, `log.rs`, `config.rs`, `scan/mod.rs::scan_all`)

- `disk_usage(paths)` walks folders in parallel with rayon, reads each folder with one `getattrlistbulk` call (falling back to `read_dir` plus `lstat` when a filesystem rejects it), sums allocated bytes, never follows symlinks, dedupes inodes with `nlink > 1` by `(dev, ino)`, and ignores unreadable entries.
- `format_bytes` uses decimal units like Finder: `1.2 GB`, `845 MB`, `12 KB`, `0 B`. One decimal under 100, none from 100 up.
- `free_bytes(path)` returns available bytes for the volume holding `path`. On macOS the user data volume is `/System/Volumes/Data`; callers pass the home dir.
- `Tools::discover()` resolves every name in `KNOWN_TOOLS` using rule 11, in parallel. Never panics; missing tools are simply absent.
- `log::record` appends `YYYY-MM-DD HH:MM:SS  <line>` to `~/Library/Logs/diskdiet.log`, creating the folder if needed, ignoring IO errors. Use `std::time::SystemTime` and format the timestamp without extra crates (UTC is fine).
- `Config::default()`: `dev_roots` empty (filled at runtime by `default_dev_roots`), `exclude` empty, `extra_cache_dirs` empty, `stale_app_days = 90`, `stale_download_days = 90`, `stale_node_modules_days = 30`, `docker_prune_until_hours = 720`, `low_space_warning_gb = 100`.
- Add `pub fn default_dev_roots(home: &Path) -> Vec<PathBuf>` returning whichever of these exist: `Develop, Developer, dev, code, Code, Projects, projects, src, workspace, repos, GitHub, git`.
- `load(explicit, home)`: explicit path must exist (error otherwise); default path is optional. `~` at the start of any path is expanded. If `dev_roots` is empty after loading, fill it with `default_dev_roots`.
- `scan_all` runs `SCANNERS` with rayon, flattens, drops items with `bytes == 0` unless the action is `PruneWorktrees` or `EmptyTrash`, drops items whose path is under any `config.exclude`, and sorts by category, then risk, then bytes descending.

### Worktrees (`scan/worktrees.rs`)

- Find git repos under `config.dev_roots` (directories that contain a `.git` **directory**), skipping `node_modules`, `target`, `.git` internals, and not descending into a repo's own `.claude/worktrees` or `.worktrees` folders.
- For each repo, parse `git worktree list --porcelain` (expose `pub fn parse_porcelain(text: &str) -> Vec<WorktreeEntry>` for tests). Skip the main worktree and detached/bare entries.
- Base branch: `origin/HEAD` symbolic ref, else the first of `main`, `master`, `development`, `develop` that exists.
- For each linked worktree compute: commits ahead of base, dirty count (porcelain without ignored), risky ignored count (rule 4), size, last commit relative date.
- Risk: `Safe` when ahead == 0 and dirty == 0 and risky ignored == 0; `Review` when ahead > 0 and dirty == 0 and risky ignored == 0; `Danger` otherwise. Preselect only `Safe`. `Danger` sets `typed_confirmation = Some("delete")`.
- Label: branch name. Detail, for example: `merged · 0 ahead · 7 uncommitted files · 3.8 GB · last commit 9 days ago · <path relative to home>`.
- Action: `RemoveWorktree { repo, worktree }`. Missing folders: rule 6.
- Expose `pub fn assess(ahead: u32, dirty: u32, risky_ignored: u32) -> Risk` and `pub fn is_build_output(path: &str) -> bool` for tests.

### Builds (`scan/builds.rs`)

- Walk `config.dev_roots` (and every worktree path is already inside them or ignored), prune at `node_modules` (but measure it, see below), `.git`, and at every match so nested matches are not double-counted.
- Matches: rule 1 for `target/`, `.next/`, `.turbo/` (whole folder), plus `node_modules` when the project's `package.json` was not modified in `stale_node_modules_days` (Review, not preselected), plus `.venv` (category `PythonEnvs`, rule 2).
- Expose `pub fn classify(dir: &Path) -> Option<(Category, Risk)>` for tests.
- Label: path relative to home. Detail: kind + size + last modified. Action: `DeletePath`.
- `target/`, `.next/`, `.turbo/` are `Safe` and preselected.

### Caches (`scan/caches.rs`)

- Package caches (category `PackageCaches`, Safe, preselected): uv (`RunCommand uv cache prune`, size of `uv cache dir`), pnpm (`RunCommand pnpm store prune`, size of `pnpm store path`), npm (`~/.npm/_cacache`, DeletePath), cargo registry cache (`~/.cargo/registry/cache`, DeletePath), Homebrew (`RunCommand brew cleanup --prune=all`, size of `brew --cache`), pip (`~/Library/Caches/pip`), bun (`~/.bun/install/cache`), Yarn (`~/Library/Caches/Yarn`), Gradle (`~/.gradle/caches`), Go build (`~/Library/Caches/go-build`), Xcode DerivedData (`~/Library/Developer/Xcode/DerivedData`), plus `config.extra_cache_dirs`.
- App caches (category `AppCaches`): one item per subfolder of `~/Library/Caches` bigger than 50 MB, excluding Apple system folders (`com.apple.*`, `CloudKit`, `GeoServices`) and the package caches above. Known regenerable ones (`ms-playwright`, `electron`, `electron-builder`, `node-gyp`, anything ending in `-updater`, `Google`, `Firefox`, `Steam`) are Safe and preselected; the rest are Review and unselected.
- Expose `pub fn app_cache_risk(name: &str) -> Option<Risk>` (None means excluded) for tests.

### Docker (`scan/docker.rs`)

- Rule 8. If `docker info` fails, return nothing.
- Parse `docker system df --format '{{json .}}'` (one JSON object per line with `Type`, `Reclaimable`, `Size`). Emit two items: unused images (`docker image prune -af --filter until=<hours>h`) and build cache (`docker builder prune -af --filter until=<hours>h`), Safe, preselected, sized from `Reclaimable` (parse strings like `37.23GB (49%)`, `17.74GB`, `512MB`, `0B`).
- If `colima` is available and running, add a third Safe item that runs `colima ssh -- sudo fstrim -a` with 0 bytes and the detail "returns freed space inside the VM to macOS". It must run after the prunes, so give it the label `Reclaim VM disk space` and place it last.
- Expose `pub fn parse_docker_size(text: &str) -> Option<u64>` for tests.

### Models (`scan/models.rs`)

- Ollama: parse `ollama list` (skip header; columns NAME, ID, SIZE, MODIFIED). One Review item per model, `RunCommand ollama rm <name>`. Skip silently if ollama is not installed or the server is down.
- Hugging Face: one Review item per `~/.cache/huggingface/hub/models--*` folder, label `org/name`, `DeletePath`.
- Expose `pub fn parse_ollama_list(text: &str) -> Vec<(String, u64)>` for tests.

### Apps (`scan/apps.rs`)

- `/Applications/*.app` and `~/Applications/*.app`. Last used from `mdls -raw -name kMDItemLastUsedDate`. `(null)` means never recorded.
- Emit a Review item when last used is older than `stale_app_days` or never recorded. Never preselected. Skip `Safari.app`, everything under `/System`, and the app running diskdiet's terminal (`$TERM_PROGRAM` mapping: `iTerm.app`, `Apple_Terminal` to `Terminal.app`, `ghostty`, `WezTerm`).
- Action `MoveToTrash`. Detail: `last opened 143 days ago` or `never opened (per Spotlight)`.

### Downloads (`scan/downloads.rs`)

- Top level of `~/Downloads`: `.dmg`, `.pkg`, `.iso` older than a week are Safe and preselected (installers), newer ones are Review; anything else older than `stale_download_days` is Review.
- Action `MoveToTrash`.

### Trash (`scan/trash.rs`)

- One item, size of `~/.Trash` (reading works without Full Disk Access), action `EmptyTrash`, Review, not preselected. If the size is 0 emit nothing.

### Actions (`actions.rs`)

- `DeletePath`: `std::fs::remove_dir_all` or `remove_file`. EPERM or EACCES leads to `NeedsFullDiskAccess`.
- `MoveToTrash` and `EmptyTrash`: `osascript -e 'tell application "Finder" to delete POSIX file "<path>"'` and `tell application "Finder" to empty trash`. Escape quotes and backslashes in paths.
- `RunCommand`: run with stdout and stderr captured; non-zero exit leads to `Failed(stderr tail)`.
- `RemoveWorktree`: recheck the worktree, then `git -C <repo> worktree remove [--force] -- <worktree>`, with `--force` only for typed-confirmed Danger items. No trailing prune. Never delete the branch.
- `PruneWorktrees`: `git -C <repo> worktree prune`.
- `dry_run` short-circuits to `Outcome::DryRun` before touching anything. Every non-dry outcome is logged (rule 14).

### UI (`ui/mod.rs`, `ui/share.rs`)

Flow, all copy in English:

1. `cliclack::intro("diskdiet")`, measure `free_bytes` before.
2. Spinner "Weighing your disk..." while `Tools::discover` + `scan_all` run.
3. A `cliclack::note` titled "You can lose up to <total>" listing each category with its total and item count, sorted by size.
4. For each non-empty category, in `Category::ALL` order: a `multiselect` titled `<title> · <total>` with the blurb as a remark, values are item indexes, labels are item labels with size, hints are details, `initial_values` = preselected items, `required(false)`, `max_rows(12)`, `filter_mode()` when there are more than 12 items. Danger items show a red marker in the label.
5. For selected items with `typed_confirmation`, ask with `cliclack::input` to type the word; skip the item if it does not match.
6. Final `confirm` "Delete <n> items (<total>)?" defaulting to false. In dry run the wording is "Dry run: nothing will be deleted. Continue?".
7. Execute with a progress bar, Docker fstrim last. Collect outcomes. If any `NeedsFullDiskAccess`, print how to grant Full Disk Access to the terminal (System Settings > Privacy & Security > Full Disk Access).
8. Measure `free_bytes` after and `outro` with "Lost <freed>. Your disk is now <used>/<total> (<pct>% free)". Then, unless `--no-share`, print the share card from `share.rs`.
9. Ctrl-C at any prompt exits cleanly with `outro_cancel("Diet postponed. Nothing was deleted.")`.

`--report` skips prompts and prints a plain table (category, item, size, risk) or, with `--json`, a JSON array. Low free space (below `low_space_warning_gb`) shows a warning in the intro.

`share.rs`: `tweet_url(freed)` returns `https://x.com/intent/post?text=<urlencoded>` with text `I just freed <freed> on my Mac with diskdiet. My AI agents had been hoarding build artifacts.\n\nhttps://github.com/brian-rey-development/diskdiet` (percent-encode by hand, no extra crate). Also `pub fn card(freed: u64, before_free: u64, after_free: u64) -> String` returns a small boxed text card for screenshots.

## Code standards

- Rust 2024, `unsafe` only in `src/size/bulk.rs`, no `unwrap()` outside tests, `anyhow` for errors at the edges.
- Functions of 20 lines or fewer (enforced by clippy `too_many_lines`), named constants instead of magic numbers and strings, early returns, immutability by default.
- No comments unless the why is non-obvious. No doc comments except on `cli.rs` args (they become `--help`).
- `cargo fmt`, `cargo clippy --all-targets -- -D warnings` and `cargo test` must pass.

## Ownership for parallel work

| Agent | Owns |
|---|---|
| core | `src/size.rs`, `src/disk.rs`, `src/tools.rs`, `src/log.rs`, `src/config.rs`, `src/scan/mod.rs` (only `scan_all`), `tests/core.rs` |
| dev-scanners | `src/scan/worktrees.rs`, `src/scan/builds.rs`, `tests/common/mod.rs`, `tests/worktrees.rs`, `tests/builds.rs` |
| system-scanners | `src/scan/caches.rs`, `src/scan/docker.rs`, `src/scan/models.rs`, `src/scan/apps.rs`, `src/scan/downloads.rs`, `src/scan/trash.rs`, `tests/system_scanners.rs` |
| ux | `src/actions.rs`, `src/ui/mod.rs`, `src/ui/share.rs`, `tests/actions.rs` |
| docs | `README.md`, `setup.sh`, `scripts/*`, `config/diskdiet.example.toml`, `docs/ARCHITECTURE.md`, `docs/SAFETY.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `LICENSE` |

`src/model.rs`, `src/cli.rs`, `src/lib.rs`, `src/main.rs`, `Cargo.toml` and this plan are the shared contract. Agents do not edit them; if a change is needed, they say so in their final report.
