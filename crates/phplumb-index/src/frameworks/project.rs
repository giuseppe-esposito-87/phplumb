//! The part of the workspace a framework module reads and answers for: the
//! directory of its project's `composer.lock`.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::filter::SKIPPED_DIR_NAMES;

/// How deep below the root detection looks for `composer.lock`: a project whose
/// Drupal site sits in `src/drupal` keeps its lock at depth 3.
const LOCK_DEPTH: usize = 4;

/// A framework module's project: a directory, without the projects of other active
/// modules inside it. Paths are canonical.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Project {
    pub dir: PathBuf,
    pub excluded: Vec<PathBuf>,
}

impl Project {
    /// Whether `path` is inside the project, comparing whole path components.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.dir) && !self.excluded.iter().any(|dir| path.starts_with(dir))
    }
}

/// The directory of the `composer.lock` nearest to `root` that lists `package`,
/// outside hidden directories, `vendor` and `node_modules`. Locks at the same depth
/// go in path order.
pub(crate) fn nearest_lock(root: &Path, package: &str) -> Option<PathBuf> {
    let mut locks: Vec<(usize, PathBuf)> = WalkBuilder::new(root)
        .standard_filters(false)
        .hidden(true)
        .max_depth(Some(LOCK_DEPTH))
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            !(is_dir
                && (entry.file_name() == "vendor"
                    || SKIPPED_DIR_NAMES
                        .iter()
                        .any(|name| entry.file_name() == *name)))
        })
        .build()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_name() == "composer.lock" && entry.file_type().is_some_and(|t| t.is_file())
        })
        .map(|entry| (entry.depth(), entry.into_path()))
        .collect();
    locks.sort();
    locks
        .into_iter()
        .find(|(_, lock)| requires(lock, package))
        .and_then(|(_, lock)| lock.parent().map(Path::to_path_buf))
}

/// Whether the `composer.lock` at `path` lists `package`.
fn requires(path: &Path, package: &str) -> bool {
    let Ok(text) = std::fs::read(path) else {
        return false;
    };
    let Ok(lock) = serde_json::from_slice::<serde_json::Value>(&text) else {
        return false;
    };
    ["packages", "packages-dev"].iter().any(|section| {
        lock[section]
            .as_array()
            .is_some_and(|packages| packages.iter().any(|p| p["name"] == package))
    })
}
