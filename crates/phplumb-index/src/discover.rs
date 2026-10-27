//! Finds the PHP files of a workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::filter::{PathFilter, SKIPPED_DIR_NAMES};
use crate::frameworks::FrameworkSettings;

pub use crate::filter::PHP_EXTENSIONS;

pub struct DiscoverOptions {
    /// Globs relative to the root, such as `vendor/rector/rector/**`.
    pub exclude: Vec<String>,
    /// Larger files are skipped: they are almost always generated data.
    pub max_file_bytes: u64,
    /// Which framework modules run.
    pub frameworks: FrameworkSettings,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            max_file_bytes: 4 * 1024 * 1024,
            frameworks: FrameworkSettings::default(),
        }
    }
}

/// Returns the PHP files under `root` as absolute paths under the canonicalized root,
/// sorted, each file once.
///
/// Ignore files such as `.gitignore` are not honored, because Drupal projects ignore
/// `vendor` and core in git while their code is still part of the project. Hidden
/// directories and `node_modules` are skipped. Inside a Composer `vendor` directory only
/// the autoload paths of installed packages are kept. Symlinks are followed; a file
/// reachable through several paths is reported once, under its canonical path when
/// that path is inside the root.
pub fn discover(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<Vec<PathBuf>> {
    let mut filter = PathFilter::new(root, opts)?;
    Ok(discover_with(&mut filter))
}

pub(crate) fn discover_with(filter: &mut PathFilter) -> Vec<PathBuf> {
    let root = filter.root().to_path_buf();
    discover_in(filter, &root)
}

/// The PHP files under `start`, a directory under the filter's root, with the same
/// rules and path choice as a full discovery.
pub(crate) fn discover_in(filter: &mut PathFilter, start: &Path) -> Vec<PathBuf> {
    let root = filter.root().to_path_buf();
    let walker = WalkBuilder::new(start)
        .hidden(true)
        .parents(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .follow_links(true)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            !(is_dir
                && SKIPPED_DIR_NAMES
                    .iter()
                    .any(|name| entry.file_name() == *name))
        })
        .build();

    let mut by_canonical: HashMap<PathBuf, PathBuf> = HashMap::new();
    for entry in walker {
        // Unreadable entries and symlink loops are skipped.
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();
        if !filter.accepts_path(path) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > filter.max_file_bytes() {
            continue;
        }
        let Some(canonical) = chosen_path(&root, path) else {
            continue;
        };
        by_canonical
            .entry(canonical.0)
            .and_modify(|current| {
                if canonical.1 < *current {
                    *current = canonical.1.clone();
                }
            })
            .or_insert(canonical.1);
    }

    let mut files: Vec<PathBuf> = by_canonical.into_values().collect();
    files.sort();
    files
}

/// The canonical path of `path` and the path the index uses for it: the canonical
/// path when it is inside the root, otherwise the path as reached.
pub(crate) fn chosen_path(root: &Path, path: &Path) -> Option<(PathBuf, PathBuf)> {
    let canonical = path.canonicalize().ok()?;
    let chosen = if canonical.starts_with(root) {
        canonical.clone()
    } else {
        path.to_path_buf()
    };
    Some((canonical, chosen))
}
