use phpnav_index::summary::{MemberKind, SymbolKind};
use phpnav_index::symbols::{Kind, OutlineSymbol, outline};

fn names(symbols: &[OutlineSymbol]) -> Vec<(&str, Kind)> {
    symbols.iter().map(|s| (s.name.as_str(), s.kind)).collect()
}

fn all(symbols: &[OutlineSymbol]) -> Vec<&OutlineSymbol> {
    symbols
        .iter()
        .flat_map(|s| std::iter::once(s).chain(all(&s.children)))
        .collect()
}

const SHAPES: &str = "<?php
namespace App;
const LIMIT = 1;
function helper() {}
interface I { function run(); }
trait T { public $t; }
enum E: string { case A = 'a'; const D = self::A; }
class C implements I {
    use T;
    const X = 1, Y = 2;
    public function __construct(private int $p) {}
    public function run() {}
}
";

#[test]
fn outline_nests_members_in_source_order() {
    let symbols = outline(SHAPES.as_bytes());

    use Kind::{Declaration as D, Member as M};
    assert_eq!(
        names(&symbols),
        vec![
            ("LIMIT", D(SymbolKind::Constant)),
            ("helper", D(SymbolKind::Function)),
            ("I", D(SymbolKind::Interface)),
            ("T", D(SymbolKind::Trait)),
            ("E", D(SymbolKind::Enum)),
            ("C", D(SymbolKind::Class)),
        ]
    );
    assert_eq!(
        names(&symbols[4].children),
        vec![
            ("A", M(MemberKind::EnumCase)),
            ("D", M(MemberKind::Constant))
        ]
    );
    assert_eq!(
        names(&symbols[5].children),
        vec![
            ("X", M(MemberKind::Constant)),
            ("Y", M(MemberKind::Constant)),
            ("__construct", M(MemberKind::Method)),
            ("p", M(MemberKind::Property)),
            ("run", M(MemberKind::Method)),
        ]
    );
}

#[test]
fn outline_ranges_contain_their_names() {
    let symbols = outline(SHAPES.as_bytes());

    for symbol in all(&symbols) {
        assert!(
            symbol.range.0 <= symbol.selection.0 && symbol.selection.1 <= symbol.range.1,
            "{symbol:?}"
        );
        let name = &SHAPES[symbol.selection.0 as usize..symbol.selection.1 as usize];
        assert_eq!(name.trim_start_matches('$'), symbol.name);
    }
}

#[test]
fn outline_skips_anonymous_classes() {
    let symbols = outline(b"<?php\n$x = new class { function f() {} };\nfunction g() {}\n");

    assert_eq!(
        names(&symbols),
        vec![("g", Kind::Declaration(SymbolKind::Function))]
    );
}

#[test]
fn outline_survives_syntax_errors() {
    let symbols =
        outline(b"<?php\nclass A { function a() { $x = ; } }\nfunction f(, ) {}\nclass B {}\n");

    let top: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(top, vec!["A", "f", "B"]);
    assert_eq!(symbols[0].children.len(), 1);
}
