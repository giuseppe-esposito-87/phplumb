use phplumb_index::builtins::{generate, is_class, is_constant, is_function};

#[test]
fn builtins_know_core_and_extension_names() {
    assert!(is_class("DateTime"));
    assert!(is_class("datetimeinterface"));
    assert!(is_class("MongoDB\\Driver\\Manager"));
    assert!(is_function("strlen"));
    assert!(is_function("STR_CONTAINS"));
    assert!(is_constant("PHP_EOL"));
    assert!(is_constant("E_ALL"));
}

#[test]
fn builtins_reject_unknown_names() {
    assert!(!is_class("App\\Foo"));
    assert!(!is_function("my_helper"));
    assert!(!is_constant("php_eol"));
}

#[test]
fn generate_lists_stub_declarations() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("ext")).unwrap();
    std::fs::write(
        dir.path().join("ext/ns.php"),
        "<?php\nnamespace NS;\nclass A {}\nfunction f() {}\nconst K = 1;\ndefine('D', 1);\n",
    )
    .unwrap();

    let list = generate(dir.path()).unwrap();

    let lines: Vec<&str> = list.lines().collect();
    assert_eq!(lines, vec!["c ns\\a", "f ns\\f", "k D", "k ns\\K"]);
}

#[test]
fn builtins_know_method_names() {
    assert!(phplumb_index::builtins::is_method("format"));
    assert!(phplumb_index::builtins::is_method("GETMESSAGE"));
    assert!(!phplumb_index::builtins::is_method("fabricated"));
}
