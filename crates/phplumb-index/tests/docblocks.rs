//! Names written in docblocks. Marker fixtures cannot sit inside a docblock, so
//! cursors are found by needle.

use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::discover::DiscoverOptions;
use phplumb_index::{Index, RefKey};
use tempfile::TempDir;

const BASE: &str =
    "<?php\nnamespace Lib;\n\nclass Base\n{\n    public function inherited() {}\n}\n";
const FOO: &str = "<?php\nnamespace Lib;\n\nclass Foo extends Base implements FooInterface\n{\n    const LIMIT = 1;\n    /**\n     * @see self::run()\n     */\n    public $name;\n    public function run() {}\n}\n";
const FUNCTIONS: &str = "<?php\nnamespace Lib;\n\nfunction helper() {}\n";
const GLOBAL: &str = "<?php\n\nfunction global_helper() {}\n";
const ROUTE: &str = "<?php\nnamespace Lib\\Ann;\n\nclass Route\n{\n}\n";
const FOO_INTERFACE: &str = "<?php\nnamespace Lib;\n\ninterface FooInterface\n{\n}\n";
const FOO_EXCEPTION: &str =
    "<?php\nnamespace Lib;\n\nclass FooException extends \\Exception\n{\n}\n";
const THING: &str = "<?php\nnamespace Lib\\Sub;\n\nclass Thing\n{\n}\n";

const USE: &str = r#"<?php
namespace App;

use Lib\Ann\Route;
use Lib\Foo;
use Lib\FooInterface;

/**
 * Builds a Foo.
 *
 * @internal
 * @property Foo $p
 * @method Foo make(FooInterface $f)
 * @mixin Foo
 * @extends \Lib\Base<Foo>
 * @template T of Foo
 * @see Foo
 * @see Foo::run()
 * @see https://example.com/Foo
 */
class Consumer
{
    /**
     * @var \Lib\FooInterface
     */
    private $foo;

    /**
     * @var \Lib\Sub\Thing
     */
    private $thing;

    /**
     * @var \Nowhere\Missing
     */
    private $missing;

    /**
     * Crée un élément.
     *
     * @var Foo
     */
    private $created;

    /**
     * @param Foo $foo Uses a Bar.
     * @param array<string, Foo> $map
     * @phpstan-param Foo $x
     * @return Foo[]|null
     * @throws \Lib\FooException
     */
    public function run($foo, $map, $x)
    {
        /** @var Foo $local */
        $local = $foo;
        return null;
    }

    /**
     * @param array<string,
     *   Foo> $continued
     * @var int
     * @return T
     */
    public function generic($continued)
    {
    }

    /**
     * @return $this
     */
    public function fluent()
    {
        return $this;
    }

    /**
     * @see Foo::run() for the Bar::stop() case
     * @see Foo::$name
     * @see \Lib\Foo::LIMIT
     * @covers \Lib\Foo::run
     * @see Foo::inherited()
     * @see Foo::missing()
     * @see https://example.com/a::b()
     * @see \Lib\helper()
     * @see global_helper()
     */
    public function members()
    {
    }

    /**
     * @Route("/x")
     */
    public function routed()
    {
    }

    /**
     * Declares a route like this:
     * @code
     * @Route("/example")
     * @endcode
     */
    public function documented()
    {
    }
}
"#;

const BRACED: &str = r#"<?php
namespace Lib {
    /** @var Foo $a */
    function braced_a($a) {}
}

namespace Other {
    /** @var Foo $b */
    function braced_b($b) {}
}
"#;

fn workspace() -> (TempDir, PathBuf, Index) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let files = [
        ("lib/Base.php", BASE),
        ("lib/Foo.php", FOO),
        ("lib/FooInterface.php", FOO_INTERFACE),
        ("lib/FooException.php", FOO_EXCEPTION),
        ("lib/Sub/Thing.php", THING),
        ("Use.php", USE),
        ("Braced.php", BRACED),
        ("lib/functions.php", FUNCTIONS),
        ("global.php", GLOBAL),
        ("lib/Ann/Route.php", ROUTE),
    ];
    for (path, content) in files {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    let index = Index::build(&root, &DiscoverOptions::default()).unwrap().0;
    (dir, root, index)
}

