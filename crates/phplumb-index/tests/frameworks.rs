use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::Index;
use phplumb_index::discover::DiscoverOptions;
use phplumb_index::frameworks::{Activation, FrameworkSettings};
use tempfile::TempDir;

fn project(composer_lock: Option<&str>) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join("index.php"), "<?php\n").unwrap();
    if let Some(lock) = composer_lock {
        fs::write(root.join("composer.lock"), lock).unwrap();
    }
    (dir, root)
}

fn active(root: &Path, drupal: Activation) -> Vec<&'static str> {
    let opts = DiscoverOptions {
        frameworks: FrameworkSettings { drupal },
        ..DiscoverOptions::default()
    };
    Index::build(root, &opts).unwrap().0.active_frameworks()
}

const DRUPAL_LOCK: &str = r#"{"packages": [{"name": "drupal/core", "version": "11.4.0"}]}"#;
const OTHER_LOCK: &str = r#"{"packages": [{"name": "symfony/console", "version": "7.0.0"}]}"#;

#[test]
fn drupal_is_detected_from_composer_lock() {
    let (_dir, root) = project(Some(DRUPAL_LOCK));

    assert_eq!(active(&root, Activation::Auto), vec!["drupal"]);
}

#[test]
fn drupal_stays_off_without_drupal_core() {
    let (_dir, other) = project(Some(OTHER_LOCK));
    let (_dir2, none) = project(None);

    assert_eq!(active(&other, Activation::Auto), Vec::<&str>::new());
    assert_eq!(active(&none, Activation::Auto), Vec::<&str>::new());
}

#[test]
fn drupal_can_be_forced() {
    let (_dir, none) = project(None);
    let (_dir2, drupal) = project(Some(DRUPAL_LOCK));

    assert_eq!(active(&none, Activation::On), vec!["drupal"]);
    assert_eq!(active(&drupal, Activation::Off), Vec::<&str>::new());
}

fn drupal_fixture() -> (PathBuf, Index) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/drupal")
        .canonicalize()
        .unwrap();
    let (index, _) = Index::build(&root, &DiscoverOptions::default()).unwrap();
    (root, index)
}

fn classes(index: &Index, id: &str) -> Vec<String> {
    index.drupal().unwrap().service_classes(index, id)
}

#[test]
fn services_are_read_with_their_key_positions() {
    let (root, index) = drupal_fixture();

    let definitions = index.drupal().unwrap().services("mymodule.greeter");

    assert_eq!(definitions.len(), 1);
    let definition = &definitions[0];
    assert_eq!(
        definition.path,
        root.join("modules/custom/mymodule/mymodule.services.yml")
    );
    let text = fs::read_to_string(&definition.path).unwrap();
    assert_eq!(
        &text[definition.start as usize..definition.end as usize],
        "mymodule.greeter"
    );
    assert_eq!(
        definition.class.as_deref(),
        Some("Drupal\\mymodule\\Greeter")
    );
}

#[test]
fn service_aliases_resolve() {
    let (_root, index) = drupal_fixture();

    assert_eq!(
        classes(&index, "entity_type.manager.alias"),
        vec!["Drupal\\Core\\Entity\\EntityTypeManager"]
    );
    assert_eq!(
        classes(&index, "logger.factory"),
        vec!["Drupal\\Core\\Logger\\LoggerChannelFactory"]
    );
    assert_eq!(classes(&index, "cycle.a"), Vec::<String>::new());
}

#[test]
fn service_parents_and_class_names_resolve() {
    let (_root, index) = drupal_fixture();

    assert_eq!(classes(&index, "child.service"), vec!["Drupal\\Core\\Base"]);
    assert_eq!(
        classes(&index, "Drupal\\Core\\Named"),
        vec!["Drupal\\Core\\Named"]
    );
    assert_eq!(
        classes(&index, "Drupal\\Core\\Autowired"),
        vec!["Drupal\\Core\\Autowired"]
    );
}

#[test]
fn decorated_services_resolve_to_both_classes() {
    let (_root, index) = drupal_fixture();

    assert_eq!(
        classes(&index, "mymodule.greeter"),
        vec![
            "Drupal\\mymodule\\Greeter",
            "Drupal\\mymodule\\GreeterDecorator"
        ]
    );
}

#[test]
fn services_defined_twice_give_both_classes() {
    let (_root, index) = drupal_fixture();

    assert_eq!(index.drupal().unwrap().services("shared.id").len(), 2);
    assert_eq!(
        classes(&index, "shared.id"),
        vec!["Drupal\\Core\\SharedCore", "Drupal\\mymodule\\SharedCustom"]
    );
}

#[test]
fn services_in_tests_and_vendor_are_ignored() {
    let (_root, index) = drupal_fixture();
    let drupal = index.drupal().unwrap();

    for id in ["test.only", "vendor.only", "_defaults", "some.parameter"] {
        assert!(drupal.services(id).is_empty(), "{id}");
    }
}

/// Definitions of the cursor right after `needle` in the fixture's module file,
/// as `relative path: text at the location`.
fn definitions_after(needle: &str) -> Vec<String> {
    let (root, mut index) = drupal_fixture();
    let path = root.join("modules/custom/mymodule/mymodule.module");
    let source = fs::read(&path).unwrap();
    let text = String::from_utf8(source.clone()).unwrap();
    let offset = text.find(needle).unwrap() + needle.len();
    let mut found: Vec<String> = index
        .definition(&path, &source, offset as u32)
        .into_iter()
        .map(|l| {
            let content = fs::read_to_string(&l.path).unwrap();
            let name = l.path.strip_prefix(&root).unwrap().display().to_string();
            format!("{name}: {}", &content[l.start as usize..l.end as usize])
        })
        .collect();
    found.sort();
    found
}

