//! References: candidate files from the key table, kept only where an occurrence
//! resolves to the symbol.

use std::path::{Component, Path, PathBuf};

use rayon::prelude::*;

use crate::index::{Index, Location};
use crate::infer::{self, Receiver};
use crate::summary::{ClassSel, FileSummary, MemberKind, NameKind, NameRef, RefKey, summarize};

/// A symbol whose references are searched.
pub(crate) enum Symbol {
    Class(String),
    Function(String),
    Constant(String),
    /// A member, with its family: its own declaration and every declaration of the
    /// same name it overrides or implements.
    Member {
        name: String,
        kind: MemberKind,
        family: Vec<Location>,
    },
}

impl Symbol {
    fn key(&self) -> RefKey {
        match self {
            Symbol::Class(fqn) => RefKey::Class(fqn.clone()),
            Symbol::Function(fqn) => RefKey::Function(fqn.clone()),
            Symbol::Constant(fqn) => RefKey::Constant(fqn.clone()),
            Symbol::Member { name, .. } => RefKey::Member(name.clone()),
        }
    }
}

/// References of `symbol` across its candidate files, scanned in parallel.
pub(crate) fn scan(index: &Index, symbol: &Symbol) -> Vec<Location> {
    let files: Vec<PathBuf> = index
        .candidate_files(&symbol.key())
        .into_iter()
        .map(Path::to_path_buf)
        .collect();
    files
        .par_iter()
        .flat_map_iter(|path| scan_file(index, path, symbol))
        .collect()
}

fn scan_file(index: &Index, path: &Path, symbol: &Symbol) -> Vec<Location> {
    let Ok(source) = std::fs::read(path) else {
        return Vec::new();
    };
    let summary = summarize(&source);
    let at = |start: u32, end: u32| Location {
        path: path.to_path_buf(),
        start,
        end,
    };
    let names = |kind: NameKind, matches: &dyn Fn(&NameRef) -> bool| -> Vec<Location> {
        summary
            .names
            .iter()
            .filter(|n| n.kind == kind && matches(n))
            .map(|n| at(n.start, n.end))
            .collect()
    };
    match symbol {
        Symbol::Class(fqn) => names(NameKind::ClassLike, &|n| n.fqn.eq_ignore_ascii_case(fqn)),
        Symbol::Function(fqn) => names(NameKind::Function, &|n| index.function_matches(n, fqn)),
        Symbol::Constant(fqn) => names(NameKind::Constant, &|n| index.constant_matches(n, fqn)),
        Symbol::Member { name, kind, family } => {
            let in_family = |targets: Vec<(Location, String)>| {
                targets.iter().any(|(target, _)| family.contains(target))
            };
            let mut found = Vec::new();
            for static_ref in &summary.static_refs {
                if static_ref.kind != *kind || !same_name(&static_ref.name, name, *kind) {
                    continue;
                }
                let targets = static_targets(index, path, &summary, static_ref, *kind, name);
                if in_family(targets) {
                    found.push(at(static_ref.start, static_ref.end));
                }
            }
            for (start, end, receiver, access_kind) in
                infer::member_accesses_named(index, &summary, &source, name)
            {
                if access_kind != *kind {
                    continue;
                }
                let targets = match receiver {
                    Receiver::Classes(classes) => index.member_targets(&classes, *kind, name),
                    Receiver::Anonymous(i) => {
                        index.anonymous_member_targets(path, &summary.class_ranges[i], *kind, name)
                    }
                };
                if in_family(targets) {
                    found.push(at(start, end));
                }
            }
            found
        }
    }
}

fn static_targets(
    index: &Index,
    path: &Path,
    summary: &FileSummary,
    static_ref: &crate::summary::StaticRef,
    kind: MemberKind,
    name: &str,
) -> Vec<(Location, String)> {
    let enclosing = summary.enclosing_class(static_ref.start);
    let class = match &static_ref.class {
        ClassSel::Named(fqn) => Some(fqn.clone()),
        ClassSel::SelfKw | ClassSel::StaticKw => match enclosing {
            Some(range) if range.fqn.is_none() => {
                return index.anonymous_member_targets(path, range, kind, name);
            }
            Some(range) => range.fqn.clone(),
            None => None,
        },
        ClassSel::ParentKw => enclosing.and_then(|range| range.parent.clone()),
    };
    class
        .map(|class| index.member_targets(&[class], kind, name))
        .unwrap_or_default()
}

/// Methods match case-insensitively; properties and constants exactly.
pub(crate) fn same_name(found: &str, wanted: &str, kind: MemberKind) -> bool {
    match kind {
        MemberKind::Method => found.eq_ignore_ascii_case(wanted),
        _ => found == wanted,
    }
}

/// Project code first, then contrib, then core, then `vendor`.
pub(crate) fn rank(path: &Path) -> u8 {
    let mut rank = 0;
    for component in path.components() {
        if let Component::Normal(name) = component {
            rank = rank.max(match name.to_str() {
                Some("vendor") => 3,
                Some("core") => 2,
                Some("contrib") => 1,
                _ => 0,
            });
        }
    }
    rank
}
