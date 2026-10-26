//! A minimal LSP client that drives the `php-navigator` binary over stdio.

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Client {
    pub child: Child,
    stdin: ChildStdin,
    messages: Receiver<Value>,
}

impl Client {
    pub fn start() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let (sender, messages) = channel();
        std::thread::spawn(move || {
            while let Some(message) = read_message(&mut stdout) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        Client {
            child,
            stdin,
            messages,
        }
    }

    pub fn send(&mut self, message: Value) {
        let body = message.to_string();
        write!(self.stdin, "Content-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
        self.stdin.flush().unwrap();
    }

    /// The next message from the server; panics after ten seconds of silence.
    pub fn receive(&mut self) -> Value {
        match self.messages.recv_timeout(TIMEOUT) {
            Ok(message) => message,
            Err(RecvTimeoutError::Timeout) => panic!("no message from the server for 10 seconds"),
            Err(RecvTimeoutError::Disconnected) => panic!("server closed stdout"),
        }
    }

    /// Sends a request and returns its result. Server-to-client requests that arrive
    /// meanwhile get a null result; notifications are skipped.
    pub fn request(&mut self, id: i64, method: &str, params: Value) -> Value {
        self.send(json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let message = self.receive();
            if message.get("method").is_some() && message.get("id").is_some() {
                self.reply_null(&message);
            } else if message.get("id") == Some(&json!(id)) {
                return message["result"].clone();
            }
        }
    }

    pub fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Reads until a notification with `method` arrives, answering server requests
    /// with null on the way.
    pub fn wait_for_notification(&mut self, method: &str) -> Value {
        let deadline = Instant::now() + TIMEOUT;
        while Instant::now() < deadline {
            let message = self.receive();
            if message.get("method") == Some(&json!(method)) && message.get("id").is_none() {
                return message;
            }
            if message.get("method").is_some() && message.get("id").is_some() {
                self.reply_null(&message);
            }
        }
        panic!("no {method} notification within 10 seconds");
    }

    pub fn reply_null(&mut self, request: &Value) {
        self.send(json!({"jsonrpc": "2.0", "id": request["id"], "result": null}));
    }

    pub fn initialize(&mut self, params: Value) {
        self.request(1, "initialize", params);
        self.notify("initialized", json!({}));
    }

    /// Sends shutdown and exit and waits up to five seconds for a clean exit.
    pub fn shutdown(&mut self) {
        self.request(9_999, "shutdown", Value::Null);
        self.notify("exit", Value::Null);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                return;
            }
            assert!(Instant::now() < deadline, "server did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_message(stdout: &mut impl BufRead) -> Option<Value> {
    let mut length = 0;
    loop {
        let mut line = String::new();
        if stdout.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(value) = line.strip_prefix("Content-Length: ") {
            length = value.parse().ok()?;
        }
    }
    let mut body = vec![0; length];
    stdout.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}
