//! Document and workspace symbols over stdio.

mod common;

use common::Client;
use serde_json::json;
use tempfile::TempDir;

/// LSP symbol kinds.
const CLASS: u64 = 5;
const METHOD: u64 = 6;

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

#[test]
fn document_symbols_over_stdio() {
    let (_dir, root_uri, mut client) = start(&[(
        "src/Target.php",
        "<?php\n/* é */ class Target { public function run() {} }\n",
    )]);
    let uri = format!("{root_uri}/src/Target.php");

    let symbols = client.request(
        2,
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": uri}}),
    );

    let class = &symbols[0];
    assert_eq!(class["name"], "Target", "{symbols}");
    assert_eq!(class["kind"], CLASS);
    assert_eq!(
        class["selectionRange"]["start"],
        json!({"line": 1, "character": 14})
    );
    assert_eq!(class["children"][0]["name"], "run");
    assert_eq!(class["children"][0]["kind"], METHOD);
    client.shutdown();
}

#[test]
fn workspace_symbols_over_stdio() {
    let (_dir, _root_uri, mut client) = start(&[
        ("src/Target.php", "<?php\n/* é */ class Target {}\n"),
        (
            "src/Other.php",
            "<?php\nnamespace App;\nclass Targeting {}\n",
        ),
    ]);

    let symbols = client.request(2, "workspace/symbol", json!({"query": "targ"}));

    let first = &symbols[0];
    assert_eq!(first["name"], "Target", "{symbols}");
    assert_eq!(first["kind"], CLASS);
    assert!(
        first["location"]["uri"]
            .as_str()
            .unwrap()
            .ends_with("/src/Target.php")
    );
    assert_eq!(
        first["location"]["range"]["start"],
        json!({"line": 1, "character": 14})
    );
    assert!(
        first.get("containerName").is_none_or(|c| c.is_null()),
        "{first}"
    );
    assert_eq!(symbols[1]["containerName"], "App");
    client.shutdown();
}
