# Architecture

diskdiet is a single Rust binary with a small library behind it. The library is split so that everything that decides what is junk (the scanners) is separate from everything that touches the disk destructively (`actions/`), and both talk through one shared vocabulary in `model.rs`. That split is what makes the safety rules testable, because a scanner can be checked against a fixture folder without ever being able to delete it.

## Module map

```
src/
  main.rs               parse Args, init the thread pool, call ui::run
  lib.rs                module list
  cli.rs                clap Args (help text lives in #[arg(help = ...)])
  model.rs              Category, Risk, Action, Item, Outcome, RunMode (the shared contract)
  session.rs            load settings, cap the rayon pool, discover tools and run the scanners

  time.rs               clock helpers, calendar math, format_timestamp
  fmt.rs                format_age, tilde_path, parse_size, count_noun and plural_s
  fsutil.rs             lowercased, child_dirs, canonical_or_self, remove_any
  platform.rs           per-OS paths (log, Trash, app caches), home_dir, volume_is_mounted
  config.rs             Config with defaults, TOML load, default_dev_roots, expand_tilde
  disk.rs               free_bytes and total_bytes via statvfs
  log.rs                append-only action log
  process.rs            run_with_deadline, run_stdout and run_shielded (external commands)

  tools.rs              Tools::discover resolves external programs, PATH first
  git/                  git program lookup and command builder, plus the pure parsers for its output
    mod.rs              program lookup and the git command builder
    porcelain.rs        parse_porcelain for git worktree list
    status.rs           count_status, hidden edits and local_changes
    ignored.rs          is_build_output, the allow-list of ignored files that are not user data
  size/
    mod.rs              disk_usage (parallel, allocated bytes, inode dedupe), format_bytes
    bulk.rs             getattrlistbulk directory reader (macOS only), the only module allowed unsafe
  safety.rs             is_safe_delete_target, the guard every deletion passes through
  freedesktop_trash.rs  move to and empty the Linux Trash per the freedesktop.org spec

  scan/                 decides what is junk, never deletes
    mod.rs              ScanContext, Scanner, SCANNERS, scan_all, scan_with_progress (per-scanner callback)
    pipeline.rs         size floor, exclusions, risk inheritance, sort order
    support.rs          helpers shared by the scanners
    worktrees/          agent worktrees and stale worktree records
      mod.rs            scan, per-repo orchestration
      discover.rs       find_repos under the dev roots
      assess.rs         Assessment, risk_from_counts, base_branch
      items.rs          turns assessments into Items
    builds.rs           BuildArtifacts and PythonEnvs
    caches/             PackageCaches and AppCaches
      mod.rs            scan, one ps call shared by both halves
      package.rs        package manager caches and extra_cache_dirs
      app.rs            ~/Library/Caches and ~/.cache
      owners.rs         is the owning app running right now
    docker.rs
    models.rs           Ollama and Hugging Face
    apps.rs             unused apps, batched mdls
    downloads.rs
    trash.rs
  actions/              the only code that mutates the disk
    mod.rs              execute, perform, outcome logging
    delete.rs           DeletePath
    trash.rs            MoveToTrash and EmptyTrash
    command.rs          RunCommand
    worktree.rs         RemoveWorktree and PruneWorktrees

  ui/                   interactive flow and --report
    mod.rs              run, interactive flow
    notices.rs          intro, live scan spinner and Docker notices
    scan_progress.rs    running totals per category and the spinner line
    summary.rs          the per-category summary
    entries.rs          per-category row order, worktree grouping, row labels, Selection
    pick.rs             multiselects, typed confirmations and the review loop
    review.rs           review screen, review and category prompts
    layout.rs           terminal width and truncation helpers
    theme.rs            cliclack theme that puts the item detail on its own line
    exec.rs             execution order and progress
    finish/             freed-space measurement, problems and the share card
    report.rs           --report table and JSON
    share.rs            share card and X intent URL

tests/
  it/main.rs            the single integration test binary
    support/            shared helpers: git fixtures, item and config builders, shared constants
    size.rs  disk.rs  config.rs  log.rs  time.rs  fmt.rs  tools.rs  platform.rs
    freedesktop_trash.rs  safety.rs
    git/                porcelain, status, ignored
    actions/            delete, command, trash, worktree, dry_run
    scan/               pipeline, builds, docker, models, apps, downloads, caches
    scan/worktrees/     assess, discover, scan, prune
    ui/                 entries, exec, layout, notices, pick, report, review, scan_progress, share
```

