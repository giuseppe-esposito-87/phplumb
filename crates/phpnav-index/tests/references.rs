use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use phpnav_index::discover::DiscoverOptions;
use phpnav_index::summary::summarize;
use phpnav_index::{Index, RefKey};
use tempfile::TempDir;

fn workspace(files: &[(&str, &str)]) -> (TempDir, PathBuf, Index) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for (path, content) in files {
        fs::write(root.join(path), format!("<?php\n{content}")).unwrap();
    }
    let index = Index::build(&root, &DiscoverOptions::default()).unwrap().0;
    (dir, root, index)
}

fn candidates(index: &Index, root: &Path, key: RefKey) -> BTreeSet<String> {
    index
        .candidate_files(&key)
        .into_iter()
        .map(|p| p.strip_prefix(root).unwrap().display().to_string())
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

const FILES: &[(&str, &str)] = &[
    ("A.php", "namespace App; class A {} function helper() {}"),
    ("B.php", "namespace App; new A(); helper(); $x->run();"),
    ("C.php", "namespace App; echo 1;"),
    ("G.php", "function global_helper() {}"),
    ("H.php", "namespace App; global_helper();"),
];

#[test]
fn candidates_include_files_mentioning_the_symbol() {
    let (_dir, root, index) = workspace(FILES);

    assert_eq!(
        candidates(&index, &root, RefKey::Class("App\\A".to_string())),
        set(&["A.php", "B.php"])
    );
    assert_eq!(
        candidates(&index, &root, RefKey::Function("app\\HELPER".to_string())),
        set(&["A.php", "B.php"])
    );
    assert_eq!(
        candidates(&index, &root, RefKey::Member("run".to_string())),
        set(&["B.php"])
    );
    assert_eq!(
        candidates(&index, &root, RefKey::Function("global_helper".to_string())),
        set(&["G.php", "H.php"])
    );
}

#[test]
fn candidates_follow_updates() {
    let (_dir, root, mut index) = workspace(FILES);
    fs::write(root.join("B.php"), "<?php\nnamespace App; new A();").unwrap();
    fs::write(root.join("D.php"), "<?php\n$y->run();").unwrap();

    index.update(&[root.join("B.php"), root.join("D.php")], &[]);

    assert_eq!(
        candidates(&index, &root, RefKey::Member("RUN".to_string())),
        set(&["D.php"])
    );
}

#[test]
fn declaration_ranges_cover_bodies() {
    let source =
        b"<?php\nfunction f() { return 1; }\nclass K { public function m() { return 2; } }\n";

    let summary = summarize(source);

    let f = summary.declarations.iter().find(|d| d.fqn == "f").unwrap();
    let text = &source[f.range.0 as usize..f.range.1 as usize];
    assert!(
        text.starts_with(b"function f") && text.ends_with(b"}"),
        "{}",
        String::from_utf8_lossy(text)
    );
    let k = summary.declarations.iter().find(|d| d.fqn == "K").unwrap();
    let m = &k.class.as_ref().unwrap().members[0];
    let text = &source[m.range.0 as usize..m.range.1 as usize];
    assert!(
        text.starts_with(b"public function m") && text.ends_with(b"}"),
        "{}",
        String::from_utf8_lossy(text)
    );
}
