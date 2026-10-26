//! The rules that decide which files belong to the index.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::composer::VendorFilter;
use crate::discover::DiscoverOptions;

/// File extensions treated as PHP source, including the Drupal ones.
pub const PHP_EXTENSIONS: &[&str] = &[
    "php", "module", "inc", "install", "theme", "profile", "engine",
];

/// Directories never indexed, whatever the exclusions say.
pub(crate) const SKIPPED_DIR_NAMES: &[&str] = &["node_modules"];

/// The rules discovery applies to each file, usable on a single path: PHP extension,
/// no hidden or `node_modules` component, exclude globs, size limit and the Composer
/// vendor filter.
pub struct PathFilter {
    root: PathBuf,
    given_root: PathBuf,
    exclude: GlobSet,
    max_file_bytes: u64,
    vendor_filters: HashMap<PathBuf, Option<VendorFilter>>,
}

impl PathFilter {
    pub fn new(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<PathFilter> {
        Ok(PathFilter {
            root: root.canonicalize()?,
            given_root: root.to_path_buf(),
            exclude: build_globset(&opts.exclude)?,
            max_file_bytes: opts.max_file_bytes,
            vendor_filters: HashMap::new(),
        })
    }

    /// The canonicalized root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `path` expressed under the canonical root when it was given under the root as
    /// the caller spelled it, for example `/var/...` against `/private/var/...`.
    pub fn under_root(&self, path: &Path) -> PathBuf {
        match path.strip_prefix(&self.given_root) {
            Ok(rest) if self.given_root != self.root => self.root.join(rest),
            _ => path.to_path_buf(),
        }
    }

    /// Whether `path`, an existing file, belongs to the index.
    pub fn accepts(&mut self, path: &Path) -> bool {
        let path = self.under_root(path);
        self.accepts_path(&path)
            && std::fs::metadata(&path)
                .is_ok_and(|meta| meta.is_file() && meta.len() <= self.max_file_bytes)
    }

    /// The path rules alone, without touching the file itself. `path` is under the
    /// canonical root.
    pub(crate) fn accepts_path(&mut self, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(&self.root) else {
            return false;
        };
        if !has_php_extension(path) || self.exclude.is_match(relative) {
            return false;
        }
        let skipped = relative.components().any(|component| match component {
            Component::Normal(name) => {
                name.to_string_lossy().starts_with('.')
                    || SKIPPED_DIR_NAMES.iter().any(|skipped| name == *skipped)
            }
            _ => false,
        });
        if skipped {
            return false;
        }
        let root = &self.root;
        let vendor_filters = &mut self.vendor_filters;
        path.ancestors()
            .skip(1)
            .take_while(|dir| dir.starts_with(root))
            .filter(|dir| dir.file_name().is_some_and(|name| name == "vendor"))
            .all(|vendor| {
                vendor_filters
                    .entry(vendor.to_path_buf())
                    .or_insert_with(|| VendorFilter::load(vendor))
                    .as_ref()
                    .is_none_or(|filter| filter.allows(path))
            })
    }

    pub(crate) fn max_file_bytes(&self) -> u64 {
        self.max_file_bytes
    }
}

fn has_php_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| PHP_EXTENSIONS.contains(&ext))
}

/// Invalid patterns are reported on stderr and skipped, so a typo in the settings
/// does not stop indexing.
fn build_globset(patterns: &[String]) -> anyhow::Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(err) => eprintln!("php-navigator: ignoring exclude pattern {pattern:?}: {err}"),
        }
    }
    Ok(builder.build()?)
}
