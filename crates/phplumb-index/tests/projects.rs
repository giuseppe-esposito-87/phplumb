use std::fs;
use std::path::{Path, PathBuf};

use phplumb_index::Index;
use phplumb_index::discover::DiscoverOptions;
use phplumb_index::frameworks::drupal::Drupal;
use phplumb_index::frameworks::{Activation, FrameworkSettings, Project};
use tempfile::TempDir;

const DRUPAL_LOCK: &str = r#"{"packages": [{"name": "drupal/core", "version": "11.4.0"}]}"#;
const SYMFONY_LOCK: &str =
    r#"{"packages": [{"name": "symfony/framework-bundle", "version": "8.1.0"}]}"#;
const DRUPAL_WITH_SYMFONY_PARTS: &str = r#"{"packages": [{"name": "drupal/core", "version": "11.4.0"}, {"name": "symfony/http-kernel", "version": "7.3.0"}]}"#;
const BOTH_LOCK: &str = r#"{"packages": [{"name": "drupal/core", "version": "11.4.0"}], "packages-dev": [{"name": "symfony/framework-bundle", "version": "8.1.0"}]}"#;

/// An empty project with one PHP file, and its canonical root.
fn project() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    fs::write(root.join("index.php"), "<?php\n").unwrap();
    (dir, root)
}

fn lock(root: &Path, dir: &str, content: &str) {
    let dir = root.join(dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("composer.lock"), content).unwrap();
}

fn build(root: &Path, settings: FrameworkSettings) -> Index {
    let opts = DiscoverOptions {
        frameworks: settings,
        ..DiscoverOptions::default()
    };
    Index::build(root, &opts).unwrap().0
}

fn auto(root: &Path) -> Index {
    build(root, FrameworkSettings::default())
}

fn scope(index: &Index, name: &str) -> (PathBuf, Vec<PathBuf>) {
    let project = index
        .framework_project(name)
        .unwrap_or_else(|| panic!("{name} is not active"));
    (project.dir.clone(), project.excluded.clone())
}

#[test]
fn symfony_is_detected_from_framework_bundle() {
    let (_dir, root) = project();
    lock(&root, "", SYMFONY_LOCK);
    assert_eq!(auto(&root).active_frameworks(), ["symfony"]);
}

#[test]
fn drupal_projects_never_activate_symfony() {
    let (_dir, root) = project();
    lock(&root, "", DRUPAL_WITH_SYMFONY_PARTS);
    assert_eq!(auto(&root).active_frameworks(), ["drupal"]);
}

#[test]
fn the_nearest_lock_wins() {
    let (_dir, root) = project();
    lock(&root, "a", DRUPAL_LOCK);
    lock(&root, "b/c", DRUPAL_LOCK);
    assert_eq!(scope(&auto(&root), "drupal"), (root.join("a"), vec![]));
}

#[test]
fn locks_at_the_same_depth_take_the_first_in_path_order() {
    let (_dir, root) = project();
    lock(&root, "b", DRUPAL_LOCK);
    lock(&root, "a", DRUPAL_LOCK);
    assert_eq!(scope(&auto(&root), "drupal"), (root.join("a"), vec![]));
}

#[test]
fn nested_projects_are_left_out_of_the_outer_one() {
    let (_dir, root) = project();
    lock(&root, "", SYMFONY_LOCK);
    lock(&root, "codebase/drupal", DRUPAL_LOCK);
    let index = auto(&root);
    assert_eq!(
        scope(&index, "symfony"),
        (root.clone(), vec![root.join("codebase/drupal")])
    );
    assert_eq!(
        scope(&index, "drupal"),
        (root.join("codebase/drupal"), vec![])
    );
}

#[test]
fn one_lock_with_both_packages_activates_both_on_the_root() {
    let (_dir, root) = project();
    lock(&root, "", BOTH_LOCK);
    let index = auto(&root);
    assert_eq!(index.active_frameworks(), ["drupal", "symfony"]);
    assert_eq!(scope(&index, "drupal"), (root.clone(), vec![]));
    assert_eq!(scope(&index, "symfony"), (root.clone(), vec![]));
}

#[test]
fn a_forced_module_without_a_lock_reads_the_root() {
    let (_dir, root) = project();
    let index = build(
        &root,
        FrameworkSettings {
            drupal: Activation::On,
            ..FrameworkSettings::default()
        },
    );
    assert_eq!(scope(&index, "drupal"), (root.clone(), vec![]));
}

#[test]
fn a_forced_module_uses_a_deeper_lock() {
    let (_dir, root) = project();
    lock(&root, "site", DRUPAL_LOCK);
    let index = build(
        &root,
        FrameworkSettings {
            drupal: Activation::On,
            ..FrameworkSettings::default()
        },
    );
    assert_eq!(scope(&index, "drupal"), (root.join("site"), vec![]));
}

#[test]
fn project_contains_compares_whole_components() {
    let project = Project {
        dir: PathBuf::from("/p/drupal"),
        excluded: vec![PathBuf::from("/p/drupal/sub")],
    };
    assert!(project.contains(Path::new("/p/drupal/x.php")));
    assert!(!project.contains(Path::new("/p/drupal2/x.php")));
    assert!(!project.contains(Path::new("/p/drupal/sub/x.php")));
    assert!(project.contains(Path::new("/p/drupal/subway/x.php")));
}

/// The fixture of a Symfony project holding a Drupal codebase.
fn nested() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/projects")
        .canonicalize()
        .unwrap()
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn drupal(index: &Index) -> &Drupal {
    index.framework::<Drupal>().expect("drupal is active")
}

#[test]
fn drupal_reads_only_its_project() {
    let index = auto(&nested());
    assert!(drupal(&index).defines_route("demo.page"));
    assert!(drupal(&index).defines_route("demo.attr"));
    assert!(!drupal(&index).defines_route("symfony_home"));
}

#[test]
fn updates_outside_the_drupal_project_are_ignored() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    copy_dir(&nested(), &root);
    let mut index = auto(&root);
    let late = root.join("src/Controller/LateController.php");
    fs::write(
        &late,
        "<?php\nnamespace App\\Controller;\nuse Symfony\\Component\\Routing\\Attribute\\Route;\nclass LateController\n{\n    #[Route('/late', name: 'symfony_late')]\n    public function late(): void {}\n}\n",
    )
    .unwrap();
    index.update(&[late], &[]);
    assert!(!drupal(&index).defines_route("symfony_late"));
}
