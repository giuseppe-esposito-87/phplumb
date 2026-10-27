use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::Index;
use phplumb_index::discover::DiscoverOptions;
use phplumb_index::summary::{MemberKind, SymbolKind};
use phplumb_index::symbols::{Kind, OutlineSymbol, outline};
use tempfile::TempDir;

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

fn workspace(files: &[(&str, &str)]) -> (TempDir, PathBuf, Index) {
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

/// Matches as `name container path`, best first.
fn search(index: &Index, root: &Path, query: &str, limit: usize) -> Vec<String> {
    index
        .workspace_symbols(query, limit)
        .into_iter()
        .map(|m| {
            let path = m
                .location
                .path
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string();
            format!("{} {} {path}", m.name, m.container.unwrap_or_default())
        })
        .collect()
}

#[test]
fn symbols_rank_exact_then_prefix_then_substring_then_subsequence() {
    let (_dir, root, index) = workspace(&[(
        "src/All.php",
        "namespace App; class NamedObjectDefault {} class BaseNode {} class NodeStorage {} class Node {}",
    )]);

    assert_eq!(
        search(&index, &root, "node", 10),
        vec![
            "Node App src/All.php",
            "NodeStorage App src/All.php",
            "BaseNode App src/All.php",
            "NamedObjectDefault App src/All.php",
        ]
    );
}

#[test]
fn symbols_match_camel_humps_and_any_case() {
    let (_dir, root, index) = workspace(&[(
        "src/Repo.php",
        "namespace App; class Repo { public function getEntity() {} }",
    )]);

    assert_eq!(
        search(&index, &root, "gEnt", 10),
        vec!["getEntity App\\Repo src/Repo.php"]
    );
    assert_eq!(
        search(&index, &root, "GETENTITY", 10),
        vec!["getEntity App\\Repo src/Repo.php"]
    );
}

#[test]
fn symbols_with_a_backslash_match_qualified_names() {
    let (_dir, root, index) = workspace(&[
        ("src/A.php", "namespace App; class Node {}"),
        ("src/B.php", "namespace Other; class Node {}"),
    ]);

    assert_eq!(
        search(&index, &root, "app\\node", 10),
        vec!["Node App src/A.php"]
    );
    assert_eq!(
        search(&index, &root, "\\App\\Node", 10),
        vec!["Node App src/A.php"]
    );
}

#[test]
fn symbols_list_project_code_before_core() {
    let (_dir, root, index) = workspace(&[
        ("web/core/lib/Node.php", "namespace Core; class Node {}"),
        ("src/Node.php", "namespace App; class Node {}"),
    ]);

    assert_eq!(
        search(&index, &root, "Node", 10),
        vec!["Node App src/Node.php", "Node Core web/core/lib/Node.php"]
    );
}

#[test]
fn symbols_respect_the_limit_and_ignore_blank_queries() {
    let (_dir, root, index) = workspace(&[(
        "src/All.php",
        "namespace App; class Node {} class NodeStorage {} class BaseNode {}",
    )]);

    assert_eq!(search(&index, &root, "node", 2).len(), 2);
    assert_eq!(search(&index, &root, "  ", 10), Vec::<String>::new());
}
