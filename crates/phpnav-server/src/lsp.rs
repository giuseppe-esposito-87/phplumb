//! The LSP server: initialization, indexing, file watching and request handling.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::time::Duration;

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{DidChangeWatchedFiles, LogMessage, Notification as _, Progress};
use lsp_types::request::{
    GotoDefinition, RegisterCapability, Request as _, Shutdown, WorkDoneProgressCreate,
};
use lsp_types::{
    DidChangeWatchedFilesParams, DidChangeWatchedFilesRegistrationOptions, FileChangeType,
    FileEvent, FileSystemWatcher, GlobPattern, GotoDefinitionParams, InitializeParams, Location,
    LogMessageParams, MessageType, NumberOrString, OneOf, Position, ProgressParams,
    ProgressParamsValue, Range, Registration, RegistrationParams, ServerCapabilities, WatchKind,
    WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressCreateParams, WorkDoneProgressEnd,
};
use phpnav_index::discover::{DiscoverOptions, PHP_EXTENSIONS};
use phpnav_index::line_index::LineIndex;
use phpnav_index::{Index, IndexStats};
use serde::Deserialize;
use serde_json::json;

use crate::convert::{path_to_uri, uri_to_path};

const PROGRESS_TOKEN: &str = "php-navigator/indexing";
const WATCH_REGISTRATION: &str = "php-navigator/watch";
const CACHE_FILE: &str = "index-v1.bin";
/// How long to wait for more file events before applying a batch. Branch switches
/// and agents report changes in many small notifications.
const DEBOUNCE: Duration = Duration::from_millis(50);

/// Parsed from `initializationOptions`:
/// `{ "exclude": ["vendor/rector/rector/**"], "cacheDir": "/path/to/storage" }`.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ServerSettings {
    pub exclude: Vec<String>,
    pub cache_dir: Option<PathBuf>,
}

/// Serves LSP over stdin and stdout until the client sends `exit`.
pub fn serve() -> anyhow::Result<()> {
    let (connection, io_threads) = Connection::stdio();
    run(connection)?;
    io_threads.join()?;
    Ok(())
}

pub fn run(connection: Connection) -> anyhow::Result<()> {
    let (id, params) = connection.initialize_start()?;
    let params: InitializeParams = serde_json::from_value(params)?;
    let capabilities = ServerCapabilities {
        definition_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    };
    connection.initialize_finish(
        id,
        json!({
            "capabilities": capabilities,
            "serverInfo": {"name": "php-navigator", "version": env!("CARGO_PKG_VERSION")},
        }),
    )?;

    let settings: ServerSettings = params
        .initialization_options
        .clone()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default();
    let reports_progress = params
        .capabilities
        .window
        .as_ref()
        .and_then(|window| window.work_done_progress)
        .unwrap_or(false);
    let watches_files = params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|workspace| workspace.did_change_watched_files)
        .and_then(|watched| watched.dynamic_registration)
        .unwrap_or(false);

    let mut server = Server {
        connection: &connection,
        root: workspace_root(&params),
        opts: DiscoverOptions {
            exclude: settings.exclude,
            ..DiscoverOptions::default()
        },
        cache: settings.cache_dir.map(|dir| dir.join(CACHE_FILE)),
        index: None,
        pending: VecDeque::new(),
    };
    if watches_files {
        server.register_watchers()?;
    }
    // Requests that arrive while indexing wait in the connection's queue.
    server.rebuild(reports_progress)?;
    server.save_cache()?;
    server.main_loop()
}

/// The first workspace folder, or the deprecated `rootUri` when there are none.
fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folder) = params.workspace_folders.as_ref().and_then(|f| f.first()) {
        return uri_to_path(&folder.uri);
    }
    #[allow(deprecated)]
    params.root_uri.as_ref().and_then(uri_to_path)
}

struct Server<'a> {
    connection: &'a Connection,
    root: Option<PathBuf>,
    opts: DiscoverOptions,
    cache: Option<PathBuf>,
    index: Option<Index>,
    /// Messages read while collecting file events, handled before the queue.
    pending: VecDeque<Message>,
}

