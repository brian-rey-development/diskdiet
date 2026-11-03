# diskdiet

**Your AI agents made your disk fat. Put it on a diet.**

[![CI](https://github.com/brian-rey-development/diskdiet/actions/workflows/ci.yml/badge.svg)](https://github.com/brian-rey-development/diskdiet/actions/workflows/ci.yml) ![macOS and Linux](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey) ![Rust](https://img.shields.io/badge/built%20with-Rust-orange) [![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

An interactive disk cleaner for developers, on macOS and Linux. It finds what generic cleaners miss, like worktrees left behind by coding agents, Rust `target/` folders and package manager caches, and lets you pick what goes.

![diskdiet demo](docs/demo.gif)

## Why

Coding agents check out a worktree, build it and leave. On my Mac, ten of those worktrees held 60 GB of Rust build output. No cleaner I tried knew they were there.

## Install

```sh
git clone https://github.com/brian-rey-development/diskdiet.git
cd diskdiet
./setup.sh
```

This builds a release binary into `~/.local/bin`. Run `./setup.sh --uninstall` to remove it.

## Usage

```sh
diskdiet                                   # scan, pick, confirm, delete
diskdiet --dry-run                         # everything except the deleting
diskdiet --report                          # table, no prompts
diskdiet --report --json                   # same, as JSON
diskdiet --only build-artifacts,downloads  # only these categories
diskdiet --root ~/work                     # also scan this folder
```

Start with `--dry-run`.

## What it finds

| Category | What | Ticked by default |
|---|---|---|
| Git worktrees | Linked worktrees of every repo it finds | Only merged and clean ones |
| Build artifacts | `target/`, `.next/`, `.turbo/`, `node_modules` of stale projects | Yes, except `node_modules` |
| Python envs | `.venv` folders | No |
| Package caches | uv, pnpm, npm, cargo, Homebrew, pip, bun, Yarn, Gradle, Go, Xcode | Yes |
| App caches | `~/Library/Caches` or `~/.cache` | Known regenerable ones |
| Docker | Unused images and build cache, Colima VM trim | Build cache and trim |
| AI models | Ollama and Hugging Face | No |
| Unused apps | Apps not opened in 90 days (macOS) | No |
| Downloads | Installers and files older than 90 days | Installers older than a week |
| Trash | Whatever is in it | No |

## Safety

- Nothing risky starts ticked.
- A worktree counts as safe only with no commits ahead, no uncommitted files and no ignored files besides build output. Removing any other worktree means typing `delete`.
- Worktrees are checked again right before removal.
- Apps and downloads go to the Trash, not straight to deletion.
- It never starts Docker.
- Every action is logged to `~/Library/Logs/diskdiet.log` on macOS or `~/.local/state/diskdiet/diskdiet.log` on Linux.

The full rules, and the story behind each one, are in [docs/SAFETY.md](docs/SAFETY.md).

## Configuration

It works without a config. To change which folders it scans, what it skips or how old counts as old, edit `~/.config/diskdiet/config.toml`, starting from [the example](config/diskdiet.example.toml).

## FAQ

**Does it need Full Disk Access?** Only on macOS, and only to empty the Trash or remove some apps. It tells you when.

**Is the freed number real?** Yes. It reads your free space before and after, and reports the difference.

**Is it fast?** A full scan of a machine with 72 GB of projects and 33 worktrees takes under 8 seconds.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). If diskdiet ever offered or removed something it shouldn't have, open a [safety report](https://github.com/brian-rey-development/diskdiet/issues/new?template=safety_report.md).

## License

[MIT](LICENSE)
