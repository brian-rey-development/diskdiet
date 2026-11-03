# Safety

A disk cleaner is a program whose whole job is deleting things, so its trust has to come from rules, not from good intentions. These are the rules diskdiet follows. Most of them exist because of something that happened (or nearly happened) while cleaning a real Mac, and the ones where the plan does not record a specific incident say so plainly. Every rule has a test in the repository, and a change that weakens a rule has to change the test first.

The short version for users. Nothing is deleted without a final confirmation, risky items start unticked, `--dry-run` shows everything without deleting anything, and every real action is written to `~/Library/Logs/diskdiet.log`.

## 1. A cache tag alone never marks a folder as disposable

`CACHEDIR.TAG` is a file that tools drop into folders they consider cache, and it is tempting to treat it as permission to delete. diskdiet does not. The tag turned out to appear where it should not be trusted. uv writes one inside `.venv`, and some model downloaders write one inside model folders, so a cleaner that trusts the tag would remove environments and downloaded models that the user thinks of as real.

Instead a build folder is recognised by its name and by a manifest next to it. `target/` must sit beside a `Cargo.toml`, `.next/` beside a `package.json`, and `.turbo/cache` inside a folder with a `package.json` or `turbo.json`.

## 2. Virtualenvs are their own category and never preselected

A `.venv` is regenerable only if the project can still recreate it, and diskdiet cannot know that. So `.venv` folders live in their own category, start unticked, and carry the hint "run `uv sync` to recreate". This rule follows from rule 1. Since the tag cannot be used to call a virtualenv disposable, the decision stays with the user. The plan does not record a separate incident for this one.

## 3. "Merged" does not mean "safe"

A worktree whose branch has zero commits ahead of the base branch looks finished. It can still hold uncommitted work, and that is the case where a careless cleaner loses a day of changes. So risk is computed from three facts together, never from one: commits ahead of base, dirty tracked or untracked files, and ignored files that are not known build output. Only a worktree that is clean on all three counts is `Safe`. One that is ahead but otherwise clean is `Review`, and everything else is `Danger`. Two kinds of local work hide from `git status`, so they are checked separately. Files flagged with `git update-index --skip-worktree` or `--assume-unchanged` (a common way to keep personal config edits) are found with `git ls-files -v` and counted as dirty when they exist on disk. A worktree with submodules is always `Danger`, because status does not look inside a submodule for ignored files and git only removes such a worktree with `--force`.

## 4. Ignored files count

`git status --porcelain` hides ignored files by design. A worktree containing nothing but a `.env` file (API keys, local database URLs, things that exist nowhere else) therefore looks perfectly clean, and the cleaner removes the only copy. diskdiet asks for `git status --porcelain --ignored=matching` and excuses only entries that are themselves a known build folder (`target/`, `node_modules/`, `.next/`, `dist/`, `build/` and a few more, listed in `src/scan/worktrees.rs`), a known generated file (`next-env.d.ts`, `*.tsbuildinfo`, `*.pyc`, `.DS_Store`) or something inside a generated folder such as `.husky/_/`. A file that merely sits below a folder called `build` does not count, so an ignored `deploy/build/signing.pem` still makes the worktree `Danger`. If anything else remains, the worktree is `Danger`.

## 5. Dangerous worktrees need a typed confirmation

Ticking a box is too easy to do by accident, especially with a long list. A `Danger` worktree is never preselected, and when you select it diskdiet asks you to type the word `delete` before it is included. If the word does not match, the item is skipped. This costs a few seconds and prevents the one mistake that cannot be undone. The plan does not record a specific incident behind this rule, it is the consequence of rules 3 and 4.

Minutes can pass between the scan and the deletion, and the agent that owns a worktree may still be writing to it. So only a worktree you confirmed by typing is removed with `git worktree remove --force`. Every other worktree is checked again right before removal, skipped with "it changed since the scan" if anything new appeared, and removed without `--force`, which makes git itself refuse if it finds uncommitted or untracked files.

## 6. Worktrees whose folder is gone are pruned, not deleted

Sometimes a worktree's folder has already been removed by hand or by a tool, but git still lists it. There is nothing to delete, so diskdiet creates one `git worktree prune` item per repo, with size zero and risk `Safe`. `git worktree prune` also forgets worktrees on an external drive that is not plugged in, so no prune item is offered for a repo with a worktree on an unmounted volume, and removing a single worktree never runs a prune. It is shown with zero bytes, because the point is to clean git's bookkeeping, not to free space. The other zero-byte item is the Colima disk trim, which frees nothing itself but hands the space Docker released back to macOS.

## 7. Worktrees can live anywhere

