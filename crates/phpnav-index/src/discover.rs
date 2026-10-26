//! Finds the PHP files of a workspace.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;

/// File extensions treated as PHP source, including the Drupal ones.
pub const PHP_EXTENSIONS: &[&str] = &[
    "php", "module", "inc", "install", "theme", "profile", "engine",
];

/// Directories never walked, whatever the exclusions say.
const SKIPPED_DIR_NAMES: &[&str] = &["node_modules"];

pub struct DiscoverOptions {
    /// Globs relative to the root, such as `vendor/rector/rector/**`.
    pub exclude: Vec<String>,
    /// Larger files are skipped: they are almost always generated data.
    pub max_file_bytes: u64,
}

impl Default for DiscoverOptions {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            max_file_bytes: 4 * 1024 * 1024,
        }
    }
}

/// Returns the PHP files under `root` as absolute paths under the canonicalized root,
/// sorted, each file once.
///
/// Ignore files such as `.gitignore` are not honored, because Drupal projects ignore
/// `vendor` and core in git while their code is still part of the project. Hidden
/// directories and `node_modules` are skipped. Symlinks are followed; a file reachable
/// through several paths is reported once, under its canonical path when that path is
/// inside the root.
pub fn discover(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<Vec<PathBuf>> {
    let root = root.canonicalize()?;
    let exclude = build_globset(&opts.exclude)?;

    let walker = WalkBuilder::new(&root)
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
        if !has_php_extension(path) {
            continue;
        }
        let relative = path.strip_prefix(&root).unwrap_or(path);
        if exclude.is_match(relative) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        if metadata.len() > opts.max_file_bytes {
            continue;
        }
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        let chosen = if canonical.starts_with(&root) {
            canonical.clone()
        } else {
            path.to_path_buf()
        };
        by_canonical
            .entry(canonical)
            .and_modify(|current| {
                if chosen < *current {
                    *current = chosen.clone();
                }
            })
            .or_insert(chosen);
    }

    let mut files: Vec<PathBuf> = by_canonical.into_values().collect();
    files.sort();
    Ok(files)
}

fn has_php_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| PHP_EXTENSIONS.contains(&ext))
}

fn build_globset(patterns: &[String]) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    Ok(builder.build()?)
}
