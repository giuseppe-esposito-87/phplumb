use std::path::{Path, PathBuf};

use phplumb_index::Index;
use phplumb_index::discover::DiscoverOptions;
use phplumb_index::frameworks::symfony::Symfony;

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/symfony")
        .canonicalize()
        .unwrap()
}

fn build() -> Index {
    Index::build(&fixture(), &DiscoverOptions::default())
        .unwrap()
        .0
}

/// The targets of the route `name`, each as its path relative to the fixture and
/// the text at its range.
fn routes(index: &Index, name: &str) -> Vec<(String, String)> {
    let symfony = index.framework::<Symfony>().expect("symfony is active");
    symfony
        .route_targets(name)
        .iter()
        .map(|target| {
            let location = &target.location;
            let text = std::fs::read(&location.path).unwrap();
            let range = &text[location.start as usize..location.end as usize];
            (
                location
                    .path
                    .strip_prefix(fixture())
                    .unwrap()
                    .display()
                    .to_string(),
                String::from_utf8_lossy(range).into_owned(),
            )
        })
        .collect()
}

fn pair(path: &str, text: &str) -> (String, String) {
    (path.to_string(), text.to_string())
}

#[test]
fn named_routes_lead_to_their_method() {
    let index = build();
    assert_eq!(
        routes(&index, "home"),
        [pair("src/Controller/HomeController.php", "index")]
    );
}

#[test]
fn class_names_prefix_method_routes() {
    let index = build();
    assert_eq!(
        routes(&index, "admin_users"),
        [pair("src/Controller/AdminController.php", "users")]
    );
}

#[test]
fn default_names_follow_symfony() {
    let index = build();
    let list = [pair("src/Controller/BlogController.php", "listAction")];
    assert_eq!(routes(&index, "app_blog_list"), list);
    assert_eq!(routes(&index, "app_blog_list_1"), list);
    assert_eq!(
        routes(&index, "app_blog_show"),
        [pair("src/Controller/BlogController.php", "show")]
    );
}

#[test]
fn single_route_methods_answer_to_their_fqcn() {
    let index = build();
    assert_eq!(
        routes(&index, "App\\Controller\\BlogController::show"),
        [pair("src/Controller/BlogController.php", "show")]
    );
    assert!(routes(&index, "App\\Controller\\BlogController::listAction").is_empty());
}

#[test]
fn invokable_classes_route_their_invoke() {
    let index = build();
    let invoke = [pair("src/Controller/AboutController.php", "__invoke")];
    assert_eq!(routes(&index, "about"), invoke);
    assert_eq!(routes(&index, "App\\Controller\\AboutController"), invoke);
    assert_eq!(
        routes(&index, "App\\Controller\\AboutController::__invoke"),
        invoke
    );
}

#[test]
fn yaml_routes_lead_to_their_key_and_controller() {
    let index = build();
    assert_eq!(
        routes(&index, "legacy_page"),
        [
            pair("config/routes.yaml", "legacy_page"),
            pair("src/Controller/LegacyController.php", "show"),
        ]
    );
    assert_eq!(
        routes(&index, "dev_tools"),
        [
            pair("config/routes/dev.yaml", "dev_tools"),
            pair("src/Controller/LegacyController.php", "__invoke"),
        ]
    );
    assert!(routes(&index, "controllers").is_empty());
}

#[test]
fn vendor_attribute_routes_give_nothing() {
    let index = build();
    assert!(routes(&index, "vendor_page").is_empty());
}

/// Go to definition inside the `nth` occurrence of `needle` in `file`, one byte in.
fn definition(index: &mut Index, file: &str, needle: &str) -> Vec<(String, String)> {
    let path = fixture().join(file);
    let source = std::fs::read(&path).unwrap();
    let offset = String::from_utf8_lossy(&source)
        .find(needle)
        .unwrap_or_else(|| panic!("no {needle} in {file}")) as u32
        + 1;
    index
        .definition(&path, &source, offset)
        .into_iter()
        .map(|location| {
            let text = std::fs::read(&location.path).unwrap();
            let range = &text[location.start as usize..location.end as usize];
            (
                location
                    .path
                    .strip_prefix(fixture())
                    .unwrap()
                    .display()
                    .to_string(),
                String::from_utf8_lossy(range).into_owned(),
            )
        })
        .collect()
}

const LINKS: &str = "src/Controller/LinksController.php";

#[test]
fn route_names_in_php_lead_to_their_routes() {
    let mut index = build();
    assert_eq!(
        definition(&mut index, LINKS, "'home'"),
        [pair("src/Controller/HomeController.php", "index")]
    );
    assert_eq!(
        definition(&mut index, LINKS, "'admin_users'"),
        [pair("src/Controller/AdminController.php", "users")]
    );
    assert_eq!(
        definition(&mut index, LINKS, "'app_blog_show'"),
        [pair("src/Controller/BlogController.php", "show")]
    );
}

#[test]
fn unknown_and_dynamic_route_names_give_nothing() {
    let mut index = build();
    assert!(definition(&mut index, LINKS, "'missing'").is_empty());
    assert!(definition(&mut index, LINKS, "$name);").is_empty());
}