#[test]
fn service_ids_lead_to_their_yaml_keys() {
    assert_eq!(
        definitions_after("\\Drupal::service('mymodule.gre"),
        vec!["modules/custom/mymodule/mymodule.services.yml: mymodule.greeter"]
    );
    assert_eq!(
        definitions_after("$container->get('entity_type.man"),
        vec!["core/core.services.yml: entity_type.manager"]
    );
}

#[test]
fn service_ids_defined_twice_lead_to_both_keys() {
    assert_eq!(
        definitions_after("\\Drupal::service('shared."),
        vec![
            "core/core.services.yml: shared.id",
            "modules/custom/mymodule/mymodule.services.yml: shared.id",
        ]
    );
}

#[test]
fn other_strings_lead_nowhere() {
    assert_eq!(definitions_after("getStorage('no"), Vec::<String>::new());
}

/// A Drupal project in a temporary directory, built with the Drupal module on.
fn temp_drupal(files: &[(&str, &str)]) -> (TempDir, PathBuf, Index) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for (path, content) in files {
        write(&root, path, content);
    }
    let opts = DiscoverOptions {
        frameworks: FrameworkSettings {
            drupal: Activation::On,
        },
        ..DiscoverOptions::default()
    };
    let index = Index::build(&root, &opts).unwrap().0;
    (dir, root, index)
}

fn write(root: &Path, path: &str, content: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

const FOO: &str = "<?php\nnamespace App;\nclass Foo { public function foo() {} }\nclass Bar { public function bar() {} }\n";

#[test]
fn services_in_hidden_directories_stay_out_after_updates() {
    let (_dir, root, mut index) = temp_drupal(&[
        ("src/Foo.php", FOO),
        (
            "modules/custom/m/m.services.yml",
            "services:\n  m.svc:\n    class: App\\Foo\n",
        ),
        (
            ".worktrees/feature/modules/custom/m/m.services.yml",
            "services:\n  m.svc:\n    class: App\\Bar\n",
        ),
    ]);
    assert_eq!(index.drupal().unwrap().services("m.svc").len(), 1);

    index.update(
        &[
            root.join(".worktrees/feature/modules/custom/m/m.services.yml"),
            root.join(".worktrees/feature"),
        ],
        &[],
    );

    assert_eq!(classes(&index, "m.svc"), vec!["App\\Foo"]);
}

#[test]
fn service_keys_after_non_ascii_text_point_at_the_id() {
    let (_dir, _root, index) = temp_drupal(&[
        ("src/Foo.php", FOO),
        (
            "modules/custom/m/m.services.yml",
            "# Servizi per l'università — caffè\nservices:\n  a.one:\n    class: App\\Foo\n    arguments: ['caffè']\n  h.à:\n    class: App\\Foo\n  b.two:\n    class: App\\Bar\n",
        ),
    ]);
    let drupal = index.drupal().unwrap();

    for id in ["a.one", "h.à", "b.two"] {
        let definition = &drupal.services(id)[0];
        let text = fs::read_to_string(&definition.path).unwrap();
        assert_eq!(
            &text[definition.start as usize..definition.end as usize],
            id
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinked_modules_are_read_once_and_follow_edits() {
    let (_dir, root, _) = temp_drupal(&[
        ("src/Foo.php", FOO),
        (
            "packages/foo/foo.services.yml",
            "services:\n  foo.svc:\n    class: App\\Foo\n",
        ),
    ]);
    fs::create_dir_all(root.join("modules/custom")).unwrap();
    std::os::unix::fs::symlink(root.join("packages/foo"), root.join("modules/custom/foo")).unwrap();
    let opts = DiscoverOptions {
        frameworks: FrameworkSettings {
            drupal: Activation::On,
        },
        ..DiscoverOptions::default()
    };
    let mut index = Index::build(&root, &opts).unwrap().0;
    assert_eq!(index.drupal().unwrap().services("foo.svc").len(), 1);

    write(
        &root,
        "packages/foo/foo.services.yml",
        "services:\n  foo.svc:\n    class: App\\Bar\n",
    );
    index.update(&[root.join("packages/foo/foo.services.yml")], &[]);

    assert_eq!(classes(&index, "foo.svc"), vec!["App\\Bar"]);
}

#[test]
fn services_files_appear_and_disappear() {
    let (_dir, root, mut index) = temp_drupal(&[("src/Foo.php", FOO)]);

    write(
        &root,
        "modules/custom/n/n.services.yml",
        "services:\n  n.svc:\n    class: App\\Foo\n",
    );
    index.update(&[root.join("modules/custom/n/n.services.yml")], &[]);
    assert_eq!(classes(&index, "n.svc"), vec!["App\\Foo"]);

    fs::remove_dir_all(root.join("modules/custom/n")).unwrap();
    index.update(&[], &[root.join("modules/custom/n")]);
    assert_eq!(classes(&index, "n.svc"), Vec::<String>::new());

    write(
        &root,
        "modules/custom/o/o.services.yml",
        "services:\n  o.svc:\n    class: App\\Bar\n",
    );
    index.update(&[root.join("modules/custom/o")], &[]);
    assert_eq!(classes(&index, "o.svc"), vec!["App\\Bar"]);
}
