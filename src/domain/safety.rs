use crate::infra::fs::paths::lowercased;
use crate::infra::sys::platform::MOUNT_ROOTS;
use std::path::{Component, Path};

const MIN_DELETE_COMPONENTS: usize = 3;
const SHARED_PARENTS: [&str; 2] = ["/users", "/home"];

fn is_plain_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path.components().count() >= MIN_DELETE_COMPONENTS
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

fn is_home_ancestor_or_direct_child(path: &Path, home: &Path) -> bool {
    let (path, home) = (lowercased(path), lowercased(home));
    home.starts_with(&path) || path.parent() == Some(home.as_path())
}

fn is_shared_parent_child(path: &Path) -> bool {
    let parent = path.parent().map(lowercased);
    SHARED_PARENTS
        .iter()
        .any(|shared| parent.as_deref() == Some(Path::new(shared)))
}

fn is_mount_root_or_mount_point_or_above(path: &Path) -> bool {
    let path = lowercased(path);
    MOUNT_ROOTS.iter().any(|(root, depth)| {
        path.strip_prefix(root.to_lowercase())
            .is_ok_and(|rest| rest.components().count() <= *depth)
    })
}

pub fn is_safe_delete_target(path: &Path, home: Option<&Path>) -> bool {
    is_plain_absolute(path)
        && !is_shared_parent_child(path)
        && !is_mount_root_or_mount_point_or_above(path)
        && home.is_none_or(|home| !is_home_ancestor_or_direct_child(path, home))
}
