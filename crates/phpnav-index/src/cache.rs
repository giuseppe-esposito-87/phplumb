//! The on-disk copy of an index, validated per file by size and modification time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::filter::PathFilter;
use crate::summary::Declaration;

const FORMAT: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CacheFile {
    format: u32,
    engine: String,
    root: PathBuf,
    exclude: Vec<String>,
    max_file_bytes: u64,
    files: Vec<CachedFile>,
}

/// One file as it was when indexed.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct CachedFile {
    pub path: PathBuf,
    pub size: u64,
    pub mtime_ns: u64,
    pub has_errors: bool,
    pub declarations: Vec<Declaration>,
}

/// The cached files keyed by path, or nothing when the cache is missing, unreadable,
/// or was written by another engine version or for other options.
pub(crate) fn load(cache: &Path, filter: &PathFilter) -> HashMap<PathBuf, CachedFile> {
    let Ok(bytes) = std::fs::read(cache) else {
        return HashMap::new();
    };
    let Ok(file) = postcard::from_bytes::<CacheFile>(&bytes) else {
        return HashMap::new();
    };
    let matches = file.format == FORMAT
        && file.engine == env!("CARGO_PKG_VERSION")
        && file.root == filter.root()
        && file.exclude == filter.exclude_patterns()
        && file.max_file_bytes == filter.max_file_bytes();
    if !matches {
        return HashMap::new();
    }
    file.files
        .into_iter()
        .map(|cached| (cached.path.clone(), cached))
        .collect()
}

/// Writes the cache to a temporary file next to `cache`, then renames it into place.
pub(crate) fn save(
    cache: &Path,
    filter: &PathFilter,
    files: Vec<CachedFile>,
) -> anyhow::Result<()> {
    let file = CacheFile {
        format: FORMAT,
        engine: env!("CARGO_PKG_VERSION").to_string(),
        root: filter.root().to_path_buf(),
        exclude: filter.exclude_patterns().to_vec(),
        max_file_bytes: filter.max_file_bytes(),
        files,
    };
    let bytes = postcard::to_stdvec(&file)?;
    if let Some(dir) = cache.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temporary = cache.with_extension("tmp");
    std::fs::write(&temporary, bytes)?;
    std::fs::rename(&temporary, cache)?;
    Ok(())
}
