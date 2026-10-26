use phpnav_index::summary::{FileSummary, NameKind, NameRef, SymbolKind, summarize};

fn php(body: &str) -> Vec<u8> {
    format!("<?php\n{body}").into_bytes()
}

fn declarations(summary: &FileSummary) -> Vec<(SymbolKind, &str)> {
    summary
        .declarations
        .iter()
        .map(|d| (d.kind, d.fqn.as_str()))
        .collect()
}

/// The name references whose source text is `text`, in source order.
fn refs_named<'a>(summary: &'a FileSummary, source: &[u8], text: &str) -> Vec<&'a NameRef> {
    let mut refs: Vec<&NameRef> = summary
        .names
        .iter()
        .filter(|n| &source[n.start as usize..n.end as usize] == text.as_bytes())
        .collect();
    refs.sort_by_key(|n| n.start);
    refs
}

#[test]
fn declarations_in_namespace() {
    let source = php(
        "namespace App\\Model; class Foo {} interface Bar {} trait Baz {} enum Qux {} function helper() {}",
    );

    let summary = summarize(&source);

    assert_eq!(
        declarations(&summary),
        vec![
            (SymbolKind::Class, "App\\Model\\Foo"),
            (SymbolKind::Interface, "App\\Model\\Bar"),
            (SymbolKind::Trait, "App\\Model\\Baz"),
            (SymbolKind::Enum, "App\\Model\\Qux"),
            (SymbolKind::Function, "App\\Model\\helper"),
        ]
    );
    let foo = &summary.declarations[0];
    assert_eq!(
        &source[foo.name_start as usize..foo.name_end as usize],
        b"Foo"
    );
}

#[test]
fn declarations_in_global_namespace() {
    let summary = summarize(&php("class Foo {}"));

    assert_eq!(declarations(&summary), vec![(SymbolKind::Class, "Foo")]);
}

#[test]
fn braced_namespaces() {
    let summary = summarize(&php(
        "namespace A { class X {} } namespace B { class X {} }",
    ));

    assert_eq!(
        declarations(&summary),
        vec![(SymbolKind::Class, "A\\X"), (SymbolKind::Class, "B\\X")]
    );
}

#[test]
fn name_refs_resolve_imports() {
    let source =
        php("namespace App; use Lib\\Thing as Alias; new Alias(); helper(); \\strlen('x'); FOO;");

    let summary = summarize(&source);

    let alias = refs_named(&summary, &source, "Alias");
    assert_eq!(alias.len(), 1);
    assert_eq!(
        (alias[0].kind, alias[0].fqn.as_str()),
        (NameKind::ClassLike, "Lib\\Thing")
    );

    let helper = refs_named(&summary, &source, "helper");
    assert_eq!(helper.len(), 1);
    assert_eq!(
        (helper[0].kind, helper[0].qualified),
        (NameKind::Function, false)
    );

    let strlen = refs_named(&summary, &source, "\\strlen");
    assert_eq!(strlen.len(), 1);
    assert_eq!(
        (strlen[0].kind, strlen[0].qualified),
        (NameKind::Function, true)
    );

    let foo = refs_named(&summary, &source, "FOO");
    assert_eq!(foo.len(), 1);
    assert_eq!(foo[0].kind, NameKind::Constant);
}

#[test]
fn namespace_names_are_not_refs() {
    let source = php("namespace App; class Foo {}");

    let summary = summarize(&source);

    assert!(refs_named(&summary, &source, "App").is_empty());
}

#[test]
fn class_and_function_with_same_name() {
    let source = php("function Foo() {} new Foo(); Foo();");

    let summary = summarize(&source);

    let kinds: Vec<NameKind> = refs_named(&summary, &source, "Foo")
        .iter()
        .filter(|n| n.start > summary.declarations[0].name_end)
        .map(|n| n.kind)
        .collect();
    assert_eq!(kinds, vec![NameKind::ClassLike, NameKind::Function]);
}

#[test]
fn use_function_items_are_function_refs() {
    let source = php("use function Lib\\helper; use Lib\\{Thing, function other};");

    let summary = summarize(&source);

    assert_eq!(
        refs_named(&summary, &source, "Lib\\helper")[0].kind,
        NameKind::Function
    );
    assert_eq!(
        refs_named(&summary, &source, "Thing")[0].kind,
        NameKind::ClassLike
    );
    assert_eq!(
        refs_named(&summary, &source, "other")[0].kind,
        NameKind::Function
    );
}

#[test]
fn attribute_names_are_class_refs() {
    let source =
        php("use Drupal\\Core\\Hook\\Attribute\\Hook; #[Hook('form_alter')] function f() {}");

    let summary = summarize(&source);

    let hook = refs_named(&summary, &source, "Hook");
    assert_eq!(hook.len(), 1);
    assert_eq!(
        (hook[0].kind, hook[0].fqn.as_str()),
        (NameKind::ClassLike, "Drupal\\Core\\Hook\\Attribute\\Hook")
    );
}

#[test]
fn recovers_after_syntax_error() {
    let summary = summarize(&php("class A { public function f( } class B {}"));

    assert!(summary.has_parse_errors);
    assert!(
        summary
            .declarations
            .iter()
            .any(|d| d.kind == SymbolKind::Class && d.fqn == "B")
    );
}

#[test]
fn tolerates_invalid_utf8() {
    let summary = summarize(b"<?php\n// \xE8\nclass Foo {}");

    assert_eq!(declarations(&summary), vec![(SymbolKind::Class, "Foo")]);
}

#[test]
fn parses_php_8_4_and_8_5_syntax() {
    let summary = summarize(&php(
        "class H { public string $n { get => 'x'; } } $r = $x |> trim(...); class After {}",
    ));

    assert!(!summary.has_parse_errors);
    assert!(summary.declarations.iter().any(|d| d.fqn == "After"));
}
