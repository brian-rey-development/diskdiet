use super::io_outcome;
use crate::domain::model::Outcome;
use crate::infra::fs::paths::remove_any;
use std::path::Path;

pub(super) fn delete_path(path: &Path) -> Outcome {
    remove_any(path).map_or_else(|error| io_outcome(&error), |()| Outcome::Done)
}
