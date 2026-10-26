use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use phpnav_index::discover::DiscoverOptions;
use phpnav_index::{CallItem, CallKind, Index};
use tempfile::TempDir;

const SOURCE: &str = "<?php\nclass X { public function __construct() {} }\nfunction helper() {}\nclass A\n{\n    public function a($unknown)\n    {\n        $this->b();\n        $this->b();\n        helper();\n        new X();\n        $unknown->c();\n    }\n\n    public function b() {}\n}\nfunction f() { (new A())->b(); }\n(new A())->b();\n";

fn setup() -> (TempDir, PathBuf, Index) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join("A.php"), SOURCE).unwrap();
    let index = Index::build(&root, &DiscoverOptions::default()).unwrap().0;
    (dir, root, index)
}

fn item(index: &mut Index, root: &Path, needle: &str, shift: usize) -> CallItem {
    let path = root.join("A.php");
    let offset = SOURCE.find(needle).unwrap() + shift;
    let mut items = index.call_items(&path, SOURCE.as_bytes(), offset as u32);
    assert_eq!(items.len(), 1, "{items:?}");
    items.remove(0)
}

fn by_name(calls: Vec<(CallItem, Vec<(u32, u32)>)>) -> BTreeMap<String, (CallKind, usize)> {
    calls
        .into_iter()
        .map(|(item, ranges)| (item.name, (item.kind, ranges.len())))
        .collect()
}

#[test]
fn call_items_on_a_call_site() {
    let (_dir, root, mut index) = setup();

    let b = item(&mut index, &root, "b();\n        $this->b();", 0);

    assert_eq!(
        (b.name.as_str(), b.kind, b.detail.as_str()),
        ("b", CallKind::Method, "A")
    );
}

#[test]
fn incoming_calls_group_by_caller() {
    let (_dir, root, mut index) = setup();
    let b = item(&mut index, &root, "function b()", "function ".len());

    let callers = by_name(index.incoming_calls(&b));

    assert_eq!(callers.get("a"), Some(&(CallKind::Method, 2)));
    assert_eq!(callers.get("f"), Some(&(CallKind::Function, 1)));
}

#[test]
fn incoming_calls_include_top_level_code() {
    let (_dir, root, mut index) = setup();
    let b = item(&mut index, &root, "function b()", "function ".len());

    let callers = by_name(index.incoming_calls(&b));

    assert_eq!(callers.get("A.php"), Some(&(CallKind::File, 1)));
}

#[test]
fn outgoing_calls_list_resolved_callees() {
    let (_dir, root, mut index) = setup();
    let a = item(&mut index, &root, "function a(", "function ".len());

    let callees = by_name(index.outgoing_calls(&a));

    assert_eq!(callees.get("b"), Some(&(CallKind::Method, 2)));
    assert_eq!(callees.get("helper"), Some(&(CallKind::Function, 1)));
    assert_eq!(callees.get("__construct"), Some(&(CallKind::Method, 1)));
    assert_eq!(callees.len(), 3, "{callees:?}");
}
