use std::fs;
use std::path::{Path, PathBuf};

use phpnav_index::discover::DiscoverOptions;
use phpnav_index::{Index, Location};
use tempfile::TempDir;

const TARGET: &str = "<?php\nnamespace App;\nclass Target {}\n";
const TARGET_MOVED: &str = "<?php\nnamespace App;\n\n\n\nclass Target {}\n";
const USE: &str = "<?php\nnamespace App;\nnew Target();\n";

struct Workspace {
    _dir: TempDir,
    root: PathBuf,
}

impl Workspace {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let workspace = Workspace { _dir: dir, root };
        for (path, content) in files {
            workspace.write(path, content);
        }
        workspace
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn index(&self, opts: &DiscoverOptions) -> Index {
        Index::build(&self.root, opts).unwrap().0
    }

    /// Definition of the first occurrence of `needle` in `file`.
    fn definition(&self, index: &mut Index, file: &str, needle: &str) -> Vec<Location> {
        let path = self.path(file);
        let source = fs::read(&path).unwrap();
        let offset = find(&source, needle.as_bytes()) as u32;
        index.definition(&path, &source, offset)
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|w| w == needle)
        .expect("needle in file")
}

fn starts(locations: &[Location]) -> Vec<(PathBuf, u32)> {
    locations
        .iter()
        .map(|l| (l.path.clone(), l.start))
        .collect()
}

fn offset_of(path: &Path, needle: &str) -> u32 {
    find(&fs::read(path).unwrap(), needle.as_bytes()) as u32
}

#[test]
fn update_sees_a_moved_class() {
    let ws = Workspace::new(&[("src/A.php", TARGET), ("src/Use.php", USE)]);
    let mut index = ws.index(&DiscoverOptions::default());
    ws.write("src/A.php", TARGET_MOVED);

    let stats = index.update(&[ws.path("src/A.php")], &[]);

    assert_eq!(stats.reparsed, 1);
    assert_eq!(
        starts(&ws.definition(&mut index, "src/Use.php", "Target")),
        vec![(
            ws.path("src/A.php"),
            offset_of(&ws.path("src/A.php"), "Target")
        )]
    );
}

#[test]
fn update_adds_a_created_file() {
    let ws = Workspace::new(&[("src/Use.php", "<?php\nnamespace App;\nnew Fresh();\n")]);
    let mut index = ws.index(&DiscoverOptions::default());
    ws.write("src/New.php", "<?php\nnamespace App;\nclass Fresh {}\n");

    index.update(&[ws.path("src/New.php")], &[]);

    assert_eq!(ws.definition(&mut index, "src/Use.php", "Fresh").len(), 1);
}

#[test]
fn update_removes_a_deleted_file() {
    let ws = Workspace::new(&[("src/A.php", TARGET), ("src/Use.php", USE)]);
    let mut index = ws.index(&DiscoverOptions::default());
    fs::remove_file(ws.path("src/A.php")).unwrap();

    let stats = index.update(&[], &[ws.path("src/A.php")]);

    assert_eq!(stats.removed, 1);
    assert!(
        ws.definition(&mut index, "src/Use.php", "Target")
            .is_empty()
    );
}

#[test]
fn update_removes_a_deleted_directory() {
    let ws = Workspace::new(&[
        ("lib/A.php", TARGET),
        ("lib/B.php", "<?php\nnamespace App;\nclass Other {}\n"),
        (
            "src/Use.php",
            "<?php\nnamespace App;\nnew Target(); new Other();\n",
        ),
    ]);
    let mut index = ws.index(&DiscoverOptions::default());
    fs::remove_dir_all(ws.path("lib")).unwrap();

    let stats = index.update(&[], &[ws.path("lib")]);

    assert_eq!(stats.removed, 2);
    assert!(
        ws.definition(&mut index, "src/Use.php", "Target")
            .is_empty()
    );
    assert!(ws.definition(&mut index, "src/Use.php", "Other").is_empty());
}

#[test]
fn update_rejects_filtered_paths() {
    let ws = Workspace::new(&[
        ("vendor/composer/installed.json", r#"{"packages": []}"#),
        ("src/Use.php", "<?php\nnew X(); new Y();\n"),
    ]);
    let opts = DiscoverOptions {
        exclude: vec!["excluded/**".to_string()],
        ..DiscoverOptions::default()
    };
    let mut index = ws.index(&opts);
    ws.write("vendor/composer/abc/X.php", "<?php\nclass X {}\n");
    ws.write("excluded/Y.php", "<?php\nclass Y {}\n");

    let stats = index.update(
        &[
            ws.path("vendor/composer/abc/X.php"),
            ws.path("excluded/Y.php"),
        ],
        &[],
    );

    assert_eq!(stats.reparsed, 0);
    assert!(ws.definition(&mut index, "src/Use.php", "X").is_empty());
    assert!(ws.definition(&mut index, "src/Use.php", "Y").is_empty());
}

#[test]
fn update_treats_missing_changed_paths_as_removed() {
    let ws = Workspace::new(&[("src/A.php", TARGET), ("src/Use.php", USE)]);
    let mut index = ws.index(&DiscoverOptions::default());
    fs::remove_file(ws.path("src/A.php")).unwrap();

    let stats = index.update(&[ws.path("src/A.php")], &[]);

    assert_eq!(stats.removed, 1);
    assert!(
        ws.definition(&mut index, "src/Use.php", "Target")
            .is_empty()
    );
}

#[test]
fn update_accepts_paths_under_a_non_canonical_root() {
    let dir = TempDir::new().unwrap();
    let ws = Workspace {
        root: dir.path().canonicalize().unwrap(),
        _dir: dir,
    };
    ws.write("src/Use.php", "<?php\nnamespace App;\nnew Fresh();\n");
    let raw_root = ws._dir.path().to_path_buf();
    let mut index = Index::build(&raw_root, &DiscoverOptions::default())
        .unwrap()
        .0;
    ws.write("src/New.php", "<?php\nnamespace App;\nclass Fresh {}\n");

    index.update(&[raw_root.join("src/New.php")], &[]);

    assert_eq!(ws.definition(&mut index, "src/Use.php", "Fresh").len(), 1);
}
