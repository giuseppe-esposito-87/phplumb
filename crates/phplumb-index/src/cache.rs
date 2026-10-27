//! The on-disk copy of an index, validated per file by size and modification time.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::filter::PathFilter;
use crate::summary::Declaration;

const FORMAT: u32 = 1;

/// The engine that writes and reads caches: the crate version plus a fingerprint of
/// the indexing code, so any rebuild of the engine invalidates older caches.
pub(crate) const ENGINE: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+", env!("PHPLUMB_INDEX_BUILD"));

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
    /// Hashes of the `RefKey`s the file mentions, for finding reference candidates.
    pub ref_keys: Vec<u64>,
}

/// The cached files keyed by path, or nothing when the cache is missing, unreadable,
/// or was written by another `engine` or for other options. postcard does not
/// describe its own layout, so trailing bytes also count as unreadable.
pub(crate) fn load(
    cache: &Path,
    filter: &PathFilter,
    engine: &str,
) -> HashMap<PathBuf, CachedFile> {
    let Ok(bytes) = std::fs::read(cache) else {
        return HashMap::new();
    };
    let Ok((file, rest)) = postcard::take_from_bytes::<CacheFile>(&bytes) else {
        return HashMap::new();
    };
    let matches = rest.is_empty()
        && file.format == FORMAT
        && file.engine == engine
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
    engine: &str,
    files: Vec<CachedFile>,
) -> anyhow::Result<()> {
    let file = CacheFile {
        format: FORMAT,
        engine: engine.to_string(),
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::{CachedFile, load, save};
    use crate::discover::DiscoverOptions;
    use crate::filter::PathFilter;

    fn file(path: PathBuf) -> CachedFile {
        CachedFile {
            path,
            size: 1,
            mtime_ns: 1,
            has_errors: false,
            declarations: Vec::new(),
            ref_keys: Vec::new(),
        }
    }

    #[test]
    fn round_trips_for_the_same_engine() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("cache.bin");
        let filter = PathFilter::new(root.path(), &DiscoverOptions::default()).unwrap();
        save(
            &cache,
            &filter,
            "engine-a",
            vec![file(root.path().join("A.php"))],
        )
        .unwrap();

        assert_eq!(load(&cache, &filter, "engine-a").len(), 1);
    }

    #[test]
    fn ignores_a_cache_from_another_engine_build() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("cache.bin");
        let filter = PathFilter::new(root.path(), &DiscoverOptions::default()).unwrap();
        save(
            &cache,
            &filter,
            "engine-a",
            vec![file(root.path().join("A.php"))],
        )
        .unwrap();

        assert!(load(&cache, &filter, "engine-b").is_empty());
    }

    #[test]
    fn ignores_a_cache_for_another_root() {
        let first = TempDir::new().unwrap();
        let second = TempDir::new().unwrap();
        let cache = first.path().join("cache.bin");
        let first_filter = PathFilter::new(first.path(), &DiscoverOptions::default()).unwrap();
        let second_filter = PathFilter::new(second.path(), &DiscoverOptions::default()).unwrap();
        save(
            &cache,
            &first_filter,
            "engine-a",
            vec![file(first.path().join("A.php"))],
        )
        .unwrap();

        assert!(load(&cache, &second_filter, "engine-a").is_empty());
    }

    #[test]
    fn ignores_trailing_bytes() {
        let root = TempDir::new().unwrap();
        let cache = root.path().join("cache.bin");
        let filter = PathFilter::new(root.path(), &DiscoverOptions::default()).unwrap();
        save(
            &cache,
            &filter,
            "engine-a",
            vec![file(root.path().join("A.php"))],
        )
        .unwrap();
        let mut bytes = std::fs::read(&cache).unwrap();
        bytes.extend_from_slice(b"junk");
        std::fs::write(&cache, bytes).unwrap();

        assert!(load(&cache, &filter, "engine-a").is_empty());
    }

    #[test]
    fn engine_id_includes_a_build_fingerprint() {
        assert!(super::ENGINE.starts_with(concat!(env!("CARGO_PKG_VERSION"), "+")));
        assert!(super::ENGINE.len() > env!("CARGO_PKG_VERSION").len() + 1);
    }
}
