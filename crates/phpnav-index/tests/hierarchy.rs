use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use phpnav_index::discover::DiscoverOptions;
use phpnav_index::{ClassRef, Index};
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

fn names(classes: Vec<ClassRef<'_>>) -> BTreeSet<String> {
    classes
        .into_iter()
        .map(|c| c.declaration.fqn.clone())
        .collect()
}

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| s.to_string()).collect()
}

const FILES: &[(&str, &str)] = &[
    ("I.php", "interface I {}"),
    ("J.php", "interface J extends I {}"),
    ("A.php", "abstract class A implements J {}"),
    ("B.php", "class B extends A {}"),
    ("E.php", "enum E implements I { case X; }"),
];

#[test]
fn subtypes_are_direct_and_all_subtypes_transitive() {
    let (_dir, _root, index) = workspace(FILES);

    assert_eq!(names(index.subtypes("I")), set(&["J", "E"]));
    assert_eq!(names(index.all_subtypes("i")), set(&["J", "A", "B", "E"]));
    assert_eq!(names(index.supertypes("B")), set(&["A"]));
    assert_eq!(names(index.supertypes("A")), set(&["J"]));
}

#[test]
fn all_subtypes_survive_cycles() {
    let (_dir, _root, index) = workspace(&[
        ("A.php", "class A extends B {}"),
        ("B.php", "class B extends A {}"),
    ]);

    assert_eq!(names(index.all_subtypes("A")), set(&["B"]));
}

#[test]
fn update_drops_removed_subtypes() {
    let (_dir, root, mut index) = workspace(FILES);
    fs::remove_file(root.join("B.php")).unwrap();

    index.update(&[], &[root.join("B.php")]);

    assert_eq!(names(index.all_subtypes("I")), set(&["J", "A", "E"]));
}
