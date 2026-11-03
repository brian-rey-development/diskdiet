# Architecture

diskdiet is a single Rust binary with a small library behind it. The library is split so that everything that decides what is junk (the scanners) is separate from everything that touches the disk destructively (`actions.rs`), and both talk through one shared vocabulary in `model.rs`. That split is what makes the safety rules testable, because a scanner can be checked against a fixture folder without ever being able to delete it.

## Module map

```
src/
  main.rs          parse Args, call ui::run
  lib.rs           module list
  cli.rs           clap Args (their doc comments become --help)
  model.rs         Category, Risk, Action, Item, Outcome (the shared contract)
  config.rs        Config with defaults, TOML load, default_dev_roots
  size/mod.rs      disk_usage (parallel, allocated bytes, inode dedupe), format_bytes
  size/bulk.rs     getattrlistbulk directory reader, the only module allowed unsafe
  disk.rs          free_bytes via statvfs
  tools.rs         Tools::discover resolves external programs once, in parallel
  log.rs           append-only action log
  scan/mod.rs      ScanContext, SCANNERS, scan_all
  scan/worktrees.rs
  scan/builds.rs   BuildArtifacts and PythonEnvs
  scan/caches.rs   PackageCaches and AppCaches
  scan/docker.rs
  scan/models.rs   Ollama and Hugging Face
  scan/apps.rs
  scan/downloads.rs
  scan/trash.rs
  actions.rs       execute(item, tools, dry_run) -> Outcome
  ui/mod.rs        interactive flow and --report
  ui/share.rs      share card and X intent URL
tests/
  common/mod.rs    fixture helpers (temp git repos, fake projects)
```

The contract lives in `model.rs`. A scanner returns `Vec<Item>`, where an `Item` has a `Category`, a `Risk` (`Safe`, `Review` or `Danger`), a measured size in bytes, a preselection flag, an optional typed confirmation word and an `Action` describing exactly what will happen. `actions.rs` turns an `Action` into an `Outcome` (`Done`, `DryRun`, `Skipped`, `Failed` or `NeedsFullDiskAccess`). Nothing else mutates the disk.

## Data flow

A run has five stages, and each one hands plain data to the next.

**1. Discover tools.** `Tools::discover` resolves every external program diskdiet may call (git, uv, pnpm, brew, docker, colima, ollama, mise) once, in parallel. Resolution follows the user's shell rather than the environment the binary inherited, because a tool installed through mise is often invisible to a plain `PATH` lookup. It checks `PATH`, then the `PATH` of the user's login shell (read once with `$SHELL -lic`, under a timeout), then `mise which <tool>`. Every external command runs through `tools::run_with_deadline`, which reads output on its own thread and kills the process on timeout, so a hung git, Docker or shell rc file cannot freeze the scan. Missing tools are simply absent, and scanners that need them return nothing.

**2. Scan in parallel.** `scan_all` runs every function in `SCANNERS` through rayon, each with a shared read-only `ScanContext` (config, home directory, tools). A scanner has the signature `fn scan(ctx: &ScanContext) -> Vec<Item>` and never panics, so any error means that item is skipped. The results are flattened, items under 1 MB are dropped to keep the lists readable (except worktrees, worktree pruning, the Trash and the Colima disk trim, which matter regardless of size), anything under `config.exclude` is dropped, and the list is sorted by category, then risk, then size descending.

**3. Select.** The UI groups items by `Category::ALL`, shows a summary, and opens one multiselect per non-empty category. Items marked `preselected` start ticked. Items with `typed_confirmation` ask the user to type the word, and are skipped if it does not match. A final confirmation, which defaults to no, closes the step. With `--report` this stage is replaced by a printed table or JSON.

**4. Execute.** Each selected item goes through `actions::execute(item, tools, dry_run)`. With `dry_run` set, the function returns `Outcome::DryRun` before touching anything. Otherwise it deletes a path, moves it to the Trash through Finder, runs the owning tool's own cleanup command, or removes a worktree with git, and the outcome is appended to `~/Library/Logs/diskdiet.log`. The Docker VM trim is ordered last, because it can only return space that the prunes have already released.

**5. Measure.** `disk::free_bytes` reads the free space of the volume holding the home directory before the scan and again after execution. The difference is the number the user sees as "freed". No estimate is ever reported as a result.

## How to add a scanner

1. Create `src/scan/<name>.rs` with `pub fn scan(ctx: &ScanContext) -> Vec<Item>`. On any error, skip the item and keep going.
2. Add the module in `src/scan/mod.rs` and add the function to `SCANNERS`.
3. If your items need a new `Category`, add it to the enum, to `Category::ALL`, and to `title()` and `blurb()` in `model.rs`. Order in `ALL` is the order the user sees the prompts.
4. Measure with `size::disk_usage`, never with file length or `du`. Pick the `Action` that makes the freed space real immediately (see rule 10 in [SAFETY.md](SAFETY.md)).
5. Decide the risk honestly. Only items you can prove are regenerable are `Safe` and preselected. Anything that might hold user data is `Review` or `Danger` and starts unticked.
6. Expose the pure decision logic (a classifier, a parser) as a `pub fn` so tests can call it without a real machine, and add fixture-based tests in `tests/`.
7. If your scanner runs an external program, resolve it through `Tools`, and emit nothing when it is missing or not running.

## Why disk usage counts allocated bytes and dedupes inodes

The size shown next to an item has to match what the user gets back, so `disk_usage` sums the space the file system has actually allocated (`ATTR_FILE_ALLOCSIZE` from `getattrlistbulk`, or `st_blocks * 512` on the fallback path, which are the same number and what `du` reports). File length is wrong in both directions. A sparse file, such as a Docker or Colima disk image, reports a length far above what it occupies, and a file system that allocates in blocks always uses more than the length of a small file.

Hardlinks need the same care. A pnpm store and a Rust `target/` folder are full of files that exist under several names, and counting each name would report space that deleting one folder will never release. So any entry with `nlink > 1` is recorded by `(dev, ino)` in one shared set for the whole measurement, and only its first sighting is counted. The set is shared across all paths in a single call, because two folders that share inodes should not both claim them.

Symlinks are never followed. Following one could count a target outside the folder (or loop), and deleting a symlink frees nothing beyond the link itself.

## Why the walker calls `getattrlistbulk` directly

Scanning is bound by how fast the kernel can hand out file metadata, not by CPU, so the walker's job is to make as few system calls as possible. A portable walker calls `readdir` and then one `lstat` per entry. macOS has `getattrlistbulk`, which fills a 128 KB buffer with the name, type, inode, link count and allocated size of many entries in a single call, and on a 72 GB folder of projects it measured 9.5 seconds against 15.7 for the portable walker, with identical totals. The call needs `unsafe` FFI, so it lives alone in `size/bulk.rs`, where only `open`, `getattrlistbulk` and `close` are unsafe and the returned buffer is parsed through bounds-checked slices. If a parse fails or a file system rejects the call, that folder is read again with `read_dir` and `lstat`, and a test checks the result against `du` on a folder large enough to need several batches.

The estimate is still an estimate, which is why the final figure is not computed from it. After execution diskdiet compares `statvfs` free space before and after, and that difference is what it reports.