impl Server<'_> {
    fn main_loop(&mut self) -> anyhow::Result<()> {
        loop {
            let message = match self.pending.pop_front() {
                Some(message) => message,
                None => match self.connection.receiver.recv() {
                    Ok(message) => message,
                    Err(_) => return Ok(()),
                },
            };
            match message {
                Message::Request(request) => {
                    if request.method == Shutdown::METHOD {
                        self.save_cache()?;
                    }
                    if self.connection.handle_shutdown(&request)? {
                        return Ok(());
                    }
                    self.handle_request(request)?;
                }
                Message::Notification(notification)
                    if notification.method == DidChangeWatchedFiles::METHOD =>
                {
                    let mut events = file_events(notification);
                    self.collect_more_events(&mut events);
                    self.apply_events(events)?;
                }
                _ => {}
            }
        }
    }

    fn handle_request(&mut self, request: Request) -> anyhow::Result<()> {
        let response = match request.method.as_str() {
            GotoDefinition::METHOD => {
                match serde_json::from_value::<GotoDefinitionParams>(request.params) {
                    Ok(params) => {
                        Response::new_ok(request.id, definition(self.index.as_mut(), params))
                    }
                    Err(err) => Response::new_err(
                        request.id,
                        ErrorCode::InvalidParams as i32,
                        err.to_string(),
                    ),
                }
            }
            _ => Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                format!("unhandled method {}", request.method),
            ),
        };
        self.connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    /// Adds every file event that arrives within the debounce window. The first other
    /// message ends the window and is handled after the batch is applied.
    fn collect_more_events(&mut self, events: &mut Vec<FileEvent>) {
        while let Ok(message) = self.connection.receiver.recv_timeout(DEBOUNCE) {
            match message {
                Message::Notification(notification)
                    if notification.method == DidChangeWatchedFiles::METHOD =>
                {
                    events.extend(file_events(notification));
                }
                other => {
                    self.pending.push_back(other);
                    return;
                }
            }
        }
    }

    /// Applies one batch of file events. A change to Composer's `installed.json`
    /// changes which vendor files belong to the index, so it rebuilds everything.
    fn apply_events(&mut self, events: Vec<FileEvent>) -> anyhow::Result<()> {
        let mut changed = BTreeSet::new();
        let mut removed = BTreeSet::new();
        let mut rebuild = false;
        for event in events {
            let Some(path) = uri_to_path(&event.uri) else {
                continue;
            };
            if path.ends_with("vendor/composer/installed.json") {
                rebuild = true;
            } else if event.typ == FileChangeType::DELETED {
                removed.insert(path);
            } else {
                changed.insert(path);
            }
        }
        if rebuild {
            return self.rebuild(false);
        }
        let Some(index) = self.index.as_mut() else {
            return Ok(());
        };
        let changed: Vec<PathBuf> = changed.into_iter().collect();
        let removed: Vec<PathBuf> = removed.into_iter().collect();
        let stats = index.update(&changed, &removed);
        self.log_info(format!(
            "Reindexed {} files, removed {} in {} ms",
            stats.reparsed, stats.removed, stats.elapsed_ms
        ))
    }

    fn register_watchers(&self) -> anyhow::Result<()> {
        let watcher = |pattern: String, kind: Option<WatchKind>| FileSystemWatcher {
            glob_pattern: GlobPattern::String(pattern),
            kind,
        };
        let options = DidChangeWatchedFilesRegistrationOptions {
            watchers: vec![
                watcher(format!("**/*.{{{}}}", PHP_EXTENSIONS.join(",")), None),
                watcher("**/vendor/composer/installed.json".to_string(), None),
                // Deleting a directory reports only the directory itself.
                watcher("**/*".to_string(), Some(WatchKind::Delete)),
            ],
        };
        let registration = Registration {
            id: WATCH_REGISTRATION.to_string(),
            method: DidChangeWatchedFiles::METHOD.to_string(),
            register_options: Some(serde_json::to_value(options)?),
        };
        self.connection.sender.send(Message::Request(Request::new(
            RequestId::from(WATCH_REGISTRATION.to_string()),
            RegisterCapability::METHOD.to_string(),
            RegistrationParams {
                registrations: vec![registration],
            },
        )))?;
        Ok(())
    }

    /// Builds the index from scratch, from the cache when there is one.
    fn rebuild(&mut self, reports_progress: bool) -> anyhow::Result<()> {
        let Some(root) = self.root.clone() else {
            return Ok(());
        };
        let token = NumberOrString::String(PROGRESS_TOKEN.to_string());
        if reports_progress {
            self.connection.sender.send(Message::Request(Request::new(
                RequestId::from(PROGRESS_TOKEN.to_string()),
                WorkDoneProgressCreate::METHOD.to_string(),
                WorkDoneProgressCreateParams {
                    token: token.clone(),
                },
            )))?;
            self.send_progress(
                &token,
                WorkDoneProgress::Begin(WorkDoneProgressBegin {
                    title: "PHP Navigator".to_string(),
                    message: Some("Indexing".to_string()),
                    ..WorkDoneProgressBegin::default()
                }),
            )?;
        }

        let (index, stats) = build_index(&root, &self.opts, self.cache.as_deref())?;
        self.index = Some(index);
        let summary = summary_message(&stats);
        self.log_info(format!(
            "{summary} ({} declarations, {} files from cache)",
            stats.declarations, stats.reused
        ))?;

        if reports_progress {
            self.send_progress(
                &token,
                WorkDoneProgress::End(WorkDoneProgressEnd {
                    message: Some(summary),
                }),
            )?;
        }
        Ok(())
    }

    /// Writes the cache when there is one. A failure is logged, not fatal: the next
    /// start parses the changed files again.
    fn save_cache(&self) -> anyhow::Result<()> {
        let (Some(cache), Some(index)) = (&self.cache, &self.index) else {
            return Ok(());
        };
        match index.save_cache(cache) {
            Ok(()) => Ok(()),
            Err(err) => self.log(
                MessageType::WARNING,
                format!("Could not write the index cache: {err:#}"),
            ),
        }
    }

    fn log_info(&self, message: String) -> anyhow::Result<()> {
        self.log(MessageType::INFO, message)
    }

    /// Sends a message to the client's log.
    fn log(&self, typ: MessageType, message: String) -> anyhow::Result<()> {
        self.connection
            .sender
            .send(Message::Notification(Notification::new(
                LogMessage::METHOD.to_string(),
                LogMessageParams { typ, message },
            )))?;
        Ok(())
    }

    fn send_progress(
        &self,
        token: &NumberOrString,
        progress: WorkDoneProgress,
    ) -> anyhow::Result<()> {
        self.connection
            .sender
            .send(Message::Notification(Notification::new(
                Progress::METHOD.to_string(),
                ProgressParams {
                    token: token.clone(),
                    value: ProgressParamsValue::WorkDone(progress),
                },
            )))?;
        Ok(())
    }
}

