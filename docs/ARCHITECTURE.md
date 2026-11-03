# Architecture

diskdiet is a single Rust binary with a small library behind it, split into four layers so that the code deciding what is junk can never delete anything. Scanners return plain `Item` values and only `features/actions/` mutates the disk, so scanners can be tested on fixture folders safely.

## Module map

```
src/
  main.rs                 parse Args, init the thread pool, call app::ui::run
  lib.rs

  app/                    wiring and the terminal, the only layer that sees everything
    cli.rs
    session.rs            settings, rayon pool cap, tool discovery
    ui/
      mod.rs
      notices.rs
      scan_progress.rs
      summary.rs
      entries.rs
      pick.rs
      review.rs
      layout.rs
      theme.rs
      exec.rs
      finish/
      report.rs
      share.rs

  domain/                 what diskdiet believes, independent of scanning and deleting
    model.rs              Category, Risk, Action, Item, Outcome, RunMode
    safety.rs             is_safe_delete_target, the guard for every deletion
    config.rs

  features/               the two halves of a run, which never import each other
    scan/                 decides what is junk, never deletes
      mod.rs              ScanContext, Scanner, SCANNERS
      pipeline.rs         size floor, exclusions, risk inheritance
      support.rs
      worktrees/
        mod.rs
        discover.rs
        assess.rs
        items.rs
      builds.rs
      caches/
        mod.rs
        package.rs
        app.rs
        owners.rs
      docker.rs
      models.rs
      apps/
      downloads.rs
      trash.rs
    actions/              the only code that mutates the disk
      mod.rs              execute
      delete.rs
      trash.rs
      command.rs
      worktree.rs

  infra/                  the operating system, knows nothing about junk
    fs/
      paths.rs
      disk.rs             free_bytes via statvfs
      size/
        mod.rs            disk_usage
        bulk.rs           getattrlistbulk reader (macOS only), the only module allowed unsafe
      freedesktop_trash.rs
    sys/
      process.rs          run_with_deadline
      tools.rs            Tools::discover
      platform.rs         per-OS paths
      log.rs
    git/                  program lookup, command builder, parsers
      mod.rs
      porcelain.rs
      status.rs
      ignored.rs          is_build_output allow-list
    util/
      time.rs
      fmt.rs

tests/
  it/main.rs              the single integration test binary, its tree mirrors src/
    architecture.rs       enforces the layer rules below
    support/              fixtures and builders
    app/  domain/  features/  infra/
```


Imports run `app` to `features` to `domain` to `infra`, and `tests/it/architecture.rs` fails the build when a file imports upward. `infra` never mentions an `Item`, `domain` borrows only path helpers from below, and `app` alone sees every layer because it wires them into one run. Inside `features`, `scan` and `actions` never import each other, so a scanner cannot delete and an action cannot decide what is junk. The test also rejects grouped `use crate::{...}` imports and `super::super` paths, which would hide a crossing from a text check.

An `Item` has a `Category`, a `Risk` (`Safe`, `Review` or `Danger`), a size in bytes, a preselection flag, an optional typed confirmation word and the `Action` that will run. `actions::execute` turns an `Action` into an `Outcome` (`Done`, `DryRun`, `Skipped`, `Failed`, `NeedsFullDiskAccess` or `Stopped`).

## Data flow

**1. Discover tools.** `Tools::discover` resolves every external program once, in parallel, trying `PATH`, then the login shell's `PATH` (`$SHELL -lic`, under a timeout), then `mise which <tool>`, since mise-installed tools are often invisible to a plain lookup. External commands run through `process::run_with_deadline`, which kills the process on timeout so a hung git, Docker or shell rc file cannot freeze the scan, and scanners whose tool is missing return nothing.

**2. Scan in parallel.** `scan_all` runs every `Scanner` in `SCANNERS` through rayon on a pool capped at eight threads, each with a shared read-only `ScanContext` of config, home directory and tools. `scan_with_progress` also reports each scanner's items as it finishes, for the UI's running totals. A panicking scanner is caught, logged and skipped. Each scanner's output drops items under 1 MB (except worktrees, worktree pruning, the Trash and the Colima disk trim) and anything under `config.exclude`. After flattening, a `Safe` item containing a riskier one inherits that risk, `--only` filters categories, and the list sorts by category, risk, then size descending.

**3. Select.** The UI opens one multiselect per non-empty category in `Category::ALL` order, with type-to-filter above ten items. `preselected` items start ticked, and items with a `typed_confirmation` ask for the word right after their category and are dropped on a mismatch. Picks are stored as item indices, so execution order ignores navigation order. A review screen follows, then a final confirmation that defaults to no. With `--report` this stage prints a table or JSON instead.

**4. Execute.** Each selected item goes through `actions::execute(item, tools, mode)`, which returns `Outcome::DryRun` before touching anything in dry-run mode. Otherwise it deletes a path, moves it to the Trash (through Finder on macOS, the freedesktop.org layout on Linux), runs the owning tool's cleanup command or removes a worktree with git, and logs the outcome. The Docker VM trim runs last because it can only return space the prunes have released.

**5. Measure.** `disk::free_bytes` reads free space on the home volume before and after execution, and the difference is the number shown as freed.

## How to add a scanner

1. Create `src/features/scan/<name>.rs` with `pub fn scan(ctx: &ScanContext) -> Vec<Item>` that skips any item that errors, then add the module and a `Scanner` entry to `SCANNERS` in `scan/mod.rs`.
2. For a new `Category`, add it to the enum, `Category::ALL` (which sets the prompt order), `title()` and `blurb()` in `domain/model.rs`.
3. Measure with `size::disk_usage`, never file length or `du`, and pick the `Action` that frees space immediately (rule 10 in [SAFETY.md](SAFETY.md)).
4. Mark an item `Safe` and preselected only when you can prove it is regenerable. Anything that might hold user data is `Review` or `Danger`.
5. Expose the decision logic as a `pub fn` and add fixture tests in `tests/it/features/scan/<name>.rs`, registered in the `mod.rs` beside it.
6. Resolve external programs through `Tools` and emit nothing when one is missing.

## Why disk usage counts allocated bytes and dedupes inodes

The size next to an item has to match what the user gets back, so `disk_usage` sums allocated space (`ATTR_FILE_ALLOCSIZE`, or `st_blocks * 512` on the fallback path, which is what `du` reports). File length is wrong both ways, since a sparse Docker or Colima disk image reports far more than it occupies and a small file occupies a full block.

A pnpm store and a Rust `target/` folder are full of hardlinks, and counting each name would report space that deleting one folder never releases. So any entry with `nlink > 1` is recorded by `(dev, ino)` in one set shared across all paths in a call, and only its first sighting counts. Symlinks are never followed, since the target could lie outside the folder or loop, and deleting a link frees only the link.

## Why the walker calls `getattrlistbulk` directly

Scanning is bound by how fast the kernel hands out metadata, so the walker should make few system calls. Linux uses `readdir` plus one `lstat` per entry over a rayon pool, while macOS `getattrlistbulk` fills a 128 KB buffer with name, type, inode, link count and allocated size for many entries in one call, and on a 72 GB folder of projects it took 9.5 seconds against 15.7 for the portable walker, with identical totals. Because it needs `unsafe` FFI, it lives alone in `infra/fs/size/bulk.rs`, compiled only on macOS, where the `getattrlistbulk` call is the one unsafe block and the buffer is parsed with bounds-checked slices. If a parse fails or a file system rejects the call, that folder is read again with `read_dir` and `lstat`, and a test checks the result against `du`.
