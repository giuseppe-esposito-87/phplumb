//! References and call hierarchy over stdio.

mod common;

use common::Client;
use serde_json::{Value, json};
use tempfile::TempDir;

fn start(files: &[(&str, &str)]) -> (TempDir, String, Client) {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    for (path, content) in files {
        std::fs::write(root.join(path), content).unwrap();
    }
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
fn references_over_stdio() {
    let (_dir, root_uri, mut client) = start(&[
        ("src/Target.php", "<?php\nnamespace App;\nclass Target {}\n"),
        (
            "src/Use.php",
            "<?php\nnamespace App;\nnew Target();\nnew Target();\n",
        ),
    ]);
    let uri = format!("{root_uri}/src/Target.php");
    let mut params = at(&uri, 2, 6);
    params["context"] = json!({"includeDeclaration": false});
    let without = client.request(2, "textDocument/references", params.clone());
    params["context"] = json!({"includeDeclaration": true});
    let with = client.request(3, "textDocument/references", params);

    assert_eq!(without.as_array().unwrap().len(), 2, "{without}");
    assert!(
        without[0]["uri"]
            .as_str()
            .unwrap()
            .ends_with("/src/Use.php")
    );
    assert_eq!(with.as_array().unwrap().len(), 3, "{with}");
    client.shutdown();
}

#[test]
fn call_hierarchy_over_stdio() {
    let (_dir, root_uri, mut client) = start(&[(
        "src/A.php",
        "<?php\nnamespace App;\nclass A\n{\n    public function a() { $this->b(); }\n    public function b() {}\n}\n",
    )]);
    let uri = format!("{root_uri}/src/A.php");

    let b = client.request(2, "textDocument/prepareCallHierarchy", at(&uri, 5, 20));
    let incoming = client.request(3, "callHierarchy/incomingCalls", json!({"item": b[0]}));
    let a = client.request(4, "textDocument/prepareCallHierarchy", at(&uri, 4, 20));
    let outgoing = client.request(5, "callHierarchy/outgoingCalls", json!({"item": a[0]}));

    assert_eq!(b[0]["name"], "b", "{b}");
    assert_eq!(incoming[0]["from"]["name"], "a", "{incoming}");
    assert_eq!(incoming[0]["fromRanges"].as_array().unwrap().len(), 1);
    assert_eq!(outgoing[0]["to"]["name"], "b", "{outgoing}");
    client.shutdown();
}