/// The offset of `word` inside the first occurrence of `needle` in `file`.
fn offset(root: &Path, file: &str, needle: &str, word: &str) -> u32 {
    let text = fs::read_to_string(root.join(file)).unwrap();
    let at = text
        .find(needle)
        .unwrap_or_else(|| panic!("{needle} not in {file}"));
    let inside = needle
        .find(word)
        .unwrap_or_else(|| panic!("{word} not in {needle}"));
    (at + inside) as u32
}

/// Each location as `path: text`, sorted.
fn described(root: &Path, locations: Vec<phplumb_index::Location>) -> Vec<String> {
    let mut found: Vec<String> = locations
        .into_iter()
        .map(|l| {
            let content = fs::read_to_string(&l.path).unwrap();
            let name = l.path.strip_prefix(root).unwrap().display().to_string();
            format!("{name}: {}", &content[l.start as usize..l.end as usize])
        })
        .collect();
    found.sort();
    found
}

fn definitions(
    index: &mut Index,
    root: &Path,
    file: &str,
    needle: &str,
    word: &str,
) -> Vec<String> {
    let path = root.join(file);
    let source = fs::read(&path).unwrap();
    let at = offset(root, file, needle, word);
    let found = index.definition(&path, &source, at);
    described(root, found)
}

fn references(index: &mut Index, root: &Path, file: &str, needle: &str, word: &str) -> Vec<String> {
    let path = root.join(file);
    let source = fs::read(&path).unwrap();
    let at = offset(root, file, needle, word);
    let found = index.references(&path, &source, at, false);
    described(root, found)
}

#[test]
fn docblock_class_names_lead_to_their_declarations() {
    let (_dir, root, mut index) = workspace();
    let cases = [
        (
            "@var \\Lib\\FooInterface",
            "FooInterface",
            "lib/FooInterface.php: FooInterface",
        ),
        ("@param Foo $foo", "Foo", "lib/Foo.php: Foo"),
        ("@return Foo[]|null", "Foo", "lib/Foo.php: Foo"),
        (
            "@throws \\Lib\\FooException",
            "FooException",
            "lib/FooException.php: FooException",
        ),
        ("@param array<string, Foo> $map", "Foo", "lib/Foo.php: Foo"),
        ("@property Foo $p", "Foo", "lib/Foo.php: Foo"),
        ("@method Foo make", "Foo", "lib/Foo.php: Foo"),
        (
            "make(FooInterface $f)",
            "FooInterface",
            "lib/FooInterface.php: FooInterface",
        ),
        ("@template T of Foo", "Foo", "lib/Foo.php: Foo"),
        ("@see Foo\n", "Foo", "lib/Foo.php: Foo"),
        ("@see Foo::run()", "Foo", "lib/Foo.php: Foo"),
        ("@phpstan-param Foo $x", "Foo", "lib/Foo.php: Foo"),
        ("@mixin Foo", "Foo", "lib/Foo.php: Foo"),
        ("@extends \\Lib\\Base<Foo>", "Base", "lib/Base.php: Base"),
        ("@extends \\Lib\\Base<Foo>", "Foo", "lib/Foo.php: Foo"),
        ("/** @var Foo $local */", "Foo", "lib/Foo.php: Foo"),
        (
            "@var \\Lib\\Sub\\Thing",
            "Thing",
            "lib/Sub/Thing.php: Thing",
        ),
    ];
    for (needle, word, expected) in cases {
        assert_eq!(
            definitions(&mut index, &root, "Use.php", needle, word),
            vec![expected],
            "{needle}"
        );
    }
}

