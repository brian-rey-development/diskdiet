use super::io_outcome;
use crate::fsutil::remove_any;
use crate::model::Outcome;
use std::path::Path;

pub(super) fn delete_path(path: &Path) -> Outcome {
    remove_any(path).map_or_else(|error| io_outcome(&error), |()| Outcome::Done)
}