Coding agents create worktrees in their own locations, for example `~/orca/workspaces/...`, well outside the folders where you keep your projects. If diskdiet only scanned your dev roots it would miss exactly the folders that grew the most. So it discovers repos inside the roots, and then follows `git worktree list` wherever it points.

## 8. diskdiet never starts Docker or Colima

Starting a container runtime to look at its disk usage is a side effect the user did not ask for, and it can take minutes and gigabytes of memory. diskdiet only inspects Docker when `docker info` already succeeds. Otherwise it reports nothing and the UI says "Docker is not running, skipped". It also requires the active Docker endpoint to be a local socket, so a `DOCKER_HOST` or context pointing at a remote server is never pruned. When it does run, it prunes unused images and build cache only. Images are `Review`, since `docker image prune -a` removes images you built locally and cannot pull again. Containers and volumes are never touched. The plan does not record a specific incident for this rule.

## 9. Permission errors are expected, and explained

macOS protects some locations on purpose. Deleting inside `~/.Trash` needs Full Disk Access, and root-owned apps such as iMovie cannot be moved with plain file APIs. This was found by trying it. So moving to the Trash and emptying it go through Finder, which has the rights, and if a plain delete fails with a permission error diskdiet reports `NeedsFullDiskAccess` and prints where to grant access (System Settings > Privacy & Security > Full Disk Access) instead of a cryptic failure.

## 10. Regenerable data is removed in a way that frees space now

Moving a cache folder to the Trash frees nothing until the Trash is emptied, and a user who sees "freed 40 GB" and an unchanged disk loses trust in the tool. So regenerable data is deleted directly or through the owning tool's own command (`uv cache prune`, `pnpm store prune`, `docker image prune`, `brew cleanup`). Those commands also know about internal references that a plain delete would break. Only apps and downloads go to the Trash, because those are things you may want back. For the same reason, if you also tick "Empty the Trash", it runs before anything is moved there, so what you sent to the Trash in this run is still there afterwards.

## 11. Tools are found the way your shell finds them

A program that inherits a minimal environment may not see the tools you installed through a version manager. pnpm provided by mise is not on the `PATH` a launched binary gets, so a naive lookup concludes pnpm is missing and skips its store, or worse, runs a different version. diskdiet looks in `PATH` first, then reads the `PATH` of your login shell once (`$SHELL -lic`, with a timeout), then tries `mise which <tool>` and accepts the answer only if it is an executable with the tool's name.

## 12. The number on screen must be true

A cleaner that promises 287 GB and delivers 180 GB teaches people not to believe it. So sizes are measured as real disk allocation (the allocated bytes `du` counts, not file length), so sparse files such as VM disk images are counted for what they occupy. Hardlinks are counted once per `(device, inode)` across the whole measurement, and symlinks are never followed. The final "freed" figure is not the estimate at all. It is the difference in free space from `statvfs` before and after. See [ARCHITECTURE.md](ARCHITECTURE.md) for the reasoning. The plan does not record a single incident here, it is the guard against the class of mistake.

## 13. Dry run deletes nothing

`--dry-run` short-circuits every action before it touches the disk and returns `DryRun` for each item. The prompts, the sizes and the summary are identical to a real run, so what you see is what would happen. The confirmation text changes to "Dry run: nothing will be deleted. Continue?" so there is no doubt which mode you are in.

## 14. Everything diskdiet does is logged

Every executed action is appended to `~/Library/Logs/diskdiet.log` with a timestamp, the action and its outcome. If something is missing afterwards, you can find out what removed it and when. The log is append-only, and a failure to write to it never blocks the cleanup.

## 15. Some paths can never be deleted

Every delete and every move to the Trash passes one guard first, whatever scanner produced it and whatever the config says. It refuses relative paths and paths with `..`, your home folder, any folder that contains it (`/Users`, `/`), anything directly inside your home folder (`~/Documents`, `~/.cache`, `~/Library`), and anything directly inside `/Users` or `/Volumes`. The case is ignored, because macOS volumes usually ignore it too. So a config line such as `extra_cache_dirs = ["~"]` produces no item at all, instead of a preselected home folder.

## 16. Context can only make an item riskier

Some items are only safe in the right circumstances, so diskdiet checks them. The cache of an app that is running (Chrome, Firefox, Steam, or Xcode for DerivedData) drops to `Review`. An installer in Downloads is preselected only after it is a week old, because a fresh `.dmg` or `.iso` may be one you are about to use. And when one item contains another (an extra cache folder that holds a Hugging Face model, for example), the outer item takes the risk of the riskiest item inside it, so ticking a `Safe` parent cannot quietly delete a `Review` child.

## Reporting a safety problem

If diskdiet ever removed something it should not have, or offered a `Safe` item that was not, please open an issue with the log lines and a description of the folder. A new incident becomes a new rule and a new test, which is how every rule above got here.
