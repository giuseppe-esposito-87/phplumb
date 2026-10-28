//! Drupal services over stdio.

mod common;

use std::path::PathBuf;

use common::Client;
use serde_json::{Value, json};
use tempfile::TempDir;

struct Workspace {
    _dir: TempDir,
    root: PathBuf,
}

impl Workspace {
    fn new(services: &str) -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ws = Workspace { _dir: dir, root };
        ws.write(
            "composer.lock",
            r#"{"packages": [{"name": "drupal/core"}]}"#,
        );
        ws.write(
            "core/lib/Drupal.php",
            "<?php\nclass Drupal\n{\n    /** @return mixed */\n    public static function service($id)\n    {\n    }\n}\n",
        );
        ws.write(
            "modules/custom/mymodule/src/Greeter.php",
            "<?php\nnamespace App;\nclass Greeter\n{\n    public function greet() {}\n}\n",
        );
        ws.write("modules/custom/mymodule/mymodule.services.yml", services);
        ws.write(
            "modules/custom/mymodule/src/Use.php",
            "<?php\nnamespace App;\n\\Drupal::service('mymodule.greeter')->greet();\n",
        );
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

    fn start(&self, capabilities: Value) -> Client {
        let mut client = Client::start();
        client.initialize(json!({
            "processId": null,
            "capabilities": capabilities,
            "workspaceFolders": [{"uri": self.uri(""), "name": "test"}],
        }));
        client
    }

    fn definition(&self, client: &mut Client, id: i64, line: u32, character: u32) -> Vec<Value> {
        client
            .request(
                id,
                "textDocument/definition",
                json!({
                    "textDocument": {"uri": self.uri("modules/custom/mymodule/src/Use.php")},
                    "position": {"line": line, "character": character},
                }),
            )
            .as_array()
            .cloned()
            .unwrap_or_default()
    }
}

const GREETER: &str = "services:\n  mymodule.greeter:\n    class: App\\Greeter\n";

#[test]
fn service_definition_over_stdio() {
    let ws = Workspace::new(GREETER);
    let mut client = ws.start(json!({}));
    client.wait_for_notification("window/logMessage");

    // Inside 'mymodule.greeter' on line 2.
    let found = ws.definition(&mut client, 2, 2, 22);

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]["uri"]
            .as_str()
            .unwrap()
            .ends_with("mymodule.services.yml")
    );
    assert_eq!(
        found[0]["range"]["start"],
        json!({"line": 1, "character": 2})
    );
    client.shutdown();
}

#[test]
fn services_follow_yaml_changes() {
    let ws = Workspace::new("services:\n  other.service:\n    class: App\\Greeter\n");
    let mut client = ws.start(json!({}));
    client.wait_for_notification("window/logMessage");
    // On greet() after the call.
    assert_eq!(ws.definition(&mut client, 2, 2, 40), Vec::<Value>::new());

    ws.write("modules/custom/mymodule/mymodule.services.yml", GREETER);
    client.notify(
        "workspace/didChangeWatchedFiles",
        json!({"changes": [{"uri": ws.uri("modules/custom/mymodule/mymodule.services.yml"), "type": 2}]}),
    );
    let found = ws.definition(&mut client, 3, 2, 40);

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0]["uri"].as_str().unwrap().ends_with("Greeter.php"));
    client.shutdown();
}

#[test]
fn registers_a_yaml_watcher() {
    let ws = Workspace::new(GREETER);
    let mut client = ws.start(json!({
        "workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true}},
    }));

    let request = client.wait_for_request("client/registerCapability");

    let watchers = request["params"]["registrations"][0]["registerOptions"]["watchers"]
        .as_array()
        .unwrap()
        .clone();
    assert!(
        watchers
            .iter()
            .any(|w| w["globPattern"] == "**/*.{yml,yaml}"),
        "{watchers:?}"
    );
    client.reply_null(&request);
    client.shutdown();
}

#[test]
fn routing_files_answer_definitions_only() {
    let ws = Workspace::new(GREETER);
    ws.write(
        "modules/custom/mymodule/src/Form/SettingsForm.php",
        "<?php\nnamespace App\\Form;\nclass SettingsForm\n{\n}\n",
    );
    ws.write(
        "modules/custom/mymodule/mymodule.routing.yml",
        "mymodule.settings:\n  path: '/admin'\n  defaults:\n    _form: '\\App\\Form\\SettingsForm'\n",
    );
    let mut client = ws.start(json!({}));
    client.wait_for_notification("window/logMessage");
    let uri = ws.uri("modules/custom/mymodule/mymodule.routing.yml");
    let position = json!({"line": 3, "character": 22});

    let found = client.request(
        10,
        "textDocument/definition",
        json!({"textDocument": {"uri": uri}, "position": position}),
    );
    let hover = client.request(
        11,
        "textDocument/hover",
        json!({"textDocument": {"uri": uri}, "position": position}),
    );
    let diagnostics = client.request(
        12,
        "textDocument/diagnostic",
        json!({"textDocument": {"uri": uri}}),
    );
    let symbols = client.request(
        13,
        "textDocument/documentSymbol",
        json!({"textDocument": {"uri": uri}}),
    );

    let found = found.as_array().cloned().unwrap_or_default();
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0]["uri"]
            .as_str()
            .unwrap()
            .ends_with("SettingsForm.php")
    );
    assert_eq!(hover, Value::Null);
    assert_eq!(diagnostics["items"], json!([]));
    assert!(symbols.is_null() || symbols == json!([]), "{symbols}");
    client.shutdown();
}
