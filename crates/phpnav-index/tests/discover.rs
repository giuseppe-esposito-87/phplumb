use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use phpnav_index::discover::{DiscoverOptions, discover};
use tempfile::TempDir;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn canonical_root(dir: &TempDir) -> PathBuf {
    dir.path().canonicalize().unwrap()
}

fn relative(root: &Path, paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .map(|p| p.strip_prefix(root).unwrap().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn finds_all_php_extensions() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    for name in [
        "a.php",
        "b.module",
        "c.inc",
        "d.install",
        "e.theme",
        "f.profile",
        "g.engine",
        "h.txt",
        "i.js",
    ] {
        write(&root, name, "<?php\n");
    }

    let found = discover(&root, &DiscoverOptions::default()).unwrap();

    assert_eq!(
        relative(&root, &found),
        vec![
            "a.php",
            "b.module",
            "c.inc",
            "d.install",
            "e.theme",
            "f.profile",
            "g.engine"
        ]
    );
}

#[test]
fn ignores_gitignore_rules() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    fs::create_dir_all(root.join(".git")).unwrap();
    write(&root, ".gitignore", "vendor/\n");
    write(&root, "vendor/x/A.php", "<?php\n");

    let found = discover(&root, &DiscoverOptions::default()).unwrap();

    assert_eq!(relative(&root, &found), vec!["vendor/x/A.php"]);
}

#[test]
fn skips_hidden_dirs_and_node_modules() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    write(&root, ".ds-clone/A.php", "<?php\n");
    write(&root, "node_modules/p/B.php", "<?php\n");
    write(&root, "src/C.php", "<?php\n");

    let found = discover(&root, &DiscoverOptions::default()).unwrap();

    assert_eq!(relative(&root, &found), vec!["src/C.php"]);
}

#[test]
fn applies_exclude_globs() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    write(&root, "vendor/rector/rector/vendor/X.php", "<?php\n");
    write(&root, "vendor/other/Y.php", "<?php\n");
    let opts = DiscoverOptions {
        exclude: vec!["vendor/rector/rector/**".to_string()],
        ..DiscoverOptions::default()
    };

    let found = discover(&root, &opts).unwrap();

    assert_eq!(relative(&root, &found), vec!["vendor/other/Y.php"]);
}

#[test]
fn skips_files_over_size_limit() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    write(&root, "big.php", "<?php\n12345");
    write(&root, "ok.php", "<?php\n1234");
    let opts = DiscoverOptions {
        max_file_bytes: 10,
        ..DiscoverOptions::default()
    };

    let found = discover(&root, &opts).unwrap();

    assert_eq!(relative(&root, &found), vec!["ok.php"]);
}

#[test]
fn dedupes_symlinked_dirs() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    write(&root, "modules/foo/Foo.php", "<?php\n");
    fs::create_dir_all(root.join("web/modules/custom")).unwrap();
    symlink("../../../modules/foo", root.join("web/modules/custom/foo")).unwrap();

    let found = discover(&root, &DiscoverOptions::default()).unwrap();

    assert_eq!(found, vec![root.join("modules/foo/Foo.php")]);
}

#[test]
fn survives_symlink_loop() {
    let dir = TempDir::new().unwrap();
    let root = canonical_root(&dir);
    write(&root, "a/A.php", "<?php\n");
    symlink("..", root.join("a/loop")).unwrap();

    let found = discover(&root, &DiscoverOptions::default()).unwrap();

    assert_eq!(relative(&root, &found), vec!["a/A.php"]);
}

#[test]
fn accepts_a_non_canonical_root() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/A.php", "<?php\n");

    let found = discover(dir.path(), &DiscoverOptions::default()).unwrap();

    assert_eq!(found, vec![canonical_root(&dir).join("src/A.php")]);
}
