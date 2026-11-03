---
name: Safety report
about: diskdiet offered or removed something it should not have
labels: safety
---

Each confirmed safety report becomes a new rule in docs/SAFETY.md and a new test.

**What was offered or removed**
The item label, its category, the risk shown (Safe, Review or Danger) and whether it was preselected.

**Why it was not safe**
For example, a worktree held a file that existed nowhere else.

**The folder**
Its layout, including which files were inside and what `.gitignore` says. No secrets.

**Log lines**
Matching lines from `~/Library/Logs/diskdiet.log`.

**Environment**
- macOS version:
- diskdiet version (`diskdiet --version`):
