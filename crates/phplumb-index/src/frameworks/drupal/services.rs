//! Drupal services: every `*.services.yml` on disk outside `vendor` and test
//! modules, and the classes a service id can give.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use yaml_rust2::parser::{Event, MarkedEventReceiver, Parser};
use yaml_rust2::scanner::Marker;

use crate::discover::chosen_path;
use crate::filter::{PathFilter, SKIPPED_DIR_NAMES, has_skipped_component};
use crate::index::Index;

/// One definition of a service id: where its key is and what it declares.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceDefinition {
    pub path: PathBuf,
    /// The id key in the YAML file.
    pub start: u32,
    pub end: u32,
    pub class: Option<String>,
    pub alias: Option<String>,
    pub parent: Option<String>,
    pub decorates: Option<String>,
}

#[derive(Default)]
pub(crate) struct Services {
    by_file: HashMap<PathBuf, Vec<(String, ServiceDefinition)>>,
    by_id: HashMap<String, Vec<ServiceDefinition>>,
    /// Decorated id to the ids decorating it.
    decorators: HashMap<String, Vec<String>>,
}

impl Services {
    /// Every services file below the filter's root.
    pub(crate) fn read(filter: &PathFilter) -> Services {
        let mut services = Services::default();
        for path in drupal_files(filter, filter.root(), SERVICES) {
            services.by_file.insert(path.clone(), parse_file(&path));
        }
        services.reindex();
        services
    }

    /// Re-reads changed services files and directories; drops removed ones.
    pub(crate) fn update(&mut self, filter: &PathFilter, changed: &[PathBuf], removed: &[PathBuf]) {
        let mut touched = false;
        for path in removed.iter().chain(changed) {
            let before = self.by_file.len();
            self.by_file.retain(|file, _| !file.starts_with(path));
            touched |= self.by_file.len() != before;
        }
        for path in changed {
            let files = if path.is_dir() {
                drupal_files(filter, path, SERVICES)
            } else {
                chosen_path(filter.root(), path)
                    .map(|(_, chosen)| chosen)
                    .filter(|chosen| is_drupal_file(filter, chosen, SERVICES) && chosen.is_file())
                    .into_iter()
                    .collect()
            };
            for file in files {
                self.by_file.insert(file.clone(), parse_file(&file));
                touched = true;
            }
        }
        if touched {
            self.reindex();
        }
    }

    pub(crate) fn definitions(&self, id: &str) -> &[ServiceDefinition] {
        self.by_id.get(id).map(Vec::as_slice).unwrap_or_default()
    }

    /// The classes `id` can give, sorted: see `Drupal::service_classes`.
    pub(crate) fn classes(&self, index: &Index, id: &str) -> Vec<String> {
        let mut found = BTreeSet::new();
        self.collect(index, id, &mut HashSet::new(), &mut found);
        found.into_iter().collect()
    }

    fn collect(
        &self,
        index: &Index,
        id: &str,
        seen: &mut HashSet<String>,
        found: &mut BTreeSet<String>,
    ) {
        if !seen.insert(id.to_string()) {
            return;
        }
        for definition in self.definitions(id) {
            match &definition.alias {
                Some(alias) => self.collect(index, alias, seen, found),
                None => found.extend(self.own_class(index, id, definition, &mut HashSet::new())),
            }
        }
        for decorator in self.decorators.get(id).into_iter().flatten() {
            self.collect(index, decorator, seen, found);
        }
    }

    /// The class a definition declares, takes from its parent, or names as its id.
    fn own_class(
        &self,
        index: &Index,
        id: &str,
        definition: &ServiceDefinition,
        seen: &mut HashSet<String>,
    ) -> Option<String> {
        if let Some(class) = &definition.class {
            return Some(class.clone());
        }
        if let Some(parent) = &definition.parent {
            if !seen.insert(parent.clone()) {
                return None;
            }
            return self
                .definitions(parent)
                .iter()
                .find_map(|p| self.own_class(index, parent, p, seen));
        }
        index.has_class(id).then(|| id.to_string())
    }

    fn reindex(&mut self) {
        self.by_id.clear();
        self.decorators.clear();
        let mut files: Vec<&PathBuf> = self.by_file.keys().collect();
        files.sort();
        for file in files {
            for (id, definition) in &self.by_file[file] {
                if let Some(decorated) = &definition.decorates {
                    self.decorators
                        .entry(decorated.clone())
                        .or_default()
                        .push(id.clone());
                }
                self.by_id
                    .entry(id.clone())
                    .or_default()
                    .push(definition.clone());
            }
        }
    }
}

const SERVICES: &str = ".services.yml";

/// Whether `path` is a file named `*{suffix}` that the Drupal registries read:
/// outside `vendor`, test modules and hidden or skipped directories.
pub(super) fn is_drupal_file(filter: &PathFilter, path: &Path, suffix: &str) -> bool {
    let name_matches = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(suffix));
    let Ok(relative) = path.strip_prefix(filter.root()) else {
        return false;
    };
    name_matches
        && !filter.excludes(relative)
        && !has_skipped_component(relative)
        && !relative.components().any(|c| {
            let name = c.as_os_str();
            name == "vendor" || name == "tests" || SKIPPED_DIR_NAMES.iter().any(|s| name == *s)
        })
}

