use crate::support::fixtures::temp_root;
use crate::support::git::{add_worktree, init_repo};
use anyhow::Result;
use diskdiet::features::scan::worktrees::find_repos;
use std::fs;

#[test]
fn repos_inside_agent_worktree_folders_are_not_rediscovered() -> Result<()> {
    let (_tmp, root) = temp_root()?;
    let repo = root.join("repo");
    init_repo(&repo)?;
    add_worktree(&repo, &repo.join(".claude/worktrees/agent"), "agent")?;
    let found = find_repos(std::slice::from_ref(&root), &[]);
    assert_eq!(found, vec![fs::canonicalize(repo)?]);
    Ok(())
}
