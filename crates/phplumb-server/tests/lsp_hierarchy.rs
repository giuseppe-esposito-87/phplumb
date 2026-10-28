//! Implementations and type hierarchy over stdio.

mod common;

use common::Client;
use serde_json::{Value, json};
use tempfile::TempDir;

fn start() -> (TempDir, String, Client) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/Shape.php"),
        "<?php\nnamespace App;\ninterface Shape {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/Square.php"),
        "<?php\nnamespace App;\nclass Square implements Shape {}\n",
    )
    .unwrap();
    let root_uri = format!("file://{}", root.display());
    let mut client = Client::start();
    client.initialize(json!({
        "processId": null,
        "capabilities": {},
        "workspaceFolders": [{"uri": root_uri, "name": "test"}],
    }));
    (dir, root_uri, client)
}

fn at(uri: &str, line: u32, character: u32) -> Value {
    json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}})
}

#[test]
fn implementation_over_stdio() {
    let (_dir, root_uri, mut client) = start();

    let result = client.request(
        2,
        "textDocument/implementation",
        at(&format!("{root_uri}/src/Shape.php"), 2, 10),
    );

    let locations = result.as_array().expect("an array of locations");
    assert_eq!(locations.len(), 1, "{result}");
    assert!(
        locations[0]["uri"]
            .as_str()
            .unwrap()
            .ends_with("/src/Square.php")
    );
    assert_eq!(
        locations[0]["range"]["start"],
        json!({"line": 2, "character": 6})
    );
    client.shutdown();
}

#[test]
fn type_hierarchy_over_stdio() {
    let (_dir, root_uri, mut client) = start();

    let square = client.request(
        2,
        "textDocument/prepareTypeHierarchy",
        at(&format!("{root_uri}/src/Square.php"), 2, 6),
    );
    let item = square[0].clone();
    let supertypes = client.request(3, "typeHierarchy/supertypes", json!({"item": item}));
    let shape = client.request(
        4,
        "textDocument/prepareTypeHierarchy",
        at(&format!("{root_uri}/src/Shape.php"), 2, 10),
    );
    let subtypes = client.request(5, "typeHierarchy/subtypes", json!({"item": shape[0]}));

    assert_eq!(item["name"], "Square");
    assert_eq!(item["detail"], "App\\Square");
    assert_eq!(supertypes[0]["name"], "Shape", "{supertypes}");
    assert_eq!(subtypes[0]["name"], "Square", "{subtypes}");
    client.shutdown();
}

/// A project class extending a built-in one, whose method calls a built-in method.
fn start_with_built_in_parent() -> (TempDir, String, Client) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/AppException.php"),
        "<?php\nnamespace App;\nclass AppException extends \\RuntimeException\n{\n    public function describe(): string { return $this->getMessage(); }\n}\n",
    )
    .unwrap();
    let root_uri = format!("file://{}", root.display());
    let mut client = Client::start();
    client.initialize(json!({
        "processId": null,
        "capabilities": {},
        "workspaceFolders": [{"uri": root_uri, "name": "test"}],
    }));
    (dir, root_uri, client)
}

/// Every `uri` in `items`, at `field` when given, is in the project.
fn all_in_project(items: &Value, field: Option<&str>, root_uri: &str) -> bool {
    items.as_array().unwrap().iter().all(|item| {
        let item = field.map_or(item, |field| &item[field]);
        item["uri"].as_str().unwrap().starts_with(root_uri)
    })
}

#[test]
fn type_hierarchy_leaves_stubs_out() {
    let (_dir, root_uri, mut client) = start_with_built_in_parent();
    let uri = format!("{root_uri}/src/AppException.php");

    let item = client.request(2, "textDocument/prepareTypeHierarchy", at(&uri, 2, 6));
    let supertypes = client.request(3, "typeHierarchy/supertypes", json!({"item": item[0]}));

    assert_eq!(item[0]["name"], "AppException", "{item}");
    assert!(all_in_project(&supertypes, None, &root_uri), "{supertypes}");
    client.shutdown();
}

#[test]
fn outgoing_calls_leave_stubs_out() {
    let (_dir, root_uri, mut client) = start_with_built_in_parent();
    let uri = format!("{root_uri}/src/AppException.php");

    let item = client.request(2, "textDocument/prepareCallHierarchy", at(&uri, 4, 20));
    let outgoing = client.request(3, "callHierarchy/outgoingCalls", json!({"item": item[0]}));

    assert_eq!(item[0]["name"], "describe", "{item}");
    assert!(
        all_in_project(&outgoing, Some("to"), &root_uri),
        "{outgoing}"
    );
    client.shutdown();
}
