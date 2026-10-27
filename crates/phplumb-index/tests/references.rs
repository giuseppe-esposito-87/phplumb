use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::discover::DiscoverOptions;
use phplumb_index::summary::summarize;
use phplumb_index::{Index, RefKey};
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

/// References of the name found at `needle` (plus `shift`) in `file`, each as
/// `file:text`, where `text` is the source the location covers.
fn references_at(
    index: &mut Index,
    root: &Path,
    file: &str,
    needle: &str,
    shift: usize,
) -> Vec<String> {
    let path = root.join(file);
    let source = fs::read(&path).unwrap();
    let text = String::from_utf8(source.clone()).unwrap();
    let offset = text.find(needle).expect("needle in file") + shift;
    let mut found: Vec<String> = index
        .references(&path, &source, offset as u32, false)
        .into_iter()
        .map(|l| {
            let content = fs::read_to_string(&l.path).unwrap();
            let name = l.path.strip_prefix(root).unwrap().display().to_string();
            format!("{name}:{}", &content[l.start as usize..l.end as usize])
        })
        .collect();
    found.sort();
    found
}

const PARENTS: &str = "class P { public function __construct($a) {} public function save() {} }
class C1 extends P { public function __construct($a) { parent::__construct($a); } public function save() { parent::save(); } }
class C2 extends P { public function __construct($a) { parent::__construct($a); } public function save() { parent::save(); } }
class P2 { private function go() {} public function run() { $this->go(); } }
class C3 extends P2 { public function go() {} }";

#[test]
fn static_calls_reach_only_their_exact_declaration() {
    let (_dir, root, mut index) = workspace(&[("P.php", PARENTS)]);

    let c1_construct = references_at(&mut index, &root, "P.php", "__construct($a) { parent", 0);
    let c1_save = references_at(
        &mut index,
        &root,
        "P.php",
        "save() { parent::save(); } }\nclass C2",
        0,
    );
    let p_save = references_at(&mut index, &root, "P.php", "save() {} }", 0);

    assert_eq!(c1_construct, Vec::<String>::new());
    assert_eq!(c1_save, Vec::<String>::new());
    assert_eq!(p_save, vec!["P.php:save", "P.php:save"]);
}

#[test]
fn private_ancestor_members_are_not_in_the_family() {
    let (_dir, root, mut index) = workspace(&[("P.php", PARENTS)]);

    let c3_go = references_at(&mut index, &root, "P.php", "go() {} }", 0);
    let p2_go = references_at(&mut index, &root, "P.php", "go() {} public", 0);

    assert_eq!(c3_go, Vec::<String>::new());
    assert_eq!(p2_go, vec!["P.php:go"]);
}

const TRAITS: &str = "interface OwnerI { public function getOwner(); }
interface NodeI extends OwnerI {}
interface Unrel { public function getOwner(); }
trait OwnerT { public function getOwner() {} }
class Node implements NodeI { use OwnerT; }
class Over implements Unrel { use OwnerT; public function getOwner() {} }
function f(NodeI $n, Node $m, OwnerI $o, Unrel $u) { $n->getOwner(); $m->getOwner(); $o->getOwner(); $u->getOwner(); }
trait SaveT { public function persist() {} }
class Base { use SaveT; }
class Leaf extends Base { public function persist() {} }
function g(Base $b) { $b->persist(); }";

#[test]
fn trait_methods_find_calls_through_their_users_supertypes() {
    let (_dir, root, mut index) = workspace(&[("T.php", TRAITS)]);

    let trait_method = references_at(&mut index, &root, "T.php", "getOwner() {} }\nclass Node", 0);

    assert_eq!(trait_method, vec!["T.php:getOwner"; 3]);
}

#[test]
fn overrides_find_calls_on_a_parent_using_a_trait() {
    let (_dir, root, mut index) = workspace(&[("T.php", TRAITS)]);

    let leaf = references_at(&mut index, &root, "T.php", "persist() {} }\nfunction g", 0);

    assert_eq!(leaf, vec!["T.php:persist"]);
}

const CONSTRUCTORS: &str = "class X { public function __construct() {} public static function create() { return new static(); } }
class Y extends X {}
class Z extends X { public function __construct() { parent::__construct(); } }
function make() { new X(); new Y; new Z(); }";

#[test]
fn constructors_are_referenced_by_new() {
    let (_dir, root, mut index) = workspace(&[("X.php", CONSTRUCTORS)]);

    let x = references_at(
        &mut index,
        &root,
        "X.php",
        "__construct() {} public static",
        0,
    );

    assert_eq!(
        x,
        vec!["X.php:X", "X.php:Y", "X.php:__construct", "X.php:static"]
    );
}

const STORES: &str = "interface Store { public function load($id); }
abstract class Base implements Store { public function load($id) {} }
class Concrete extends Base {}
class Unrelated { public function load($id) {} }
function f(Concrete $c, Store $s, Unrelated $u) { $c->load(1); $s->load(2); $u->load(3); }";

#[test]
fn interface_methods_find_calls_typed_as_implementations() {
    let (_dir, root, mut index) = workspace(&[("S.php", STORES)]);

    let store_load = references_at(&mut index, &root, "S.php", "load($id); }", 0);

    assert_eq!(store_load, vec!["S.php:load", "S.php:load"]);
}
