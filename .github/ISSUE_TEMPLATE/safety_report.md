---
name: Safety report
about: diskdiet offered or removed something it should not have
labels: safety
---

Safety reports are the most important issues in this repository. Each confirmed one becomes a new rule in docs/SAFETY.md and a new test.

**What was offered or removed**
The item label, its category and the risk diskdiet showed (Safe, Review or Danger). Was it preselected?

**Why it was not safe**
For example, a worktree held a file that existed nowhere else, or a cache was in use.

**The folder**
Describe its layout (which files were inside, what `.gitignore` says). Do not paste secrets.

**Log lines**
The matching lines from `~/Library/Logs/diskdiet.log`.

**Environment**
- macOS version:
- diskdiet version (`diskdiet --version`):