/// The files named `*{suffix}` that the Drupal registries read below `start`, a
/// directory under the filter's root, each once under the path PHP discovery would
/// choose for it.
pub(super) fn drupal_files(filter: &PathFilter, start: &Path, suffix: &str) -> Vec<PathBuf> {
    if !filter.accepts_dir(start) {
        return Vec::new();
    }
    let walker = WalkBuilder::new(start)
        .hidden(true)
        .parents(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .follow_links(true)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let name = entry.file_name();
            !(is_dir
                && (name == "vendor"
                    || name == "tests"
                    || SKIPPED_DIR_NAMES.iter().any(|s| name == *s)))
        })
        .build();
    let mut by_canonical: HashMap<PathBuf, PathBuf> = HashMap::new();
    for entry in walker.flatten() {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let Some((canonical, chosen)) = chosen_path(filter.root(), entry.path()) else {
            continue;
        };
        if is_drupal_file(filter, &chosen, suffix) {
            by_canonical.entry(canonical).or_insert(chosen);
        }
    }
    let mut files: Vec<PathBuf> = by_canonical.into_values().collect();
    files.sort();
    files
}

/// The service definitions of one file, with their ids; nothing when it does not
/// parse.
fn parse_file(path: &Path) -> Vec<(String, ServiceDefinition)> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut builder = Builder::default();
    if Parser::new_from_str(&text)
        .load(&mut builder, false)
        .is_err()
    {
        return Vec::new();
    }
    let Some(Node::Map(top)) = builder.root else {
        return Vec::new();
    };
    let Some((_, Node::Map(services))) = top
        .into_iter()
        .find(|(key, _)| key.text() == Some("services"))
    else {
        return Vec::new();
    };
    let lines: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let mut found = Vec::new();
    for (key, value) in services {
        let Node::Scalar {
            value: id,
            line,
            column,
        } = key
        else {
            continue;
        };
        if id.starts_with('_') {
            continue;
        }
        let (start, end) = key_span(&text, &lines, line, column, &id);
        let mut definition = ServiceDefinition {
            path: path.to_path_buf(),
            start,
            end,
            class: None,
            alias: None,
            parent: None,
            decorates: None,
        };
        match value {
            Node::Scalar { value, .. } => {
                definition.alias = value.strip_prefix('@').map(str::to_string);
            }
            Node::Map(entries) => {
                for (key, value) in entries {
                    let Some(text) = value.text() else {
                        continue;
                    };
                    let text = text.trim_start_matches('\\').to_string();
                    match key.text() {
                        Some("class") => definition.class = Some(text),
                        Some("alias") => definition.alias = Some(text),
                        Some("parent") => definition.parent = Some(text),
                        Some("decorates") => definition.decorates = Some(text),
                        _ => {}
                    }
                }
            }
            Node::Other => {}
        }
        found.push((id, definition));
    }
    found
}

/// The byte range of the key `id` written on 1-based `line` from character
/// `column`: the id itself, or, when the written key differs from its value (escapes),
/// an empty range where the key starts.
fn key_span(text: &str, lines: &[usize], line: usize, column: usize, id: &str) -> (u32, u32) {
    let Some(&line_start) = lines.get(line.saturating_sub(1)) else {
        return (0, 0);
    };
    let line_text = &text[line_start..];
    let line_text = &line_text[..line_text.find('\n').unwrap_or(line_text.len())];
    let key_start = line_start
        + line_text
            .char_indices()
            .nth(column)
            .map_or(line_text.len(), |(i, _)| i);
    for start in [key_start, key_start + 1] {
        if text.get(start..start + id.len()) == Some(id) {
            return (start as u32, (start + id.len()) as u32);
        }
    }
    (key_start as u32, key_start as u32)
}

/// A YAML node, keeping where scalars start: a 1-based line and a character column.
enum Node {
    Scalar {
        value: String,
        line: usize,
        column: usize,
    },
    Map(Vec<(Node, Node)>),
    Other,
}

impl Node {
    fn text(&self) -> Option<&str> {
        match self {
            Node::Scalar { value, .. } => Some(value),
            _ => None,
        }
    }
}

enum Frame {
    Map(Vec<(Node, Node)>, Option<Node>),
    Sequence,
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    root: Option<Node>,
}

impl Builder {
    fn push(&mut self, node: Node) {
        match self.stack.last_mut() {
            Some(Frame::Map(entries, key)) => match key.take() {
                Some(key) => entries.push((key, node)),
                None => *key = Some(node),
            },
            Some(Frame::Sequence) => {}
            None => self.root = Some(node),
        }
    }
}

impl MarkedEventReceiver for Builder {
    fn on_event(&mut self, event: Event, mark: Marker) {
        match event {
            Event::Scalar(value, ..) => self.push(Node::Scalar {
                value,
                line: mark.line(),
                column: mark.col(),
            }),
            Event::MappingStart(..) => self.stack.push(Frame::Map(Vec::new(), None)),
            Event::MappingEnd => {
                if let Some(Frame::Map(entries, _)) = self.stack.pop() {
                    self.push(Node::Map(entries));
                }
            }
            Event::SequenceStart(..) => self.stack.push(Frame::Sequence),
            Event::SequenceEnd => {
                self.stack.pop();
                self.push(Node::Other);
            }
            Event::Alias(_) => self.push(Node::Other),
            _ => {}
        }
    }
}
