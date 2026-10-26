//! Call hierarchy: the callers and the callees of functions and methods.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::index::{Index, Location};
use crate::infer;
use crate::references;
use crate::summary::{FileSummary, MemberKind, NameKind, SymbolKind, summarize};

/// A function, method or file taking part in calls. `range` covers the whole
/// declaration; `selection` its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallItem {
    pub name: String,
    pub kind: CallKind,
    /// The declaring class for methods, the namespace for functions, the path for files.
    pub detail: String,
    pub path: PathBuf,
    pub range: (u32, u32),
    pub selection: (u32, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallKind {
    Function,
    Method,
    /// Code outside any function, in the file at `path`.
    File,
}

pub type Calls = Vec<(CallItem, Vec<(u32, u32)>)>;

/// Callers of `item`, each with the call ranges inside it.
pub(crate) fn incoming(index: &mut Index, item: &CallItem) -> Calls {
    if item.kind == CallKind::File {
        return Vec::new();
    }
    let target = Location {
        path: item.path.clone(),
        start: item.selection.0,
        end: item.selection.1,
    };
    let Some(symbol) = index.symbol_at(&target) else {
        return Vec::new();
    };
    let mut by_file: BTreeMap<PathBuf, Vec<Location>> = BTreeMap::new();
    for reference in references::scan(index, &symbol) {
        by_file
            .entry(reference.path.clone())
            .or_default()
            .push(reference);
    }
    let mut calls: Calls = Vec::new();
    for (path, references) in by_file {
        let Ok(source) = std::fs::read(&path) else {
            continue;
        };
        let summary = summarize(&source);
        for reference in references {
            if !is_call(&source, reference.end) {
                continue;
            }
            let caller = caller_at(&path, &summary, reference.start, source.len() as u32);
            add(&mut calls, caller, (reference.start, reference.end));
        }
    }
    calls
}

/// Functions and methods called inside `item`'s body, each with the call ranges.
pub(crate) fn outgoing(index: &mut Index, item: &CallItem) -> Calls {
    if item.kind == CallKind::File {
        return Vec::new();
    }
    let Ok(source) = std::fs::read(&item.path) else {
        return Vec::new();
    };
    let summary = summarize(&source);
    let inside = |offset: u32| item.range.0 <= offset && offset <= item.range.1;

    let mut sites: Vec<(u32, u32)> = summary
        .names
        .iter()
        .filter(|n| n.kind == NameKind::Function && inside(n.start) && is_call(&source, n.end))
        .map(|n| (n.start, n.end))
        .collect();
    sites.extend(
        summary
            .static_refs
            .iter()
            .filter(|r| r.kind == MemberKind::Method && inside(r.start))
            .map(|r| (r.start, r.end)),
    );
    sites.extend(
        infer::member_access_spans(&source)
            .into_iter()
            .filter(|(start, _, kind)| *kind == MemberKind::Method && inside(*start))
            .map(|(start, end, _)| (start, end)),
    );
    sites.sort_unstable();

    let mut calls: Calls = Vec::new();
    for site in sites {
        for target in index.definition(&item.path, &source, site.0) {
            if let Some(callee) = index.call_item_at(&target) {
                add(&mut calls, callee, site);
            }
        }
    }
    for name in &summary.names {
        if name.kind != NameKind::ClassLike
            || !inside(name.start)
            || !after_new(&source, name.start)
        {
            continue;
        }
        for (class, member) in index.find_member(&name.fqn, MemberKind::Method, "__construct") {
            let target = Location {
                path: class.path.to_path_buf(),
                start: member.name_start,
                end: member.name_end,
            };
            if let Some(callee) = index.call_item_at(&target) {
                add(&mut calls, callee, (name.start, name.end));
            }
        }
    }
    calls
}

fn add(calls: &mut Calls, item: CallItem, range: (u32, u32)) {
    match calls.iter_mut().find(|(existing, _)| *existing == item) {
        Some((_, ranges)) => {
            if !ranges.contains(&range) {
                ranges.push(range);
            }
        }
        None => calls.push((item, vec![range])),
    }
}

/// Whether the name ending at `end` is called: an opening parenthesis follows.
fn is_call(source: &[u8], end: u32) -> bool {
    source[end as usize..]
        .iter()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|&b| b == b'(')
}

/// Whether the name starting at `start` follows the `new` keyword.
fn after_new(source: &[u8], start: u32) -> bool {
    let before = source[..start as usize].trim_ascii_end();
    before.len() >= 3
        && before[before.len() - 3..].eq_ignore_ascii_case(b"new")
        && before
            .get(before.len().wrapping_sub(4))
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
}

/// The innermost function or method of the file containing `offset`, or the file.
fn caller_at(path: &Path, summary: &FileSummary, offset: u32, length: u32) -> CallItem {
    let contains = |range: (u32, u32)| range.0 <= offset && offset <= range.1;
    let mut best: Option<CallItem> = None;
    let mut consider = |candidate: CallItem| {
        if contains(candidate.range) && best.as_ref().is_none_or(|b| candidate.range.0 >= b.range.0)
        {
            best = Some(candidate);
        }
    };
    for declaration in &summary.declarations {
        if declaration.kind == SymbolKind::Function {
            let (namespace, name) = split(&declaration.fqn);
            consider(CallItem {
                name: name.to_string(),
                kind: CallKind::Function,
                detail: namespace.to_string(),
                path: path.to_path_buf(),
                range: declaration.range,
                selection: (declaration.name_start, declaration.name_end),
            });
        }
        for member in declaration.class.iter().flat_map(|s| s.members.iter()) {
            if member.kind == MemberKind::Method {
                consider(method_item(path, &declaration.fqn, member));
            }
        }
    }
    for range in summary.class_ranges.iter().filter(|r| r.fqn.is_none()) {
        for member in range
            .members
            .iter()
            .filter(|m| m.kind == MemberKind::Method)
        {
            consider(method_item(path, "class@anonymous", member));
        }
    }
    best.unwrap_or_else(|| CallItem {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        kind: CallKind::File,
        detail: path.display().to_string(),
        path: path.to_path_buf(),
        range: (0, length),
        selection: (0, 0),
    })
}

pub(crate) fn method_item(path: &Path, class: &str, member: &crate::summary::Member) -> CallItem {
    CallItem {
        name: member.name.clone(),
        kind: CallKind::Method,
        detail: class.to_string(),
        path: path.to_path_buf(),
        range: member.range,
        selection: (member.name_start, member.name_end),
    }
}

/// A fully qualified name split into its namespace and its short name.
pub(crate) fn split(fqn: &str) -> (&str, &str) {
    fqn.rsplit_once('\\').unwrap_or(("", fqn))
}
