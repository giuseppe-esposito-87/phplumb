use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use phpnav_index::Index;
use phpnav_index::discover::DiscoverOptions;
use phpnav_index::phpdoc::TypeExpr;
use phpnav_index::summary::MemberKind;
use tempfile::TempDir;

fn index(files: &[(&str, &str)]) -> (TempDir, PathBuf, Index) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for (path, content) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("<?php\n{content}")).unwrap();
    }
    let index = Index::build(&root, &DiscoverOptions::default()).unwrap().0;
    (dir, root, index)
}

#[test]
fn find_member_follows_the_lookup_order() {
    let (_dir, root, index) = index(&[
        (
            "I.php",
            "interface I { public function onlyInInterface(); }",
        ),
        ("P.php", "class P implements I { public function m() {} }"),
        ("T.php", "trait T { public function m() {} }"),
        ("C.php", "class C extends P { use T; }"),
    ]);

    let m = index.find_member("C", MemberKind::Method, "M");
    let only = index.find_member("C", MemberKind::Method, "onlyInInterface");

    assert_eq!(m.len(), 1);
    assert_eq!(m[0].0.path, root.join("T.php"));
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].0.path, root.join("I.php"));
}

#[test]
fn find_member_survives_cycles() {
    let (_dir, _root, index) = index(&[
        ("A.php", "class A extends B {}"),
        ("B.php", "class B extends A { use T; } trait T { use T; }"),
    ]);
    let started = Instant::now();

    let found = index.find_member("A", MemberKind::Method, "x");

    assert!(found.is_empty());
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn property_typed_only_by_phpdoc_in_a_parent() {
    let (_dir, root, index) = index(&[
        ("P.php", "class P { /** @var Foo */ protected $foo; }"),
        ("C.php", "class C extends P {}"),
    ]);

    let found = index.find_member("C", MemberKind::Property, "foo");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].0.path, root.join("P.php"));
    assert_eq!(found[0].1.ty, Some(TypeExpr::Named("Foo".to_string())));
}

#[test]
fn property_names_are_case_sensitive() {
    let (_dir, _root, index) = index(&[("C.php", "class C { public $foo; }")]);

    assert!(
        index
            .find_member("C", MemberKind::Property, "Foo")
            .is_empty()
    );
    assert_eq!(index.find_member("c", MemberKind::Property, "foo").len(), 1);
}
