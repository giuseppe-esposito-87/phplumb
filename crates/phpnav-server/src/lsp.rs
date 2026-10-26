//! The LSP server: initialization, indexing and request handling.

use std::path::{Path, PathBuf};

use lsp_server::{Connection, ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::notification::{LogMessage, Notification as _, Progress};
use lsp_types::request::{GotoDefinition, Request as _, WorkDoneProgressCreate};
use lsp_types::{
    GotoDefinitionParams, InitializeParams, Location, LogMessageParams, MessageType,
    NumberOrString, OneOf, Position, ProgressParams, ProgressParamsValue, Range,
    ServerCapabilities, WorkDoneProgress, WorkDoneProgressBegin, WorkDoneProgressCreateParams,
    WorkDoneProgressEnd,
};
use phpnav_index::discover::DiscoverOptions;
use phpnav_index::line_index::LineIndex;
use phpnav_index::{Index, IndexStats};
use serde::Deserialize;
use serde_json::json;

use crate::convert::{path_to_uri, uri_to_path};

const PROGRESS_TOKEN: &str = "php-navigator/indexing";

/// Parsed from `initializationOptions`: `{ "exclude": ["vendor/rector/rector/**"] }`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ServerSettings {
    pub exclude: Vec<String>,
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

    // Requests that arrive while indexing wait in the connection's queue.
    let mut index = match workspace_root(&params) {
        Some(root) => Some(build_index(
            &connection,
            &root,
            &settings,
            reports_progress,
        )?),
        None => None,
    };
    main_loop(&connection, index.as_mut())
}

/// The first workspace folder, or the deprecated `rootUri` when there are none.
fn workspace_root(params: &InitializeParams) -> Option<PathBuf> {
    if let Some(folder) = params.workspace_folders.as_ref().and_then(|f| f.first()) {
        return uri_to_path(&folder.uri);
    }
    #[allow(deprecated)]
    params.root_uri.as_ref().and_then(uri_to_path)
}

fn build_index(
    connection: &Connection,
    root: &Path,
    settings: &ServerSettings,
    reports_progress: bool,
) -> anyhow::Result<Index> {
    let token = NumberOrString::String(PROGRESS_TOKEN.to_string());
    if reports_progress {
        connection.sender.send(Message::Request(Request::new(
            RequestId::from(PROGRESS_TOKEN.to_string()),
            WorkDoneProgressCreate::METHOD.to_string(),
            WorkDoneProgressCreateParams {
                token: token.clone(),
            },
        )))?;
        send_progress(
            connection,
            &token,
            WorkDoneProgress::Begin(WorkDoneProgressBegin {
                title: "PHP Navigator".to_string(),
                message: Some("Indexing".to_string()),
                ..WorkDoneProgressBegin::default()
            }),
        )?;
    }

    let opts = DiscoverOptions {
        exclude: settings.exclude.clone(),
        ..DiscoverOptions::default()
    };
    let (index, stats) = Index::build(root, &opts)?;
    let summary = summary_message(&stats);
    log_info(
        connection,
        format!("{summary} ({} declarations)", stats.declarations),
    )?;

    if reports_progress {
        send_progress(
            connection,
            &token,
            WorkDoneProgress::End(WorkDoneProgressEnd {
                message: Some(summary),
            }),
        )?;
    }
    Ok(index)
}

/// Sends a message to the client's log at info level.
fn log_info(connection: &Connection, message: String) -> anyhow::Result<()> {
    connection
        .sender
        .send(Message::Notification(Notification::new(
            LogMessage::METHOD.to_string(),
            LogMessageParams {
                typ: MessageType::INFO,
                message,
            },
        )))?;
    Ok(())
}

fn summary_message(stats: &IndexStats) -> String {
    format!("Indexed {} files in {} ms", stats.files, stats.elapsed_ms)
}

fn send_progress(
    connection: &Connection,
    token: &NumberOrString,
    progress: WorkDoneProgress,
) -> anyhow::Result<()> {
    connection
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

fn main_loop(connection: &Connection, mut index: Option<&mut Index>) -> anyhow::Result<()> {
    for message in &connection.receiver {
        let Message::Request(request) = message else {
            continue;
        };
        if connection.handle_shutdown(&request)? {
            return Ok(());
        }
        let response = match request.method.as_str() {
            GotoDefinition::METHOD => {
                match serde_json::from_value::<GotoDefinitionParams>(request.params) {
                    Ok(params) => {
                        Response::new_ok(request.id, definition(index.as_deref_mut(), params))
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
        connection.sender.send(Message::Response(response))?;
    }
    Ok(())
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
