# Records a dry run into docs/demo.gif, inside a fake home folder so no real project names show.
# Install vhs with `brew install vhs`, build with `cargo build --release`,
# then run from the repo root with `vhs scripts/demo.tape`.
# The key sequence is checked by driving the same flow with expect against the same sandbox.

Output docs/demo.gif

Set Shell "zsh"
Set FontSize 16
Set FontFamily "Menlo"
Set Width 1400
Set Height 1000
Set Padding 24
Set Theme "Catppuccin Latte"
Set TypingSpeed 50ms
Set WindowBar Colorful

Hide
Type `export PATH="$PWD/target/release:$PATH" HOME="$(cd "$(./scripts/demo-sandbox.sh)" && pwd -P)" && cd ~ && PROMPT='$ ' && clear`
Enter
Sleep 3s
Show

Type "diskdiet --dry-run --only agent-worktrees,build-artifacts,downloads"
Sleep 500ms
Enter

Wait+Screen@60s /Git worktrees ·/
Sleep 1.5s
Down
Sleep 300ms
Down
Sleep 300ms
Space
Sleep 1s
Enter

Wait+Screen@10s /Build artifacts ·/
Sleep 1.5s
Enter

Wait+Screen@10s /Downloads ·/
Sleep 1s
Enter

Wait+Screen@10s /press Enter to keep it/
Sleep 1s
Type "delete"
Sleep 500ms
Enter

Wait+Screen@10s /Continue\?/
Sleep 1s
Type "y"

Wait+Screen@30s /Dry run complete/
Sleep 4s
