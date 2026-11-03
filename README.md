# diskdiet

**Your AI agents made your disk fat. Put it on a diet.**

diskdiet is an interactive, developer-aware disk cleaner for macOS. It finds the junk that generic cleaners cannot see (git worktrees left behind by coding agents, Rust `target/` folders, Turborepo caches, uv and pnpm stores, Docker images inside Colima) and lets you tick, item by item, what goes.

![diskdiet demo](docs/demo.gif)

<!-- Re-record with `vhs scripts/demo.tape`. It runs inside a fake home folder built by scripts/demo-sandbox.sh, so no real project names show. -->

## Why this exists

One real Mac went from 642 GB used to 319 GB used. 105 GB of that were Rust `target/` folders, and 60 GB of those sat inside ten agent worktrees of a single repo. Each agent had checked out its own copy, built it, and left. Nothing in the usual cleaning tools knew those folders existed, because to them a worktree is just another directory.

diskdiet knows the difference between a build directory you can rebuild in a minute and a worktree that still holds a `.env` you forgot about. It measures real disk usage, shows you the risk of every item, and refuses to preselect anything it cannot prove is safe.

## Install

From source, today.

```sh
git clone https://github.com/brian-rey-development/diskdiet.git
cd diskdiet
./setup.sh
```

`setup.sh` checks for Rust (and offers to install it with rustup), builds a release binary, puts it in `~/.local/bin/diskdiet`, and creates `~/.config/diskdiet/config.toml` if you do not have one. Run `./setup.sh --uninstall` to remove it again.

| Method | Status |
|---|---|
| `./setup.sh` (from source) | Available |
| `brew install diskdiet` | Coming soon |
| `npx diskdiet` | Coming soon |

## Usage

```sh
diskdiet                       # scan, pick what to remove, confirm, delete
diskdiet --dry-run             # the full flow, but nothing is ever deleted
diskdiet --report              # plain table, no prompts
diskdiet --report --json       # the same report as a JSON array
diskdiet --root ~/work         # also scan this folder for projects (repeatable)
diskdiet --only build-artifacts,downloads   # only these categories
diskdiet --config ./my.toml    # use a specific config file
diskdiet --no-share            # skip the share card at the end
```

Start with `diskdiet --dry-run`. You get the same scan and the same prompts, every action reports "dry run" instead of running, and the last screen tells you how much could go instead of how much went.

The interactive flow is short. diskdiet weighs your disk in parallel, shows a summary ("You can lose up to 46.9 GB"), then opens one multiselect per category. Safe items come preselected, risky ones come unselected and flagged. After a final confirmation it executes the selection, measures the free space again, and prints how much you really got back.

## What it finds

| Category | What is in it | Default risk | Preselected |
|---|---|---|---|
| Git worktrees | Linked worktrees of every repo it finds, wherever they live | Safe, Review or Danger, computed per worktree | Only Safe ones (merged, clean, nothing unusual ignored) |
| Build artifacts | `target/`, `.next/`, `.turbo/`, and `node_modules` of projects untouched for 30 days | Safe (`node_modules` is Review) | Yes, except `node_modules` |
| Python virtualenvs | Project `.venv` folders | Review | No ("run `uv sync` to recreate") |
| Package manager caches | uv, pnpm, npm, cargo registry, Homebrew, pip, bun, Yarn, Gradle, Go build, Xcode DerivedData | Safe | Yes |
| App caches | Big folders in `~/Library/Caches`, such as Playwright, Electron and updaters | Safe for known regenerable ones, Review for the rest and while the owning app runs | Only the Safe ones |
| Docker | Unused images and build cache on a local Docker, plus a VM trim when Colima is running | Safe, images are Review | Build cache and trim only |
| AI models | Ollama models and Hugging Face downloads | Review | No |
| Unused apps | Apps in `/Applications` with no sign of use for 90 days (Spotlight's last-used date, plus the app's preferences, container and Application Support folder) | Review | No |
| Downloads | `.dmg`, `.pkg` and `.iso` files, other files older than 90 days | Safe for installers older than a week, Review for the rest | Only week-old installers |
| Trash | Whatever is already in your Trash | Review | No |

