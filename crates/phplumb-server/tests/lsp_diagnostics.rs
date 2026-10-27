//! Pull diagnostics over stdio.

mod common;

use std::path::PathBuf;

use common::Client;
use serde_json::{Value, json};
use tempfile::TempDir;

const BROKEN: &str = "<?php\nnamespace App;\nnew Missing();\n";

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

    /// Starts the server with `extra` merged into the initialize params and waits
    /// until its first index is built.
    fn start(&self, extra: Value) -> Client {
        let mut params = json!({
            "processId": null,
            "capabilities": {},
            "workspaceFolders": [{"uri": self.uri(""), "name": "test"}],
        });
        if let (Value::Object(params), Value::Object(extra)) = (&mut params, extra) {
            params.extend(extra);
        }
        let mut client = Client::start();
        client.initialize(params);
        client.wait_for_notification("window/logMessage");
        client
    }
}

fn pull(client: &mut Client, id: i64, uri: &str) -> Vec<Value> {
    let report = client.request(
        id,
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    assert_eq!(report["kind"], "full", "{report}");
    report["items"].as_array().cloned().unwrap_or_default()
}

#[test]
fn diagnostics_over_stdio() {
    let ws = Workspace::new(&[("src/Use.php", BROKEN)]);
    let mut client = ws.start(json!({}));

    let items = pull(&mut client, 2, &ws.uri("src/Use.php"));

    assert_eq!(items.len(), 1, "{items:?}");
    assert_eq!(items[0]["message"], "Undefined class 'App\\Missing'");
    assert_eq!(items[0]["severity"], 1);
    assert_eq!(items[0]["source"], "phplumb");
    assert_eq!(
        items[0]["range"]["start"],
        json!({"line": 2, "character": 4})
    );
    assert_eq!(
        items[0]["range"]["end"],
        json!({"line": 2, "character": 11})
    );
    client.shutdown();
}

#[test]
fn diagnostics_skip_core_and_contrib() {
    let ws = Workspace::new(&[
        ("web/core/lib/Use.php", BROKEN),
        ("web/modules/contrib/tool/src/Use.php", BROKEN),
    ]);
    let mut client = ws.start(json!({}));

    assert_eq!(
        pull(&mut client, 2, &ws.uri("web/core/lib/Use.php")),
        Vec::<Value>::new()
    );
    assert_eq!(
        pull(
            &mut client,
            3,
            &ws.uri("web/modules/contrib/tool/src/Use.php")
        ),
        Vec::<Value>::new()
    );
    client.shutdown();
}

#[test]
fn diagnostics_refresh_after_changes() {
    let ws = Workspace::new(&[("src/Use.php", BROKEN)]);
    let mut client = ws.start(json!({
        "capabilities": {"workspace": {"diagnostics": {"refreshSupport": true}}},
    }));
    let initial = client.wait_for_request("workspace/diagnostic/refresh");
    client.reply_null(&initial);
    assert_eq!(pull(&mut client, 2, &ws.uri("src/Use.php")).len(), 1);

    ws.write(
        "src/Missing.php",
        "<?php\nnamespace App;\nclass Missing {}\n",
    );
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": ws.uri("src/Missing.php"), "type": 1}]}),
    );
    let after_change = client.wait_for_request("workspace/diagnostic/refresh");
    client.reply_null(&after_change);

    assert_eq!(
        pull(&mut client, 3, &ws.uri("src/Use.php")),
        Vec::<Value>::new()
    );
    client.shutdown();
}

#[test]
fn diagnostics_can_be_disabled() {
    let ws = Workspace::new(&[("src/Use.php", BROKEN)]);
    let mut client = ws.start(json!({"initializationOptions": {"diagnostics": false}}));

    assert_eq!(
        pull(&mut client, 2, &ws.uri("src/Use.php")),
        Vec::<Value>::new()
    );
    client.shutdown();
}
