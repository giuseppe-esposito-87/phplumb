//! Drives the `php-navigator` binary over stdio the way an editor does.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tempfile::TempDir;

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Client {
    fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Client {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut length = 0;
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).unwrap();
            assert!(read > 0, "server closed stdout");
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(value) = line.strip_prefix("Content-Length: ") {
                length = value.parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        self.stdout.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    /// Sends a request and returns its result, skipping notifications and
    /// server-to-client requests in between.
    fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let message = self.receive();
            if message.get("id") == Some(&json!(id)) && message.get("method").is_none() {
                return message["result"].clone();
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }
}

#[test]
fn definition_over_stdio() {
    let dir = TempDir::new().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/Target.php"),
        "<?php\nnamespace App;\nclass Target {}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/Use.php"),
        "<?php\nnamespace App;\n$x = 'è'; new Target();\n",
    )
    .unwrap();
    let root_uri = format!("file://{}", root.display());
    let use_uri = format!("{root_uri}/src/Use.php");

    let mut client = Client::start();
    client.request(
        1,
        "initialize",
        json!({
            "processId": null,
            "capabilities": {},
            "workspaceFolders": [{"uri": root_uri, "name": "test"}],
        }),
    );
    client.notify("initialized", json!({}));

    let result = client.request(
        2,
        "textDocument/definition",
        json!({
            "textDocument": {"uri": use_uri},
            "position": {"line": 2, "character": 14},
        }),
    );

    let locations = result.as_array().expect("definition returns an array");
    assert_eq!(locations.len(), 1);
    assert!(
        locations[0]["uri"]
            .as_str()
            .unwrap()
            .ends_with("/src/Target.php")
    );
    assert_eq!(
        locations[0]["range"]["start"],
        json!({"line": 2, "character": 6})
    );

    client.request(3, "shutdown", Value::Null);
    client.notify("exit", Value::Null);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = client.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "server did not exit");
        std::thread::sleep(Duration::from_millis(20));
    }
}