The modules form layers, and imports only point downward. The leaf modules (`time`, `fmt`, `fsutil`, `platform`, `config`, `disk`, `log`, `model` and `process`) depend on nothing above them. On top of those sit `tools`, `git`, `size`, `safety` and `freedesktop_trash`, which each wrap one external concern. `scan` and `actions` come next and never import each other, which is why a scanner cannot delete anything and an action cannot decide what is junk. Both read only the shared vocabulary in `model.rs`. `session`, `ui` and `cli` sit on top and wire everything together. The test tree mirrors this layout, so a test for `scan/worktrees/assess.rs` lives in `tests/it/scan/worktrees/assess.rs`.

The contract lives in `model.rs`. A scanner returns `Vec<Item>`, where an `Item` has a `Category`, a `Risk` (`Safe`, `Review` or `Danger`), a measured size in bytes, a preselection flag, an optional typed confirmation word and an `Action` describing exactly what will happen. `actions::execute` turns an `Action` into an `Outcome` (`Done`, `DryRun`, `Skipped`, `Failed` or `NeedsFullDiskAccess`). Nothing else mutates the disk.

## Data flow

A run has five stages, and each one hands plain data to the next.

**1. Discover tools.** `Tools::discover` resolves every external program diskdiet may call (git, uv, pnpm, brew, docker, colima, ollama, mise) once, in parallel. Resolution tries the inherited `PATH` first, which answers for most tools without starting any process, and only falls back to the user's shell for tools it did not find, because a tool installed through mise is often invisible to a plain `PATH` lookup. The fallback reads the `PATH` of the user's login shell once (with `$SHELL -lic`, under a timeout), then asks `mise which <tool>`. Every external command runs through `process::run_with_deadline`, which reads output on its own thread and kills the process on timeout, so a hung git, Docker or shell rc file cannot freeze the scan. Missing tools are simply absent, and scanners that need them return nothing.

**2. Scan in parallel.** `scan_all` (or `scan_with_progress`, which also calls a callback with the kept items of each scanner as it finishes, so the UI can show running totals) runs every `Scanner` in `SCANNERS` through rayon, on a pool capped at eight threads, each with a shared read-only `ScanContext` (config, home directory, tools). A scanner has the signature `fn scan(ctx: &ScanContext) -> Vec<Item>` and never panics, so any error means that item is skipped. The results are flattened, items under 1 MB are dropped to keep the lists readable (except worktrees, worktree pruning, the Trash and the Colima disk trim, which matter regardless of size), anything under `config.exclude` is dropped, a `Safe` item that contains a riskier one inherits that risk, `--only` then keeps just the chosen categories (after the inheritance, so every scanner still runs), and the list is sorted by category, then risk, then size descending.

**3. Select.** The UI groups items by `Category::ALL`, shows a summary, and opens one multiselect per non-empty category, with type-to-filter on lists longer than ten items and worktrees grouped by repository. Items marked `preselected` start ticked. Items with `typed_confirmation` ask the user to type the word right after their category, and are dropped from the selection if it does not match. The picks are kept as a set of item indices, so execution order does not change however the user navigates. A review screen then lists everything selected, grouped by category with subtotals and a grand total, and offers Continue, Change a category or Cancel. Changing a category reopens just that list with the current selection ticked, and only items that were not already selected ask for their typed confirmation again. A final confirmation, which defaults to no, closes the step. With `--report` this stage is replaced by a printed table or JSON.

