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

#[allow(dead_code)]
fn definition(index: &mut Index, file: &str, needle: &str, nth: usize) -> Vec<Location> {
    let (path, source, offset) = at(file, needle, nth);
    index.definition(&path, &source, offset)
}

/// Each location as its path relative to the fixture and the text at its range.
#[allow(dead_code)]
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
