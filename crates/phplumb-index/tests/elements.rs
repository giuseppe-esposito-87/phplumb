use std::path::{Path, PathBuf};

use phplumb_index::discover::DiscoverOptions;
use phplumb_index::phpdoc::TypeExpr;
use phplumb_index::summary::{ClassShape, MemberKind, summarize};
use phplumb_index::{Index, Location};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/elements")
        .canonicalize()
        .unwrap()
}

fn build() -> Index {
    Index::build(&fixture(), &DiscoverOptions::default())
        .unwrap()
        .0
}

/// The file `file` of the fixture, its content, and the offset of the `nth`
/// occurrence of `needle` in it.
fn at(file: &str, needle: &str, nth: usize) -> (PathBuf, Vec<u8>, u32) {
    let path = fixture().join(file);
    let source = std::fs::read(&path).unwrap();
    let text = String::from_utf8(source.clone()).unwrap();
    let offset = text
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("no occurrence {nth} of {needle} in {file}"))
        .0;
    (path, source, offset as u32)
}

fn definition(index: &mut Index, file: &str, needle: &str, nth: usize) -> Vec<Location> {
    let (path, source, offset) = at(file, needle, nth);
    index.definition(&path, &source, offset)
}

/// Each location as its path relative to the fixture and the text at its range.
fn described(locations: &[Location]) -> Vec<(String, String)> {
    locations
        .iter()
        .map(|location| {
            let text = std::fs::read(&location.path).unwrap();
            let range = &text[location.start as usize..location.end as usize];
            let relative = location
                .path
                .strip_prefix(fixture())
                .unwrap_or(&location.path);
            (
                relative.display().to_string(),
                String::from_utf8_lossy(range).into_owned(),
            )
        })
        .collect()
}

fn shape(source: &str, fqn: &str) -> ClassShape {
    summarize(source.as_bytes())
        .declarations
        .into_iter()
        .find(|d| d.fqn == fqn)
        .and_then(|d| d.class)
        .unwrap_or_else(|| panic!("no class {fqn}"))
}

fn named(fqn: &str) -> TypeExpr {
    TypeExpr::Named(fqn.to_string())
}

#[test]
fn generic_doc_types_keep_their_arguments() {
    let shape = shape(
        "<?php namespace App; use Lib\\Collection; class S { /** @var Collection<int, Item> */ public $c; }",
        "App\\S",
    );
    let property = shape
        .members
        .iter()
        .find(|m| m.kind == MemberKind::Property && m.name == "c")
        .expect("property c");
    assert_eq!(
        property.ty,
        Some(TypeExpr::Generic {
            class: "Lib\\Collection".to_string(),
            args: vec![TypeExpr::Other, named("App\\Item")],
        })
    );
}

#[test]
fn class_templates_and_generic_supertypes() {
    let shape = shape(
        "<?php namespace Lib;\n\
         /**\n\
          * @template TKey\n\
          * @template-covariant T\n\
          * @template-extends \\IteratorAggregate<TKey, T>\n\
          */\n\
         interface C extends \\IteratorAggregate {}",
        "Lib\\C",
    );
    assert_eq!(shape.templates, ["Lib\\TKey", "Lib\\T"]);
    assert_eq!(
        shape.generic_supertypes,
        [(
            "IteratorAggregate".to_string(),
            vec![named("Lib\\TKey"), named("Lib\\T")]
        )]
    );
}

#[test]
fn hover_shows_generic_arguments() {
    let mut index = build();
    let (path, source, offset) = at("app/Hover.php", "$c;", 0);
    let hover = index.hover(&path, &source, offset).expect("a hover");
    assert!(
        hover
            .markdown
            .contains("\\Lib\\Collection<mixed, \\App\\Item> $c"),
        "{}",
        hover.markdown
    );
}

/// The `label` method of `App\Item`.
fn item_label() -> [(String, String); 1] {
    [("app/Item.php".to_string(), "label".to_string())]
}

/// Go to definition on the `nth` `label();` of `file`.
fn label(index: &mut Index, file: &str, nth: usize) -> Vec<(String, String)> {
    described(&definition(index, file, "label();", nth))
}

#[test]
fn foreach_over_a_generic_collection() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 0), item_label());
}

#[test]
fn foreach_over_a_concrete_collection_subclass() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Uses.php", 0), item_label());
}

#[test]
fn foreach_over_single_argument_iterables() {
    let mut index = build();
    assert_eq!(
        label(&mut index, "app/Uses.php", 1),
        item_label(),
        "Iterator"
    );
    assert_eq!(
        label(&mut index, "app/Uses.php", 2),
        item_label(),
        "Generator"
    );
}

#[test]
fn foreach_over_a_nullable_typed_array() {
    let mut index = build();
    assert_eq!(
        label(&mut index, "app/Uses.php", 3),
        item_label(),
        "foreach"
    );
    assert_eq!(label(&mut index, "app/Uses.php", 4), item_label(), "access");
}

#[test]
fn array_access_on_typed_arrays() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 1), item_label());
    assert_eq!(label(&mut index, "app/Shelf.php", 2), item_label());
}

#[test]
fn array_access_on_a_generic_collection() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 3), item_label());
}

#[test]
fn undeclared_elements_give_nothing() {
    let mut index = build();
    for nth in [4, 5, 6] {
        assert!(label(&mut index, "app/Shelf.php", nth).is_empty(), "{nth}");
    }
}

#[test]
fn cyclic_generic_supertypes_end() {
    let mut index = build();
    assert!(label(&mut index, "app/Uses.php", 5).is_empty());
}

#[test]
fn template_returns_take_the_receiver_arguments() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 7), item_label());
}

#[test]
fn template_arrays_take_the_receiver_arguments() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 8), item_label());
}

#[test]
fn generic_returns_continue_the_chain() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Shelf.php", 9), item_label());
}

#[test]
fn fewer_arguments_leave_templates_unknown() {
    let mut index = build();
    assert_eq!(label(&mut index, "app/Uses.php", 6), item_label());
    assert!(label(&mut index, "app/Uses.php", 7).is_empty());
}

#[test]
fn unbound_templates_give_nothing() {
    let mut index = build();
    assert!(label(&mut index, "app/Shelf.php", 10).is_empty());
}
