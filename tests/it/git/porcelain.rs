use diskdiet::git::porcelain::parse_porcelain;
use std::path::PathBuf;

#[test]
fn porcelain_parser_handles_spaces_detached_and_bare_entries() {
    let text = "worktree /Users/me/My Repo\nHEAD aaa\nbranch refs/heads/main\n\n\
                worktree /Users/me/wt one\nHEAD bbb\nbranch refs/heads/feat/x\n\n\
                worktree /Users/me/wt-detached\nHEAD ccc\ndetached\n\n\
                worktree /srv/bare.git\nbare\n\n\
                worktree /Volumes/x/gone\nHEAD ddd\nbranch refs/heads/gone\n\
                locked on a usb drive\nprunable gitdir file points to non-existent location\n";
    let entries = parse_porcelain(text);
    assert_eq!(entries.len(), 5);
    assert_eq!(entries[0].path, PathBuf::from("/Users/me/My Repo"));
    assert_eq!(entries[1].path, PathBuf::from("/Users/me/wt one"));
    assert_eq!(entries[1].branch.as_deref(), Some("feat/x"));
    assert!(entries[2].detached && entries[2].branch.is_none());
    assert!(entries[3].bare && entries[3].head.is_none());
    assert!(entries[4].locked && entries[4].prunable);
}

#[test]
fn porcelain_parser_ignores_empty_input_and_quoted_paths() {
    assert!(parse_porcelain("").is_empty());
    assert!(parse_porcelain("\n\n").is_empty());
    assert!(parse_porcelain("worktree \"/odd\\npath\"\nHEAD a\n").is_empty());
}