#[test]
fn docblock_words_that_are_not_classes_lead_nowhere() {
    let (_dir, root, mut index) = workspace();
    let cases = [
        ("@var int", "int"),
        ("@return $this", "this"),
        ("@return Foo[]|null", "null"),
        ("@param Foo $foo", "foo"),
        ("@return T", "T"),
        ("Builds a Foo.", "Foo"),
        ("Uses a Bar.", "Bar"),
        ("https://example.com/Foo", "Foo"),
        ("@var \\Nowhere\\Missing", "Missing"),
    ];
    for (needle, word) in cases {
        assert_eq!(
            definitions(&mut index, &root, "Use.php", needle, word),
            Vec::<String>::new(),
            "{needle}"
        );
    }
}

#[test]
fn continued_types_lead_to_the_right_class_or_nowhere() {
    let (_dir, root, mut index) = workspace();

    let found = definitions(&mut index, &root, "Use.php", "*   Foo> $continued", "Foo");

    assert!(
        found.is_empty() || found == vec!["lib/Foo.php: Foo"],
        "{found:?}"
    );
}

#[test]
fn non_ascii_text_keeps_docblock_ranges() {
    let (_dir, root, mut index) = workspace();

    assert_eq!(
        definitions(
            &mut index,
            &root,
            "Use.php",
            "élément.\n     *\n     * @var Foo",
            "Foo"
        ),
        vec!["lib/Foo.php: Foo"]
    );
}

#[test]
fn braced_namespaces_resolve_their_own_docblocks() {
    let (_dir, root, mut index) = workspace();

    assert_eq!(
        definitions(&mut index, &root, "Braced.php", "@var Foo $a", "Foo"),
        vec!["lib/Foo.php: Foo"]
    );
    assert_eq!(
        definitions(&mut index, &root, "Braced.php", "@var Foo $b", "Foo"),
        Vec::<String>::new()
    );
}

#[test]
fn hover_on_a_docblock_class_name_shows_the_class() {
    let (_dir, root, mut index) = workspace();
    let path = root.join("Use.php");
    let source = fs::read(&path).unwrap();
    let at = offset(&root, "Use.php", "@var \\Lib\\FooInterface", "FooInterface");

    let hover = index.hover(&path, &source, at).map(|h| h.markdown);

    assert!(
        hover
            .as_deref()
            .is_some_and(|h| h.contains("interface FooInterface")),
        "{hover:?}"
    );
}

#[test]
fn implementations_of_a_docblock_interface_name_are_its_implementers() {
    let (_dir, root, mut index) = workspace();
    let path = root.join("Use.php");
    let source = fs::read(&path).unwrap();
    let at = offset(&root, "Use.php", "@var \\Lib\\FooInterface", "FooInterface");

    let found = index.implementations(&path, &source, at);

    assert_eq!(described(&root, found), vec!["lib/Foo.php: Foo"]);
}

#[test]
fn references_of_a_class_include_docblock_mentions() {
    let (_dir, root, mut index) = workspace();

    let found = references(
        &mut index,
        &root,
        "lib/FooException.php",
        "class FooException",
        "FooException",
    );

    assert!(
        found
            .iter()
            .any(|f| f.starts_with("Use.php: ") && f.ends_with("FooException")),
        "{found:?}"
    );
    let candidates: Vec<String> = index
        .candidate_files(&RefKey::Class("Lib\\FooException".to_string()))
        .into_iter()
        .map(|p| p.strip_prefix(&root).unwrap().display().to_string())
        .collect();
    assert!(
        candidates.contains(&"Use.php".to_string()),
        "{candidates:?}"
    );
}

#[test]
fn docblock_only_mentions_follow_updates() {
    let (_dir, root, mut index) = workspace();
    fs::write(
        root.join("Later.php"),
        "<?php\nnamespace App;\n\nclass Later\n{\n}\n",
    )
    .unwrap();
    index.update(&[root.join("Later.php")], &[]);
    fs::write(
        root.join("Later.php"),
        "<?php\nnamespace App;\n\nclass Later\n{\n    /**\n     * @var \\Lib\\Sub\\Thing\n     */\n    private $thing;\n}\n",
    )
    .unwrap();
    index.update(&[root.join("Later.php")], &[]);

    let found = references(
        &mut index,
        &root,
        "lib/Sub/Thing.php",
        "class Thing",
        "Thing",
    );

    assert!(
        found.iter().any(|f| f.starts_with("Later.php: ")),
        "{found:?}"
    );
}

