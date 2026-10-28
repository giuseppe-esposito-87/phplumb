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

const TWIG: &str = "templates/base.html.twig";

#[test]
fn route_names_in_twig_lead_to_their_routes() {
    let mut index = build();
    assert_eq!(
        definition(&mut index, TWIG, "'home'"),
        [pair("src/Controller/HomeController.php", "index")]
    );
    assert_eq!(
        definition(&mut index, TWIG, "'admin_users'"),
        [pair("src/Controller/AdminController.php", "users")]
    );
    assert_eq!(
        definition(&mut index, TWIG, "'app_blog_show'"),
        [pair("src/Controller/BlogController.php", "show")]
    );
}

#[test]
fn twig_without_a_literal_route_gives_nothing() {
    let mut index = build();
    assert!(definition(&mut index, TWIG, "name)").is_empty());
    assert!(definition(&mut index, TWIG, "\"home\"").is_empty());
    assert!(definition(&mut index, TWIG, "'home' ~").is_empty());
    assert!(definition(&mut index, TWIG, "'home'|").is_empty());
}

#[test]
fn twig_files_get_no_php_answers() {
    let mut index = build();
    let path = fixture().join(TWIG);
    let source = std::fs::read(&path).unwrap();
    let offset = String::from_utf8_lossy(&source).find("path").unwrap() as u32;
    assert!(index.hover(&path, &source, offset).is_none());
    assert!(index.diagnostics(&path, &source).is_empty());
}

#[test]
fn abstract_controllers_define_no_routes() {
    let index = build();
    assert!(routes(&index, "index").is_empty());
    assert!(routes(&index, "app_crud_list").is_empty());
}

#[test]
fn inherited_route_methods_belong_to_the_subclass() {
    let index = build();
    let crud = "src/Controller/CrudController.php";
    assert_eq!(routes(&index, "post_index"), [pair(crud, "index")]);
    assert_eq!(routes(&index, "post_app_post_list"), [pair(crud, "list")]);
    assert_eq!(
        routes(&index, "App\\Controller\\PostController::index"),
        [pair(crud, "index")]
    );
}

/// The file the template `name` resolves to, relative to the fixture.
fn template(index: &Index, name: &str) -> Option<String> {
    let symfony = index.framework::<Symfony>().expect("symfony is active");
    symfony
        .template_file(name)
        .map(|path| path.strip_prefix(fixture()).unwrap().display().to_string())
}

#[test]
fn namespaced_names_use_their_paths() {
    let index = build();
    assert_eq!(
        template(&index, "@Blog/list.html.twig").as_deref(),
        Some("src/Blog/templates/list.html.twig")
    );
    assert_eq!(
        template(&index, "@Blog/card.html.twig").as_deref(),
        Some("src/Shared/templates/card.html.twig")
    );
    assert_eq!(
        template(&index, "@Dev/tools.html.twig").as_deref(),
        Some("dev/templates/tools.html.twig")
    );
}

#[test]
fn plain_names_use_the_main_namespace_then_the_default_path() {
    let index = build();
    assert_eq!(
        template(&index, "base.html.twig").as_deref(),
        Some("extra/base.html.twig")
    );
    assert_eq!(
        template(&index, "only_extra.html.twig").as_deref(),
        Some("extra/only_extra.html.twig")
    );
}

#[test]
fn unknown_names_and_namespaces_give_nothing() {
    let index = build();
    assert_eq!(template(&index, "@Twig/Exception/error.html.twig"), None);
    assert_eq!(template(&index, "missing.html.twig"), None);
}

const PAGES: &str = "src/Controller/PagesController.php";

#[test]
fn template_names_in_php_open_their_file() {
    let mut index = build();
    for (needle, file) in [
        (
            "'@Blog/list.html.twig'",
            "src/Blog/templates/list.html.twig",
        ),
        ("'base.html.twig'", "extra/base.html.twig"),
        (
            "'@Blog/card.html.twig'",
            "src/Shared/templates/card.html.twig",
        ),
        ("'@Dev/tools.html.twig'", "dev/templates/tools.html.twig"),
    ] {
        assert_eq!(
            definition(&mut index, PAGES, needle),
            [pair(file, "")],
            "{needle}"
        );
    }
}

#[test]
fn unknown_template_names_give_nothing() {
    let mut index = build();
    assert!(definition(&mut index, PAGES, "'missing.html.twig'").is_empty());
    // The method name is not the attribute: it never opens the template.
    let on_name = definition(&mut index, PAGES, "tools()");
    assert!(
        on_name.iter().all(|(path, _)| !path.ends_with(".twig")),
        "{on_name:?}"
    );
}

const PAGE: &str = "templates/page.html.twig";

/// Go to definition one byte inside `needle` on line `line` (from 1) of `file`.
fn definition_on_line(
    index: &mut Index,
    file: &str,
    line: usize,
    needle: &str,
) -> Vec<(String, String)> {
    let path = fixture().join(file);
    let source = std::fs::read(&path).unwrap();
    let text = String::from_utf8_lossy(&source).into_owned();
    let line_start: usize = text
        .split_inclusive('\n')
        .take(line - 1)
        .map(str::len)
        .sum();
    let column = text[line_start..].find(needle).unwrap();
    let offset = (line_start + column + 1) as u32;
    index
        .definition(&path, &source, offset)
        .into_iter()
        .map(|location| {
            let content = std::fs::read(&location.path).unwrap();
            let range = &content[location.start as usize..location.end as usize];
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

#[test]
fn template_names_in_twig_open_their_file() {
    let mut index = build();
    for (line, needle, file) in [
        (1, "'base.html.twig'", "extra/base.html.twig"),
        (
            2,
            "'@Blog/list.html.twig'",
            "src/Blog/templates/list.html.twig",
        ),
        (
            3,
            "\"@Blog/card.html.twig\"",
            "src/Shared/templates/card.html.twig",
        ),
        (4, "'only_extra.html.twig'", "extra/only_extra.html.twig"),
        (5, "'@Dev/tools.html.twig'", "dev/templates/tools.html.twig"),
        (6, "'base.html.twig'", "extra/base.html.twig"),
        (
            7,
            "'@Blog/list.html.twig'",
            "src/Blog/templates/list.html.twig",
        ),
        (8, "'base.html.twig'", "extra/base.html.twig"),
        (9, "'base.html.twig'", "extra/base.html.twig"),
    ] {
        assert_eq!(
            definition_on_line(&mut index, PAGE, line, needle),
            [pair(file, "")],
            "line {line}"
        );
    }
}

#[test]
fn twig_template_variables_give_nothing() {
    let mut index = build();
    assert!(definition_on_line(&mut index, PAGE, 10, "template").is_empty());
    assert!(definition_on_line(&mut index, PAGE, 12, "'base.html.twig'").is_empty());
}

#[test]
fn routes_and_templates_on_one_line() {
    let mut index = build();
    assert_eq!(
        definition_on_line(&mut index, PAGE, 11, "'home'"),
        [pair("src/Controller/HomeController.php", "index")]
    );
    assert_eq!(
        definition_on_line(&mut index, PAGE, 11, "'@Blog/card.html.twig'"),
        [pair("src/Shared/templates/card.html.twig", "")]
    );
}
