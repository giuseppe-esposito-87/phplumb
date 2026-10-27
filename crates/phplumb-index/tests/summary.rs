use phplumb_index::phpdoc::TypeExpr;
use phplumb_index::summary::{
    ClassShape, Declaration, FileSummary, Member, MemberKind, NameKind, NameRef, SymbolKind,
    summarize,
};

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

fn class<'a>(summary: &'a FileSummary, fqn: &str) -> &'a Declaration {
    summary
        .declarations
        .iter()
        .find(|d| d.fqn == fqn)
        .unwrap_or_else(|| panic!("no declaration {fqn}"))
}

fn shape<'a>(summary: &'a FileSummary, fqn: &str) -> &'a ClassShape {
    class(summary, fqn).class.as_ref().expect("a class shape")
}

fn member<'a>(shape: &'a ClassShape, name: &str) -> &'a Member {
    shape
        .members
        .iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no member {name}"))
}

fn named(fqn: &str) -> Option<TypeExpr> {
    Some(TypeExpr::Named(fqn.to_string()))
}

#[test]
fn class_shape_lists_parent_interfaces_and_traits() {
    let summary = summarize(&php(
        "namespace App; use Lib\\Base; class Foo extends Base implements \\Countable, Bar { use T1, \\Lib\\T2; }",
    ));

    let foo = shape(&summary, "App\\Foo");
    assert_eq!(foo.parent.as_deref(), Some("Lib\\Base"));
    assert_eq!(foo.interfaces, vec!["Countable", "App\\Bar"]);
    assert_eq!(foo.traits, vec!["App\\T1", "Lib\\T2"]);
}

#[test]
fn members_with_types() {
    let source = php(
        "namespace App;\nclass Foo {\n  const A = 1;\n  public static ?Bar $p;\n  /** @var Baz[] */\n  protected array $list;\n  public function __construct(private Qux $q) {}\n  /** @return static */\n  public function make(): self {}\n}",
    );
    let summary = summarize(&source);
    let foo = shape(&summary, "App\\Foo");

    assert_eq!(member(foo, "A").kind, MemberKind::Constant);
    let p = member(foo, "p");
    assert_eq!(
        (p.kind, p.is_static, p.ty.clone()),
        (MemberKind::Property, true, named("App\\Bar"))
    );
    assert_eq!(
        member(foo, "list").ty,
        Some(TypeExpr::ArrayOf(Box::new(TypeExpr::Named(
            "App\\Baz".to_string()
        ))))
    );
    let q = member(foo, "q");
    assert_eq!(
        (q.kind, q.ty.clone()),
        (MemberKind::Property, named("App\\Qux"))
    );
    let make = member(foo, "make");
    assert_eq!(
        (make.kind, make.ty.clone()),
        (MemberKind::Method, Some(TypeExpr::StaticType))
    );
    assert_eq!(
        &source[make.name_start as usize..make.name_end as usize],
        b"make"
    );
    assert_eq!(make.signature, "public function make(): self");
    assert_eq!(
        member(foo, "__construct").params,
        vec![("q".to_string(), named("App\\Qux"))]
    );
}

#[test]
fn enum_cases_and_interface_extends() {
    let summary = summarize(&php(
        "namespace App; enum Suit: string { case Hearts = 'h'; } interface I extends J, K {}",
    ));

    assert_eq!(
        member(shape(&summary, "App\\Suit"), "Hearts").kind,
        MemberKind::EnumCase
    );
    assert_eq!(
        shape(&summary, "App\\I").interfaces,
        vec!["App\\J", "App\\K"]
    );
}

#[test]
fn function_returns_and_signatures() {
    let summary = summarize(&php(
        "namespace App;\n/**\n * Builds.\n * @return Foo\n */\nfunction build(int $a = 1): ?object {}",
    ));

    let build = class(&summary, "App\\build");
    assert_eq!(build.returns, named("App\\Foo"));
    assert_eq!(build.signature, "function build(int $a = 1): ?object");
    assert!(build.doc.as_deref().unwrap().contains("Builds."));
}

#[test]
fn global_constants() {
    let summary = summarize(&php(
        "namespace App; const LIMIT = 10; define('GLOBAL_LIMIT', 5);",
    ));

    assert_eq!(class(&summary, "App\\LIMIT").kind, SymbolKind::Constant);
    assert_eq!(class(&summary, "GLOBAL_LIMIT").kind, SymbolKind::Constant);
}

#[test]
fn abstract_flags() {
    let summary = summarize(&php(
        "abstract class A { abstract function f(); function g() {} } interface I { function h(); } class C {}",
    ));

    let a = shape(&summary, "A");
    assert!(a.is_abstract);
    assert!(member(a, "f").is_abstract);
    assert!(!member(a, "g").is_abstract);
    let i = shape(&summary, "I");
    assert!(i.is_abstract);
    assert!(member(i, "h").is_abstract);
    assert!(!shape(&summary, "C").is_abstract);
}

