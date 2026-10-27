use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use phplumb_index::discover::DiscoverOptions;
use phplumb_index::{ClassRef, Index};
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

fn implementations_at(
    index: &mut Index,
    root: &std::path::Path,
    file: &str,
    needle: &str,
) -> BTreeSet<String> {
    let path = root.join(file);
    let source = fs::read(&path).unwrap();
    let offset = source
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())
        .unwrap() as u32;
    index
        .implementations(&path, &source, offset)
        .into_iter()
        .map(|l| l.path.strip_prefix(root).unwrap().display().to_string())
        .collect()
}

#[test]
fn same_named_classes_do_not_leak_methods() {
    let (_dir, root, mut index) = workspace(&[
        (
            "Shape.php",
            "namespace P; interface Shape { public function run(); }",
        ),
        (
            "Dup1.php",
            "namespace P; class Dup implements Shape { public function run() {} }",
        ),
        (
            "Other.php",
            "namespace P; class Other { public function run() {} }",
        ),
        ("Dup2.php", "namespace P; class Dup extends Other {}"),
    ]);

    assert_eq!(
        implementations_at(&mut index, &root, "Shape.php", "run"),
        set(&["Dup1.php"])
    );
}

#[test]
fn implementations_read_the_cursor_file_as_it_is_now() {
    let (_dir, root, mut index) = workspace(&[
        ("I.php", "interface I { public function run(); }"),
        (
            "A.php",
            "class A implements I { public function run() {} public function zap() {} }",
        ),
    ]);
    fs::write(
        root.join("I.php"),
        "<?php\ninterface I { public function zap(); }",
    )
    .unwrap();

    let path = root.join("I.php");
    let source = fs::read(&path).unwrap();
    let offset = source.windows(3).position(|w| w == b"zap").unwrap() as u32;
    let found = index.implementations(&path, &source, offset);

    let a = fs::read(root.join("A.php")).unwrap();
    let zap = a.windows(3).position(|w| w == b"zap").unwrap() as u32;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].start, zap);
}
