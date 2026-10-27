//! File watching: the server follows changes the client reports.

mod common;

use std::path::{Path, PathBuf};

use common::Client;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Workspace {
    _dir: TempDir,
    root: PathBuf,
}

impl Workspace {
    fn new(files: &[(&str, &str)]) -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ws = Workspace { _dir: dir, root };
        for (path, content) in files {
            ws.write(path, content);
        }
        ws
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn uri(&self, relative: &str) -> String {
        format!("file://{}", self.root.join(relative).display())
    }

    fn start(&self, extra: Value) -> Client {
        let mut params = json!({
            "processId": null,
            "capabilities": {},
            "workspaceFolders": [{"uri": self.uri(""), "name": "test"}],
        });
        merge(&mut params, extra);
        let mut client = Client::start();
        client.initialize(params);
        client
    }

    /// Starts the server and waits until its first index is built.
    fn start_indexed(&self, extra: Value) -> Client {
        let mut client = self.start(extra);
        client.wait_for_notification("window/logMessage");
        client
    }
}

fn merge(target: &mut Value, extra: Value) {
    if let (Value::Object(target), Value::Object(extra)) = (target, extra) {
        for (key, value) in extra {
            target.insert(key, value);
        }
    }
}

fn changes(client: &mut Client, events: Vec<(String, u8)>) {
    let changes: Vec<Value> = events
        .into_iter()
        .map(|(uri, typ)| json!({"uri": uri, "type": typ}))
        .collect();
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": changes}),
    );
}