#[test]
fn docblock_names_are_not_diagnosed() {
    let (_dir, root, index) = workspace();
    let path = root.join("Use.php");
    let source = fs::read(&path).unwrap();

    let found: Vec<String> = index
        .diagnostics(&path, &source)
        .into_iter()
        .map(|d| d.message)
        .collect();

    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn see_members_lead_to_their_declarations() {
    let (_dir, root, mut index) = workspace();
    let cases = [
        ("Use.php", "@see Foo::run() for", "run", "lib/Foo.php: run"),
        ("Use.php", "@see Foo::$name", "$name", "lib/Foo.php: name"),
        (
            "Use.php",
            "@see \\Lib\\Foo::LIMIT",
            "LIMIT",
            "lib/Foo.php: LIMIT",
        ),
        (
            "Use.php",
            "@covers \\Lib\\Foo::run",
            "run",
            "lib/Foo.php: run",
        ),
        (
            "Use.php",
            "@see Foo::inherited()",
            "inherited",
            "lib/Base.php: inherited",
        ),
        ("lib/Foo.php", "@see self::run()", "run", "lib/Foo.php: run"),
    ];
    for (file, needle, word, expected) in cases {
        assert_eq!(
            definitions(&mut index, &root, file, needle, word),
            vec![expected],
            "{needle}"
        );
    }
}

#[test]
fn see_functions_lead_to_their_declarations() {
    let (_dir, root, mut index) = workspace();

    assert_eq!(
        definitions(
            &mut index,
            &root,
            "Use.php",
            "@see \\Lib\\helper()",
            "helper"
        ),
        vec!["lib/functions.php: helper"]
    );
    assert_eq!(
        definitions(
            &mut index,
            &root,
            "Use.php",
            "@see global_helper()",
            "global_helper"
        ),
        vec!["global.php: global_helper"]
    );
}

#[test]
fn see_words_that_are_not_members_lead_nowhere() {
    let (_dir, root, mut index) = workspace();
    let cases = [
        ("@see Foo::missing()", "missing"),
        ("https://example.com/a::b()", "b()"),
        ("the Bar::stop() case", "stop"),
    ];
    for (needle, word) in cases {
        assert_eq!(
            definitions(&mut index, &root, "Use.php", needle, word),
            Vec::<String>::new(),
            "{needle}"
        );
    }
}

#[test]
fn references_of_a_method_include_see_mentions() {
    let (_dir, root, mut index) = workspace();

    let found = references(&mut index, &root, "lib/Foo.php", "function run", "run");

    let in_use = found.iter().filter(|f| *f == "Use.php: run").count();
    assert_eq!(in_use, 3, "{found:?}");
    assert!(found.contains(&"lib/Foo.php: run".to_string()), "{found:?}");
}

#[test]
fn hover_on_a_see_member_shows_the_member() {
    let (_dir, root, mut index) = workspace();
    let path = root.join("Use.php");
    let source = fs::read(&path).unwrap();
    let at = offset(&root, "Use.php", "@see Foo::run() for", "run");

    let hover = index.hover(&path, &source, at).map(|h| h.markdown);

    assert!(
        hover
            .as_deref()
            .is_some_and(|h| h.contains("Lib\\Foo::run()")),
        "{hover:?}"
    );
}

#[test]
fn imported_annotation_names_lead_to_their_class() {
    let (_dir, root, mut index) = workspace();

    assert_eq!(
        definitions(&mut index, &root, "Use.php", "@Route(\"/x\")", "Route"),
        vec!["lib/Ann/Route.php: Route"]
    );
}

#[test]
fn annotation_examples_lead_nowhere() {
    let (_dir, root, mut index) = workspace();

    assert_eq!(
        definitions(
            &mut index,
            &root,
            "Use.php",
            "@Route(\"/example\")",
            "Route"
        ),
        Vec::<String>::new()
    );
}
