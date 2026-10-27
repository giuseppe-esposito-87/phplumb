//! The `root` setting: a project in a folder below the workspace folder.

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
    /// The project in `src/app` and a copy of its class in `tools`.
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let ws = Workspace { _dir: dir, root };
        ws.write("src/app/Greeter.php", "<?php\nclass Greeter {}\n");
        ws.write("src/app/Use.php", "<?php\nnew Greeter();\n");
        ws.write("tools/Greeter.php", "<?php\nclass Greeter {}\n");
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

    fn start(&self, options: Value) -> Client {
        let mut client = Client::start();
        client.initialize(json!({
            "processId": null,
            "capabilities": {},
            "workspaceFolders": [{"uri": self.uri(""), "name": "test"}],
            "initializationOptions": options,
        }));
        client
    }

    /// The files `new Greeter()` in `src/app/Use.php` leads to.
    fn greeter_targets(&self, client: &mut Client) -> Vec<String> {
        let result = client.request(
            2,
            "textDocument/definition",
            json!({
                "textDocument": {"uri": self.uri("src/app/Use.php")},
                "position": {"line": 1, "character": 6},
            }),
        );
        let mut targets: Vec<String> = result
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|location| location["uri"].as_str().unwrap().to_string())
            .collect();
        targets.sort();
        targets
    }
}

#[test]
fn the_whole_workspace_is_indexed_by_default() {
    let ws = Workspace::new();
    let mut client = ws.start(json!({}));

    assert_eq!(
        ws.greeter_targets(&mut client),
        vec![ws.uri("src/app/Greeter.php"), ws.uri("tools/Greeter.php")]
    );
    client.shutdown();
}

#[test]
fn root_limits_the_index_to_a_folder_below_the_workspace() {
    let ws = Workspace::new();
    let mut client = ws.start(json!({"root": "src/app"}));

    assert_eq!(
        ws.greeter_targets(&mut client),
        vec![ws.uri("src/app/Greeter.php")]
    );
    client.shutdown();
}

#[test]
fn exclude_globs_are_relative_to_root() {
    let ws = Workspace::new();
    ws.write("src/app/lib/Greeter.php", "<?php\nclass Greeter {}\n");
    let mut client = ws.start(json!({"root": "src/app", "exclude": ["lib/**"]}));

    assert_eq!(
        ws.greeter_targets(&mut client),
        vec![ws.uri("src/app/Greeter.php")]
    );
    client.shutdown();
}

#[test]
fn a_root_that_is_no_folder_falls_back_to_the_workspace_with_a_warning() {
    let ws = Workspace::new();
    let mut client = ws.start(json!({"root": "missing"}));

    let warning = client.wait_for_notification("window/showMessage");
    assert_eq!(warning["params"]["type"], json!(2));
    assert!(
        warning["params"]["message"]
            .as_str()
            .unwrap()
            .contains("missing"),
        "{warning}"
    );
    assert_eq!(
        ws.greeter_targets(&mut client),
        vec![ws.uri("src/app/Greeter.php"), ws.uri("tools/Greeter.php")]
    );
    client.shutdown();
}