fn build_index(
    root: &Path,
    opts: &DiscoverOptions,
    cache: Option<&Path>,
) -> anyhow::Result<(Index, IndexStats)> {
    match cache {
        Some(cache) => Index::build_with_cache(root, opts, cache),
        None => Index::build(root, opts),
    }
}

fn file_events(notification: Notification) -> Vec<FileEvent> {
    serde_json::from_value::<DidChangeWatchedFilesParams>(notification.params)
        .map(|params| params.changes)
        .unwrap_or_default()
}

fn summary_message(stats: &IndexStats) -> String {
    format!("Indexed {} files in {} ms", stats.files, stats.elapsed_ms)
}

/// Reads the requested file from disk, so the query sees what the agent last wrote.
fn definition(index: Option<&mut Index>, params: GotoDefinitionParams) -> Vec<Location> {
    let Some(index) = index else {
        return Vec::new();
    };
    let position = params.text_document_position_params;
    let Some(path) = uri_to_path(&position.text_document.uri) else {
        return Vec::new();
    };
    let Ok(source) = std::fs::read(&path) else {
        return Vec::new();
    };
    let lines = LineIndex::new(&source);
    let Some(offset) = lines.offset(&source, position.position.line, position.position.character)
    else {
        return Vec::new();
    };
    index
        .definition(&path, &source, offset)
        .iter()
        .filter_map(|target| {
            let text = std::fs::read(&target.path).ok()?;
            let lines = LineIndex::new(&text);
            let (start_line, start_col) = lines.position(&text, target.start);
            let (end_line, end_col) = lines.position(&text, target.end);
            Some(Location {
                uri: path_to_uri(&target.path),
                range: Range::new(
                    Position::new(start_line, start_col),
                    Position::new(end_line, end_col),
                ),
            })
        })
        .collect()
}