**4. Execute.** Each selected item goes through `actions::execute(item, tools, mode)`. In dry-run mode the function returns `Outcome::DryRun` before touching anything. Otherwise it deletes a path, moves it to the Trash (through Finder on macOS, through the freedesktop.org layout on Linux), runs the owning tool's own cleanup command, or removes a worktree with git, and the outcome is appended to the log (`platform::log_path`). The Docker VM trim is ordered last, because it can only return space that the prunes have already released.

**5. Measure.** `disk::free_bytes` reads the free space of the volume holding the home directory before the scan and again after execution. The difference is the number the user sees as "freed". No estimate is ever reported as a result.

## How to add a scanner

1. Create `src/scan/<name>.rs` with `pub fn scan(ctx: &ScanContext) -> Vec<Item>`. On any error, skip the item and keep going.
2. Add the module in `src/scan/mod.rs` and add a `Scanner` entry (name, scan function) to `SCANNERS`.
3. If your items need a new `Category`, add it to the enum, to `Category::ALL`, and to `title()` and `blurb()` in `model.rs`. Order in `ALL` is the order the user sees the prompts.
4. Measure with `size::disk_usage`, never with file length or `du`. Pick the `Action` that makes the freed space real immediately (see rule 10 in [SAFETY.md](SAFETY.md)).
5. Decide the risk honestly. Only items you can prove are regenerable are `Safe` and preselected. Anything that might hold user data is `Review` or `Danger` and starts unticked.
6. Expose the pure decision logic (a classifier, a parser) as a `pub fn` so tests can call it without a real machine, and add fixture-based tests in `tests/it/scan/<name>.rs`, registered in `tests/it/scan/mod.rs`.
7. If your scanner runs an external program, resolve it through `Tools`, and emit nothing when it is missing or not running.

## Why disk usage counts allocated bytes and dedupes inodes

The size shown next to an item has to match what the user gets back, so `disk_usage` sums the space the file system has actually allocated (`ATTR_FILE_ALLOCSIZE` from `getattrlistbulk`, or `st_blocks * 512` on the fallback path, which are the same number and what `du` reports). File length is wrong in both directions. A sparse file, such as a Docker or Colima disk image, reports a length far above what it occupies, and a file system that allocates in blocks always uses more than the length of a small file.

Hardlinks need the same care. A pnpm store and a Rust `target/` folder are full of files that exist under several names, and counting each name would report space that deleting one folder will never release. So any entry with `nlink > 1` is recorded by `(dev, ino)` in one shared set for the whole measurement, and only its first sighting is counted. The set is shared across all paths in a single call, because two folders that share inodes should not both claim them.

Symlinks are never followed. Following one could count a target outside the folder (or loop), and deleting a symlink frees nothing beyond the link itself.

## Why the walker calls `getattrlistbulk` directly

Scanning is bound by how fast the kernel can hand out file metadata, not by CPU, so the walker's job is to make as few system calls as possible. A portable walker calls `readdir` and then one `lstat` per entry. On Linux diskdiet uses exactly that, spread over a rayon pool. macOS has `getattrlistbulk`, which fills a 128 KB buffer with the name, type, inode, link count and allocated size of many entries in a single call, and on a 72 GB folder of projects it measured 9.5 seconds against 15.7 for the portable walker, with identical totals. The call needs `unsafe` FFI, so it lives alone in `size/bulk.rs`, compiled only on macOS, where only `open`, `getattrlistbulk` and `close` are unsafe and the returned buffer is parsed through bounds-checked slices. If a parse fails or a file system rejects the call, that folder is read again with `read_dir` and `lstat`, and a test checks the result against `du` on a folder large enough to need several batches.

The estimate is still an estimate, which is why the final figure is not computed from it. After execution diskdiet compares `statvfs` free space before and after, and that difference is what it reports.
