use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::discover::DiscoverOptions;
use phplumb_index::{Index, IndexStats};
use tempfile::TempDir;

struct Workspace {
    _dir: TempDir,
    _cache_dir: TempDir,
    root: PathBuf,
    cache: PathBuf,
}

impl Workspace {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let cache_dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let cache = cache_dir.path().join("index-v1.bin");
        let ws = Workspace {
            _dir: dir,
            _cache_dir: cache_dir,
            root,
            cache,
        };
        ws.write("src/A.php", "<?php\nclass A {}\n");
        ws.write("src/B.php", "<?php\nclass B {}\n");
        ws.write("src/Use.php", "<?php\nnew A(); new B(); new C();\n");
        ws
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn build(&self, opts: &DiscoverOptions) -> (Index, IndexStats) {
        Index::build_with_cache(&self.root, opts, &self.cache).unwrap()
    }

    fn resolves(&self, index: &mut Index, name: &str) -> bool {
        let path = self.root.join("src/Use.php");
        let source = fs::read(&path).unwrap();
        let offset = source
            .windows(name.len())
            .position(|w| w == name.as_bytes())
            .unwrap() as u32;
        !index.definition(&path, &source, offset).is_empty()
    }
}

#[test]
fn warm_start_reuses_unchanged_files() {
    let ws = Workspace::new();
    let (cold, cold_stats) = ws.build(&DiscoverOptions::default());
    cold.save_cache(&ws.cache).unwrap();

    let (mut warm, stats) = ws.build(&DiscoverOptions::default());

    assert_eq!((cold_stats.reused, cold_stats.reparsed), (0, 3));
    assert_eq!((stats.reused, stats.reparsed), (3, 0));
    assert!(ws.resolves(&mut warm, "A()"));
    assert!(ws.resolves(&mut warm, "B()"));
}

#[test]
fn warm_start_reparses_changed_files() {
    let ws = Workspace::new();
    ws.build(&DiscoverOptions::default())
        .0
        .save_cache(&ws.cache)
        .unwrap();
    ws.write("src/B.php", "<?php\nclass B {}\nclass C {}\n");

    let (mut warm, stats) = ws.build(&DiscoverOptions::default());

    assert_eq!((stats.reused, stats.reparsed), (2, 1));
    assert!(ws.resolves(&mut warm, "C()"));
}

#[test]
fn warm_start_drops_deleted_and_adds_new_files() {
    let ws = Workspace::new();
    ws.build(&DiscoverOptions::default())
        .0
        .save_cache(&ws.cache)
        .unwrap();
    fs::remove_file(ws.root.join("src/A.php")).unwrap();
    ws.write("src/C.php", "<?php\nclass C {}\n");

    let (mut warm, stats) = ws.build(&DiscoverOptions::default());

    assert_eq!((stats.reused, stats.reparsed), (2, 1));
    assert!(!ws.resolves(&mut warm, "A()"));
    assert!(ws.resolves(&mut warm, "C()"));
}

#[test]
fn cache_with_other_exclusions_is_ignored() {
    let ws = Workspace::new();
    ws.build(&DiscoverOptions::default())
        .0
        .save_cache(&ws.cache)
        .unwrap();
    let opts = DiscoverOptions {
        exclude: vec!["nothing/**".to_string()],
        ..DiscoverOptions::default()
    };

    let (_, stats) = ws.build(&opts);

    assert_eq!((stats.reused, stats.reparsed), (0, 3));
}

#[test]
fn corrupt_cache_falls_back_to_full_build() {
    let ws = Workspace::new();
    fs::write(&ws.cache, b"not a cache").unwrap();

    let (mut index, stats) = ws.build(&DiscoverOptions::default());

    assert_eq!((stats.reused, stats.reparsed), (0, 3));
    assert!(ws.resolves(&mut index, "A()"));
}

#[test]
fn save_cache_replaces_atomically() {
    let ws = Workspace::new();
    let (index, _) = ws.build(&DiscoverOptions::default());

    index.save_cache(&ws.cache).unwrap();
    index.save_cache(&ws.cache).unwrap();

    let names: Vec<String> = fs::read_dir(ws.cache.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["index-v1.bin"]);
}

#[test]
fn save_cache_creates_its_directory() {
    let ws = Workspace::new();
    let (index, _) = ws.build(&DiscoverOptions::default());
    let nested: &Path = &ws.cache.parent().unwrap().join("a/b/index-v1.bin");

    index.save_cache(nested).unwrap();

    assert!(nested.exists());
}
