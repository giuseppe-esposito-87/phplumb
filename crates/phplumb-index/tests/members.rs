use std::fs;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use phplumb_index::Index;
use phplumb_index::discover::DiscoverOptions;
use phplumb_index::phpdoc::TypeExpr;
use phplumb_index::summary::MemberKind;
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

fn definition_targets(
    index: &mut Index,
    root: &std::path::Path,
    file: &str,
    needle: &str,
) -> Vec<String> {
    let path = root.join(file);
    let source = fs::read(&path).unwrap();
    let text = String::from_utf8(source.clone()).unwrap();
    let offset = text.find(needle).unwrap() + needle.len();
    let mut found: Vec<String> = index
        .definition(&path, &source, offset as u32)
        .into_iter()
        .map(|l| {
            let content = fs::read_to_string(&l.path).unwrap();
            let line = content[..l.start as usize].matches('\n').count() + 1;
            format!("{}:{line}", l.path.strip_prefix(root).unwrap().display())
        })
        .collect();
    found.sort();
    found
}

const HOLDER: &str = "class A { public function a() {} }
class B { public function b() {} }
class Holder {
    protected $held;
    public function __construct() { $this->held = new A(); }
    public function run() { $this->held->a(); $this->held->b(); }
}
";

#[test]
fn property_types_follow_edited_assignments() {
    let (_dir, root, mut index) = index(&[("Holder.php", HOLDER)]);
    assert_eq!(
        definition_targets(&mut index, &root, "Holder.php", "$this->held->"),
        vec!["Holder.php:2"]
    );

    fs::write(
        root.join("Holder.php"),
        format!("<?php\n{}", HOLDER.replace("new A()", "new B()")),
    )
    .unwrap();
    index.update(&[root.join("Holder.php")], &[]);

    assert_eq!(
        definition_targets(
            &mut index,
            &root,
            "Holder.php",
            "$this->held->a(); $this->held->"
        ),
        vec!["Holder.php:3"]
    );
}

#[test]
fn properties_assigned_from_each_other_terminate() {
    let (_dir, root, mut index) = index(&[(
        "Loop.php",
        "class A { public function a() {} }
class Loop {
    protected $x;
    protected $y;
    public function __construct() { $this->x = $this->y; $this->y = $this->x; $this->x = new A(); }
    public function run() { $this->y->a(); $this->x->a(); }
}
",
    )]);

    assert_eq!(
        definition_targets(&mut index, &root, "Loop.php", "$this->x->"),
        vec!["Loop.php:2"]
    );
}

#[test]
fn references_through_assigned_properties_are_stable() {
    let mut files: Vec<(String, String)> = vec![(
        "A.php".to_string(),
        "class A { public function target() {} }\n".to_string(),
    )];
    for i in 0..40 {
        files.push((
            format!("H{i}.php"),
            format!(
                "class H{i} {{ protected $a; public function __construct() {{ $this->a = new A(); }} public function run() {{ $this->a->target(); }} }}\n"
            ),
        ));
    }
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let (_dir, root, mut index) = index(&borrowed);
    let path = root.join("A.php");
    let source = fs::read(&path).unwrap();
    let offset = String::from_utf8(source.clone())
        .unwrap()
        .find("target")
        .unwrap() as u32;

    let counts: Vec<usize> = (0..5)
        .map(|_| index.references(&path, &source, offset, false).len())
        .collect();

    assert_eq!(counts, vec![40; 5]);
}

#[test]
fn properties_initialized_lazily_get_their_type() {
    let (_dir, root, mut index) = index(&[(
        "Lazy.php",
        "class A { public function a() {} }
class Lazy {
    protected $held;
    public function held() { return $this->held ??= new A(); }
    public function run() { $this->held->a(); }
}
",
    )]);

    assert_eq!(
        definition_targets(&mut index, &root, "Lazy.php", "$this->held->"),
        vec!["Lazy.php:2"]
    );
}

const CROSSED: &str = "class X { public function hit() {} }
class C {
    protected D $d;
    protected $p;
    public function __construct() { $this->p = $this->d->q; }
    public function run() { $this->p->hit(); }
}
class D {
    protected C $c;
    protected $q;
    public function __construct() { $this->q = $this->c->p; $this->q = new X(); }
    public function run() { $this->q->hit(); }
}
";

#[test]
fn assignments_read_across_classes_do_not_depend_on_query_order() {
    let in_c = "$this->p->";
    let in_d = "$this->q->hit(); }\n}\n";
    let (_a, root_a, mut first) = index(&[("Crossed.php", CROSSED)]);
    let c_first = definition_targets(&mut first, &root_a, "Crossed.php", in_c);
    let d_after = definition_targets(&mut first, &root_a, "Crossed.php", "$this->q->");
    let (_b, root_b, mut second) = index(&[("Crossed.php", CROSSED)]);
    let d_first = definition_targets(&mut second, &root_b, "Crossed.php", "$this->q->");
    let c_after = definition_targets(&mut second, &root_b, "Crossed.php", in_c);

    assert!(CROSSED.contains(in_d));
    assert_eq!(d_first, vec!["Crossed.php:2"]);
    assert_eq!(c_first, c_after);
    assert_eq!(d_after, d_first);
}

#[test]
fn properties_assigned_an_unknown_value_get_no_assigned_type() {
    let (_dir, root, mut index) = index(&[(
        "Unknown.php",
        "class A { public function a() {} }
class Unknown {
    protected $h;
    public function __construct($x) { $this->h = $x; }
    public function reset() { $this->h = new A(); }
    public function run() { $this->h->a(); }
}
",
    )]);

    assert!(definition_targets(&mut index, &root, "Unknown.php", "$this->h->").is_empty());
}

#[test]
fn properties_reset_to_null_keep_their_assigned_type() {
    let (_dir, root, mut index) = index(&[(
        "Reset.php",
        "class A { public function a() {} }
class Reset {
    protected $h;
    public function __construct() { $this->h = new A(); }
    public function clear() { $this->h = NULL; }
    public function run() { $this->h->a(); }
}
",
    )]);

    assert_eq!(
        definition_targets(&mut index, &root, "Reset.php", "$this->h->"),
        vec!["Reset.php:2"]
    );
}

#[test]
fn private_properties_of_a_parent_do_not_type_the_child_property() {
    let (_dir, root, mut index) = index(&[(
        "Private.php",
        "class A { public function a() {} }
class B { public function a() {} }
class Base {
    private $h;
    public function __construct() { $this->h = new A(); }
}
class Child extends Base {
    private $h;
    public function __construct() { parent::__construct(); $this->h = new B(); }
    public function run() { $this->h->a(); }
}
",
    )]);

    assert_eq!(
        definition_targets(&mut index, &root, "Private.php", "$this->h->"),
        vec!["Private.php:3"]
    );
}

#[test]
fn php_files_with_other_extensions_keep_their_navigation() {
    let (_dir, root, mut index) = index(&[
        ("Ctrl.php", "class Ctrl { public function run() {} }\n"),
        ("Page.phtml", "(new Ctrl())->run();\n"),
    ]);
    let path = root.join("Page.phtml");
    let source = fs::read(&path).unwrap();
    let offset = String::from_utf8(source.clone())
        .unwrap()
        .find("new Ct")
        .unwrap()
        + 5;

    let found = index.definition(&path, &source, offset as u32);

    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].path, root.join("Ctrl.php"));
    assert!(index.hover(&path, &source, offset as u32).is_some());
}
