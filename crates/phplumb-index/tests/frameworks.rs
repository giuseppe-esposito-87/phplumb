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
    definitions_in("modules/custom/mymodule/mymodule.module", needle)
}

/// Like `definitions_after`, in the fixture file `file`.
fn definitions_in(file: &str, needle: &str) -> Vec<String> {
    let (root, mut index) = drupal_fixture();
    let path = root.join(file);
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

fn entity(index: &Index, id: &str) -> Vec<(String, String)> {
    index
        .drupal()
        .unwrap()
        .entity_types(id)
        .iter()
        .map(|e| (e.class.clone(), e.storage.clone()))
        .collect()
}

#[test]
fn entity_types_are_read_from_attributes_and_annotations() {
    let (_root, index) = drupal_fixture();

    assert_eq!(
        entity(&index, "thing"),
        vec![(
            "Drupal\\mymodule\\Entity\\Thing".into(),
            "Drupal\\mymodule\\ThingStorage".into()
        )]
    );
    assert_eq!(
        entity(&index, "gadget"),
        vec![(
            "Drupal\\gadget\\Entity\\Gadget".into(),
            "Drupal\\Core\\Entity\\Sql\\SqlContentEntityStorage".into()
        )]
    );
    assert_eq!(
        entity(&index, "setting"),
        vec![(
            "Drupal\\mymodule\\Entity\\Setting".into(),
            "Drupal\\Core\\Config\\Entity\\ConfigEntityStorage".into()
        )]
    );
}

#[test]
fn entity_types_defined_twice_are_all_kept() {
    let (_root, index) = drupal_fixture();

    let mut dual = entity(&index, "dual");
    dual.sort();
    assert_eq!(
        dual,
        vec![
            (
                "Drupal\\mymodule\\Entity\\DualCustom".into(),
                "Drupal\\Core\\Entity\\Sql\\SqlContentEntityStorage".into()
            ),
            (
                "Drupal\\override\\Entity\\DualOverride".into(),
                "Drupal\\override\\OverrideStorage".into()
            ),
        ],
        "an attribute without an import (Dual) does not name ContentEntityType"
    );
}

#[test]
fn entity_types_in_test_modules_are_ignored() {
    let (_root, index) = drupal_fixture();

    assert!(
        index
            .drupal()
            .unwrap()
            .entity_types("test_thing")
            .is_empty()
    );
}

#[test]
fn entity_type_ids_lead_to_their_declarations() {
    assert_eq!(
        definitions_in(
            "modules/custom/mymodule/src/Entities.php",
            "getStorage('thi"
        ),
        vec!["modules/custom/mymodule/src/Entity/Thing.php: thing"]
    );
    assert_eq!(
        definitions_in(
            "modules/custom/mymodule/src/Entities.php",
            "getStorage('gad"
        ),
        vec!["modules/contrib/gadget/src/Entity/Gadget.php: gadget"]
    );
}

#[test]
fn entity_type_ids_declared_twice_lead_to_both() {
    assert_eq!(
        definitions_in("modules/custom/mymodule/src/Entities.php", "getStorage('du"),
        vec![
            "modules/contrib/override/src/Entity/DualOverride.php: dual",
            "modules/custom/mymodule/src/Entity/DualCustom.php: dual",
        ]
    );
}

const ENTITY_BASE: &str =
    "<?php\nnamespace Drupal\\Core\\Entity;\nabstract class ContentEntityBase {}\n";

#[test]
fn entity_types_follow_updates() {
    let (_dir, root, mut index) = temp_drupal(&[("core/lib/Base.php", ENTITY_BASE)]);
    let thing = "modules/custom/m/src/Entity/Thing.php";

    write(
        &root,
        thing,
        "<?php\nnamespace M;\nuse Drupal\\Core\\Entity\\Attribute\\ContentEntityType;\n#[ContentEntityType(id: 'new_thing')]\nclass Thing extends \\Drupal\\Core\\Entity\\ContentEntityBase {}\n",
    );
    index.update(&[root.join(thing)], &[]);
    assert_eq!(index.drupal().unwrap().entity_types("new_thing").len(), 1);

    fs::remove_file(root.join(thing)).unwrap();
    index.update(&[], &[root.join(thing)]);
    assert!(index.drupal().unwrap().entity_types("new_thing").is_empty());
}

#[test]
fn entity_type_ids_point_at_the_id_key() {
    let (_dir, _root, index) = temp_drupal(&[
        ("core/lib/Base.php", ENTITY_BASE),
        (
            "modules/contrib/w/src/Entity/Widget.php",
            "<?php\nnamespace W;\n/**\n * @ContentEntityType(\n *   base_table = \"widget\",\n *   id = \"widget\",\n * )\n */\nclass Widget extends \\Drupal\\Core\\Entity\\ContentEntityBase {}\n",
        ),
    ]);
    let widget = &index.drupal().unwrap().entity_types("widget")[0];

    let location = widget.id_location().unwrap();

    let text = fs::read_to_string(&location.path).unwrap();
    let before = &text[..location.start as usize];
    assert!(
        before.trim_end_matches('"').trim_end().ends_with("id ="),
        "{before:?}"
    );
}

fn implementations(index: &Index, root: &Path, hook: &str) -> Vec<String> {
    index
        .drupal()
        .unwrap()
        .hook_implementations(hook)
        .iter()
        .map(|i| {
            format!(
                "{} {} {}",
                i.path.strip_prefix(root).unwrap().display(),
                i.name,
                i.extension
            )
        })
        .collect()
}

#[test]
fn hook_implementations_follow_drupal_discovery() {
    let (root, index) = drupal_fixture();

    assert_eq!(
        implementations(&index, &root, "cron"),
        vec![
            "modules/custom/mymodule/mymodule.module mymodule_cron mymodule",
            "modules/custom/mymodule/src/Hook/MymoduleHooks.php cron mymodule",
            "modules/custom/mymodule/src/Hook/MymoduleHooks.php tick mymodule",
            "modules/custom/mymodule/src/Hook/MymoduleHooks.php gadgetCron gadget",
            "modules/custom/mymodule_extra/mymodule_extra.module mymodule_extra_cron mymodule_extra",
        ]
    );
    assert_eq!(
        implementations(&index, &root, "thing_info"),
        vec!["modules/custom/mymodule/src/Hook/ThingInfo.php __invoke mymodule"]
    );
    assert!(implementations(&index, &root, "stop").is_empty());
    assert!(implementations(&index, &root, "page_top").is_empty());
    assert!(implementations(&index, &root, "update_10001").is_empty());
    assert!(implementations(&index, &root, "install").is_empty());
}

#[test]
fn theme_hooks_belong_to_the_theme() {
    use phplumb_index::frameworks::drupal::ExtensionKind;
    let (_root, index) = drupal_fixture();

    let found = index
        .drupal()
        .unwrap()
        .hook_implementations("preprocess_node");

    assert_eq!(found.len(), 1);
    assert_eq!(
        (
            found[0].name.as_str(),
            found[0].extension.as_str(),
            found[0].kind
        ),
        ("mytheme_preprocess_node", "mytheme", ExtensionKind::Theme)
    );
}

#[test]
fn hook_docs_are_read_from_api_files() {
    let (root, index) = drupal_fixture();

    let docs: Vec<String> = index
        .drupal()
        .unwrap()
        .hook_docs("cron")
        .iter()
        .map(|d| {
            format!(
                "{} {}",
                d.path.strip_prefix(&root).unwrap().display(),
                d.name
            )
        })
        .collect();

    assert_eq!(docs, vec!["core/core.api.php cron"]);
}

#[test]
fn hook_implementations_follow_updates() {
    let (_dir, root, mut index) = temp_drupal(&[
        ("modules/custom/m/m.info.yml", "name: M\ntype: module\n"),
        ("modules/custom/m/m.module", "<?php\nfunction m_cron() {}\n"),
    ]);
    assert!(implementations(&index, &root, "mail").is_empty());

    write(
        &root,
        "modules/custom/m/m.module",
        "<?php\nfunction m_cron() {}\nfunction m_mail() {}\n",
    );
    index.update(&[root.join("modules/custom/m/m.module")], &[]);

    assert_eq!(
        implementations(&index, &root, "mail"),
        vec!["modules/custom/m/m.module m_mail m"]
    );
}