The numbers (30 days, 90 days) are defaults, and you can change them in the config file.

## Why it is safe

Every rule in diskdiet comes from something that actually went wrong, or nearly did, while cleaning a real machine. A merged branch does not mean a clean worktree. A `CACHEDIR.TAG` file does not mean a folder is disposable. A `git status` that looks clean can be hiding a `.env`.

So diskdiet never deletes by a single signal, treats ignored files as real files, asks you to type a word before it removes a dangerous worktree, never starts Docker, and writes every action it takes to `~/Library/Logs/diskdiet.log`. The full list, with the story behind each rule, is in [docs/SAFETY.md](docs/SAFETY.md).

## Configuration

diskdiet runs without any config. If you want to change which folders it scans, what it ignores, or how old is "old", copy [config/diskdiet.example.toml](config/diskdiet.example.toml) to `~/.config/diskdiet/config.toml` (`setup.sh` does this for you). Every field is commented with its default. Without a config, diskdiet looks for the usual development folders (`~/Develop`, `~/Developer`, `~/code`, `~/Projects`, `~/src` and a few more) that exist on your machine.

## How it compares

Generic cleaners work from a list of known cache locations and a size sort. They do not understand that a directory is a git worktree, that `target/` is only disposable next to a `Cargo.toml`, or that a pnpm store is shared by every project on the machine. Either they miss these entirely, or they offer them in the same flat list as your browser cache with no sense of what is safe. diskdiet is built around those developer-specific cases first and keeps the generic ones (caches, installers, Trash) as the supporting cast.

## FAQ

**Does it need Full Disk Access?**
Not for most of it. If macOS hides the inside of your Trash from your terminal, the Trash item is left out, and emptying the Trash or removing some root-owned apps can fail with a permission error, and when that happens diskdiet tells you exactly where to grant access (System Settings > Privacy & Security > Full Disk Access, for your terminal app) instead of failing quietly.

**Does it touch my code?**
No. It removes build output and caches that your tools regenerate, and git worktrees you select. A worktree is removed with `git worktree remove`, and the branch itself is never deleted, so committed work stays in the repository. Worktrees with uncommitted files, or with ignored files that are not known build output (a `.env`, a local database), are flagged Danger and need you to type a confirmation word.

**Does it start Docker?**
No. It only inspects Docker when `docker info` already succeeds. If Docker or Colima is stopped, diskdiet prints "Docker is not running, skipped" and moves on. Containers and volumes are never touched, only unused images and build cache.

**What does dry run do?**
It runs everything except the deleting. Scanning, sizing, risk checks, prompts and the final summary all happen as usual, and every action returns "dry run" instead of executing. Nothing is removed, moved or logged as a real action. Use it to see what diskdiet would do on your machine before you let it do it.

**Is the "freed" number real?**
Yes. Right before deleting and again after, diskdiet reads the free space of your volume and reports the difference, so the number on the screen is what your disk now has, not an estimate. Apps and downloads go to the Trash, which frees nothing until it is emptied, and the final screen says so instead of celebrating 0 bytes. The "up to" numbers shown before that are estimates. They use real disk allocation (not file length), count hardlinks once and never follow symlinks, but APFS clones (which pnpm uses for `node_modules` on macOS) share blocks invisibly, so deleting a cloned folder can free less than its size.

**Is it fast?**
It scans every category in parallel. On the Mac from the story, a full scan (72 GB of projects, 33 worktrees, every cache) takes about 9 seconds, while `du -sk ~/Develop` alone takes 42. It reads each folder with one `getattrlistbulk` call, the macOS system call Finder uses, which returns the size of every entry in a batch instead of one `stat` per file.

## Contributing

Bug reports, new scanners and safety fixes are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) for setup and the one hard rule (every safety rule needs a test), and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for how the code is laid out.

## License

[MIT](LICENSE)