#[test]
fn magic_marks_call_handlers_and_docblock_tags() {
    let summary = summarize(
        b"<?php
class Calls { public function __call($n, $a) {} }
class StaticCalls { public static function __callStatic($n, $a) {} }
/** @method int foo() */
class Documented {}
/**
 * Mixes in another class.
 *
 * @mixin Other
 */
class Mixed {}
/** A plain class. */
class Plain { public function run() {} }
",
    );

    let magic: Vec<(&str, bool)> = summary
        .declarations
        .iter()
        .map(|d| (d.fqn.as_str(), d.class.as_ref().unwrap().magic))
        .collect();
    assert_eq!(
        magic,
        vec![
            ("Calls", true),
            ("StaticCalls", true),
            ("Documented", true),
            ("Mixed", true),
            ("Plain", false),
        ]
    );
}

#[test]
fn guarded_collects_exists_checks() {
    let summary = summarize(
        b"<?php
namespace App;
use Lib\\Foo;
if (class_exists(Foo::class)) {}
if (\\class_exists('Lib\\\\Bar')) {}
function_exists('helper');
method_exists($x, 'run');
defined('LIMIT');
",
    );

    let mut guarded = summary.guarded.clone();
    guarded.sort();
    assert_eq!(
        guarded,
        vec!["helper", "lib\\bar", "lib\\foo", "limit", "run"]
    );
}

fn metadata_of<'a>(summary: &'a FileSummary, fqn: &str) -> &'a [phplumb_index::metadata::Metadata] {
    &summary
        .declarations
        .iter()
        .find(|d| d.fqn == fqn)
        .unwrap()
        .class
        .as_ref()
        .unwrap()
        .metadata
}

#[test]
fn attributes_are_recorded_with_resolved_values() {
    use phplumb_index::metadata::MetaValue;
    let summary = summarize(&php("namespace App;
use Drupal\\Core\\Entity\\Attribute\\ContentEntityType;
use App\\Storage;
#[ContentEntityType(
  id: 'thing',
  label: new T('X'),
  handlers: ['storage' => Storage::class, 'form' => ['default' => F::class]],
)]
class Thing {}"));

    let metadata = metadata_of(&summary, "App\\Thing");

    assert_eq!(metadata.len(), 1);
    let entity = &metadata[0];
    assert_eq!(
        entity.name,
        "Drupal\\Core\\Entity\\Attribute\\ContentEntityType"
    );
    assert_eq!(
        entity.argument("id"),
        Some(&MetaValue::String("thing".to_string()))
    );
    let Some(MetaValue::List(handlers)) = entity.argument("handlers") else {
        panic!("{:?}", entity.argument("handlers"));
    };
    assert_eq!(
        handlers[0],
        (
            Some("storage".to_string()),
            MetaValue::Class("App\\Storage".to_string())
        )
    );
    let Some(MetaValue::Nested(label)) = entity.argument("label") else {
        panic!("{:?}", entity.argument("label"));
    };
    assert_eq!(label.name, "App\\T");
}

#[test]
fn annotations_are_recorded_with_resolved_names() {
    use phplumb_index::metadata::MetaValue;
    let summary = summarize(&php("namespace App;
use Drupal\\Core\\Entity\\Annotation\\ContentEntityType;
/**
 * Docs about the thing.
 *
 * @ContentEntityType(
 *   id = \"thing\",
 *   label = @Translation(\"Thing\"),
 *   handlers = {
 *     \"storage\" = \"App\\Storage\",
 *   },
 *   revisionable = TRUE,
 * )
 */
class Thing {}"));

    let metadata = metadata_of(&summary, "App\\Thing");

    assert_eq!(metadata.len(), 1, "{metadata:?}");
    let entity = &metadata[0];
    assert_eq!(
        entity.name,
        "Drupal\\Core\\Entity\\Annotation\\ContentEntityType"
    );
    assert_eq!(
        entity.argument("id"),
        Some(&MetaValue::String("thing".to_string()))
    );
    assert_eq!(
        entity.argument("revisionable"),
        Some(&MetaValue::Bool(true))
    );
    let Some(MetaValue::List(handlers)) = entity.argument("handlers") else {
        panic!("{:?}", entity.argument("handlers"));
    };
    assert_eq!(
        handlers[0],
        (
            Some("storage".to_string()),
            MetaValue::String("App\\Storage".to_string())
        )
    );
    let Some(MetaValue::Nested(label)) = entity.argument("label") else {
        panic!("{:?}", entity.argument("label"));
    };
    assert_eq!(label.name, "App\\Translation");
}

#[test]
fn broken_annotations_give_nothing() {
    let summary = summarize(&php(
        "/**\n * @ContentEntityType(\n *   id = \"x\",\n */\nclass Broken {}",
    ));

    assert_eq!(metadata_of(&summary, "Broken"), &[]);
}