fn definition(client: &mut Client, uri: &str, line: u32, character: u32) -> Vec<Value> {
    client
        .request(
            2,
            "textDocument/definition",
            json!({"textDocument": {"uri": uri}, "position": {"line": line, "character": character}}),
        )
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn ends_with(location: &Value, suffix: &str) -> bool {
    location["uri"].as_str().unwrap().ends_with(suffix)
}

#[test]
fn registers_file_watchers() {
    let ws = Workspace::new(&[("src/A.php", "<?php\nclass A {}\n")]);

    let mut client = ws.start(json!({
        "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}},
    }));
    let request = client.wait_for_request("client/registerCapability");

    let registration = &request["params"]["registrations"][0];
    assert_eq!(registration["method"], "workspace/didChangeWatchedFiles");
    assert_eq!(
        registration["registerOptions"]["watchers"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(
        registration["registerOptions"]["watchers"][0]["globPattern"],
        "**/*.php"
    );
    // The catch-all watcher reports created and deleted directories: Create | Delete.
    assert_eq!(registration["registerOptions"]["watchers"][3]["kind"], 5);
    client.reply_null(&request);
    client.shutdown();
}

#[test]
fn watches_drupal_php_extensions_when_drupal_is_active() {
    let ws = Workspace::new(&[("src/A.php", "<?php\nclass A {}\n")]);

    let mut client = ws.start(json!({
        "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}}},
        "initializationOptions": {"frameworks": {"drupal": "on"}},
    }));
    let request = client.wait_for_request("client/registerCapability");

    assert_eq!(
        request["params"]["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"],
        "**/*.{php,module,inc,install,theme,profile,engine}"
    );
    client.reply_null(&request);
    client.shutdown();
}

#[test]
fn announces_php_extensions() {
    let ws = Workspace::new(&[("src/A.php", "<?php\nclass A {}\n")]);

    let mut client = ws.start(json!({}));
    let notification = client.wait_for_notification("phplumb/phpExtensions");

    assert_eq!(notification["params"], json!({"extensions": ["php"]}));
    client.shutdown();
}

#[test]
fn announces_drupal_php_extensions_when_drupal_is_active() {
    let ws = Workspace::new(&[("src/A.php", "<?php\nclass A {}\n")]);

    let mut client = ws.start(json!({
        "initializationOptions": {"frameworks": {"drupal": "on"}},
    }));
    let notification = client.wait_for_notification("phplumb/phpExtensions");

    assert_eq!(
        notification["params"],
        json!({"extensions": ["php", "module", "inc", "install", "theme", "profile", "engine"]})
    );
    client.shutdown();
}

#[test]
fn applies_a_created_file() {
    let ws = Workspace::new(&[("src/Use.php", "<?php\nnamespace App;\nnew Fresh();\n")]);
    let mut client = ws.start_indexed(json!({}));
    ws.write("src/Fresh.php", "<?php\nnamespace App;\nclass Fresh {}\n");

    changes(&mut client, vec![(ws.uri("src/Fresh.php"), 1)]);
    let found = definition(&mut client, &ws.uri("src/Use.php"), 2, 4);

    assert_eq!(found.len(), 1);
    assert!(ends_with(&found[0], "/src/Fresh.php"));
    client.shutdown();
}

#[test]
fn applies_a_deleted_directory() {
    let ws = Workspace::new(&[
        ("lib/Target.php", "<?php\nnamespace App;\nclass Target {}\n"),
        ("src/Use.php", "<?php\nnamespace App;\nnew Target();\n"),
    ]);
    let mut client = ws.start_indexed(json!({}));
    std::fs::remove_dir_all(ws.root.join("lib")).unwrap();

    changes(&mut client, vec![(ws.uri("lib"), 3)]);

    assert!(definition(&mut client, &ws.uri("src/Use.php"), 2, 4).is_empty());
    client.shutdown();
}

#[test]
fn rebuilds_when_installed_json_changes() {
    let ws = Workspace::new(&[
        ("vendor/composer/installed.json", r#"{"packages": []}"#),
        (
            "vendor/acme/lib/src/A.php",
            "<?php\nnamespace Acme;\nclass A {}\n",
        ),
        ("src/Use.php", "<?php\nnew \\Acme\\A();\n"),
    ]);
    let mut client = ws.start_indexed(json!({}));
    assert!(definition(&mut client, &ws.uri("src/Use.php"), 1, 5).is_empty());
    ws.write(
        "vendor/composer/installed.json",
        r#"{"packages": [{"name": "acme/lib", "install-path": "../acme/lib", "autoload": {"psr-4": {"Acme\\": "src/"}}}]}"#,
    );

    changes(
        &mut client,
        vec![(ws.uri("vendor/composer/installed.json"), 2)],
    );
    let found = definition(&mut client, &ws.uri("src/Use.php"), 1, 5);

    assert_eq!(found.len(), 1);
    assert!(ends_with(&found[0], "/vendor/acme/lib/src/A.php"));
    client.shutdown();
}

#[test]
fn coalesces_bursts_of_changes() {
    let ws = Workspace::new(&[
        ("src/Target.php", "<?php\nnamespace App;\nclass Target {}\n"),
        ("src/Use.php", "<?php\nnamespace App;\nnew Target();\n"),
    ]);
    let mut client = ws.start_indexed(json!({}));

    for _ in 0..50 {
        changes(&mut client, vec![(ws.uri("src/Target.php"), 2)]);
    }
    let (result, notifications) = client.request_collecting(
        2,
        "textDocument/definition",
        json!({"textDocument": {"uri": ws.uri("src/Use.php")}, "position": {"line": 2, "character": 4}}),
    );

    let reindexed = notifications
        .iter()
        .filter(|n| n["method"] == "window/logMessage")
        .filter(|n| {
            n["params"]["message"]
                .as_str()
                .unwrap()
                .starts_with("Reindexed")
        })
        .count();
    assert_eq!(reindexed, 1);
    assert_eq!(result.as_array().unwrap().len(), 1);
    client.shutdown();
}

#[test]
fn writes_the_cache_dir() {
    let ws = Workspace::new(&[("src/A.php", "<?php\nclass A {}\n")]);
    let cache = TempDir::new().unwrap();
    let cache_dir: &Path = cache.path();

    let mut client = ws.start(json!({"initializationOptions": {"cacheDir": cache_dir}}));
    client.shutdown();

    assert!(cache_dir.join("index-v1.bin").exists());
}

#[test]
fn applies_a_renamed_directory() {
    let ws = Workspace::new(&[
        ("lib/Target.php", "<?php\nnamespace App;\nclass Target {}\n"),
        ("src/Use.php", "<?php\nnamespace App;\nnew Target();\n"),
    ]);
    let mut client = ws.start_indexed(json!({}));
    std::fs::rename(ws.root.join("lib"), ws.root.join("lib2")).unwrap();

    changes(&mut client, vec![(ws.uri("lib"), 3), (ws.uri("lib2"), 1)]);
    let found = definition(&mut client, &ws.uri("src/Use.php"), 2, 4);

    assert_eq!(found.len(), 1);
    assert!(ends_with(&found[0], "/lib2/Target.php"));
    client.shutdown();
}

#[test]
fn does_not_log_batches_that_change_nothing() {
    let ws = Workspace::new(&[("src/Use.php", "<?php\nnew A();\n")]);
    let mut client = ws.start_indexed(json!({}));

    changes(&mut client, vec![(ws.uri(".git/index.lock"), 3)]);
    let (_, notifications) = client.request_collecting(
        2,
        "textDocument/definition",
        json!({"textDocument": {"uri": ws.uri("src/Use.php")}, "position": {"line": 1, "character": 4}}),
    );

    assert!(
        notifications
            .iter()
            .all(|n| n["method"] != "window/logMessage")
    );
    client.shutdown();
}
