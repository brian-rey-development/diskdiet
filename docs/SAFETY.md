# Safety

A disk cleaner exists to delete things, so its trust has to come from rules, most of which came from something that happened or nearly happened on a real Mac. Every rule has a test, and weakening a rule means changing its test first.

## 1. A cache tag alone never marks a folder as disposable

uv writes a `CACHEDIR.TAG` inside `.venv` and some model downloaders write one inside model folders, so trusting the tag would delete things you think of as real. A build folder is recognised by its name plus a manifest beside it. `target/` needs a `Cargo.toml`, `.next/` a `package.json`, and `.turbo/cache` a `package.json` or `turbo.json`.

## 2. Virtualenvs are their own category and never preselected

A `.venv` is regenerable only if the project can still recreate it, which diskdiet cannot know. So `.venv` folders start unticked and carry the hint "run `uv sync` to recreate".

## 3. "Merged" does not mean "safe"

A worktree with zero commits ahead of its base looks finished but can still hold a day of uncommitted changes. Risk combines commits ahead, dirty files and ignored files that are not known build output, and only a worktree clean on all three is `Safe`, one that is merely ahead is `Review`, and the rest are `Danger`. Files marked `--skip-worktree` or `--assume-unchanged` hide from `git status`, so `git ls-files -v` finds them and they count as dirty. Submodules make a worktree `Danger`, because git only removes it with `--force`.

## 4. Ignored files count

`git status --porcelain` hides ignored files, so a worktree holding only a `.env` with API keys looks clean and the cleaner removes the only copy. diskdiet adds `--ignored=matching` and excuses only the output listed in `src/infra/git/ignored.rs`, which always includes `node_modules/`, `.next/`, `.turbo/`, `__pycache__/`, `.venv/`, `.svelte-kit/`, `.nuxt/`, `.tanstack/`, `.parcel-cache/` and the pytest, ruff and mypy caches. Names people also use for their own files (`target/`, `dist/`, `build/`, `coverage/`, `.cache/`) are excused only beside a manifest such as `package.json`, `Cargo.toml`, `pyproject.toml`, `go.mod` or `Makefile`, so `build/` next to a `package.json` is output while `deploy/build/signing.pem` is not. Generated files like `next-env.d.ts`, `*.tsbuildinfo`, `*.pyc` and `.DS_Store`, and folders like `.husky/_/`, are excused too. Anything else makes the worktree `Danger`.

## 5. Dangerous worktrees need a typed confirmation

A tick is too easy to hit in a long list, so a `Danger` worktree is never preselected and is included only after you type `delete`, and any other input skips it. Ticking one while editing a category from the review screen asks again. Since the owning agent may still be writing minutes after the scan, only a worktree you confirmed by typing is removed with `git worktree remove --force`. The others are rechecked right before removal, skipped with "it changed since the scan" if anything new appeared, and removed without `--force`, so git itself refuses on uncommitted files.

## 6. Worktrees whose folder is gone are pruned, not deleted

When a worktree's folder was removed by hand but git still lists it, diskdiet offers one zero-byte `Safe` item per repo that runs `git worktree prune`. Prune also forgets worktrees on an unplugged external drive, so none is offered when a worktree sits on an unmounted volume (under `/Volumes`, `/media`, `/run/media` or `/mnt`), and removing a single worktree never prunes. The check repeats before the prune using `git worktree list --porcelain` with a timeout, and a mount counts only if it is on a different device than its parent, since an unmounted `/mnt/x` is an empty folder. If the listing fails, the prune is skipped.

## 7. Worktrees can live anywhere

Coding agents create worktrees in places like `~/orca/workspaces/...`, outside your project folders, so scanning only the dev roots would miss the biggest ones. diskdiet finds repos in the roots and then follows `git worktree list` wherever it points.

## 8. diskdiet never starts Docker or Colima

Starting a container runtime just to measure it can take minutes and gigabytes of memory, so diskdiet inspects Docker only when `docker info` already succeeds, and otherwise says "Docker is not running, skipped". The endpoint must be a local socket, so a remote `DOCKER_HOST` is never pruned. Only unused images and build cache are pruned, with images marked `Review` since `docker image prune -a` removes local builds you cannot pull again. Containers and volumes are never touched.

## 9. Permission errors are expected, and explained

On macOS, deleting inside `~/.Trash` needs Full Disk Access and root-owned apps such as iMovie cannot be moved with plain file APIs, so trashing and emptying go through Finder. A delete that fails for permissions reports `NeedsFullDiskAccess` with the System Settings path to grant it. On Linux the Trash is the freedesktop.org one in `~/.local/share/Trash`, restorable from file managers, and a file on another drive is reported instead of copied.

## 10. Regenerable data is removed in a way that frees space now

Moving a cache to the Trash frees nothing until it is emptied, and "freed 40 GB" over an unchanged disk loses trust. Regenerable data is therefore deleted directly or through the owning tool (`uv cache prune`, `pnpm store prune`, `docker image prune`, `brew cleanup`), which also knows references a plain delete would break. Only apps and downloads go to the Trash, since you may want them back, and "Empty the Trash" runs before anything is moved there so that the same run's trashed items survive.

## 11. Tools are found the way your shell finds them

A launched binary inherits a minimal `PATH` that lacks pnpm when mise provides it, so a naive lookup skips its store or runs another version. diskdiet checks `PATH`, then your login shell's `PATH` once (`$SHELL -lic`, with a timeout), then `mise which <tool>`.

## 12. The number on screen must be true

A cleaner that promises 287 GB and delivers 180 GB teaches people to ignore it. Sizes are real disk allocation, as `du` counts, so sparse VM images count for what they occupy, hardlinks count once per `(device, inode)` and symlinks are never followed. The final "freed" figure is the free-space difference from `statvfs` before and after, never the estimate. See [ARCHITECTURE.md](ARCHITECTURE.md).

## 13. Dry run deletes nothing

`--dry-run` short-circuits every action before it touches the disk and returns `DryRun`. Prompts, sizes and the summary match a real run, and the confirmation text says nothing will be deleted.

## 14. Everything diskdiet does is logged

Every executed action is appended with a timestamp and outcome to `~/Library/Logs/diskdiet.log` on macOS or `~/.local/state/diskdiet/diskdiet.log` on Linux, so you can tell what removed something and when. A failure to write the log never blocks the cleanup.

## 15. Some paths can never be deleted

Every delete and move to the Trash passes one guard, whatever the scanner or config says. It refuses relative paths and `..`, your home folder and everything above it (`/Users`, `/`), anything directly inside your home (`~/Documents`, `~/.cache`, `~/Library`), `/Users` or `/home`, and every mount root and mount point with its parents (`/Volumes/X`, `/mnt/x`, `/media/me/Drive`, `/run/media/me`). Case is ignored, as macOS volumes mostly do. A config line such as `extra_cache_dirs = ["~"]` produces no item at all.

## 16. Context can only make an item riskier

The cache of a running app (Chrome, Firefox, Steam, or Xcode for DerivedData) drops to `Review`, and diskdiet asks `ps` again before deleting it, skipping the item if the app runs or `ps` fails, since an unreadable process list is never read as empty. An installer in Downloads is preselected only after a week, since a fresh `.dmg` or `.iso` may be one you are about to use. When one item contains another, the outer item takes the riskiest risk inside, so ticking a `Safe` parent cannot quietly delete a `Review` child, even under `--only`, because the filter applies after risk is inherited.

## Reporting a safety problem

If diskdiet removed something it should not have, or offered a `Safe` item that was not, open an issue with the log lines and a description of the folder. A new incident becomes a new rule and a new test.
