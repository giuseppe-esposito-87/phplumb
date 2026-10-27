//! Symbols of a file (its outline) and of the whole workspace.

use std::path::Path;

use crate::index::Location;
use crate::references;
use crate::summary::{Declaration, MemberKind, SymbolKind, summarize};

/// What a symbol is: a top-level declaration or a member of a class-like.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Declaration(SymbolKind),
    Member(MemberKind),
}

/// A named declaration of a file. `range` covers the whole declaration and always
/// contains `selection`, its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineSymbol {
    pub name: String,
    pub kind: Kind,
    pub range: (u32, u32),
    pub selection: (u32, u32),
    /// Members of a class-like, in source order.
    pub children: Vec<OutlineSymbol>,
}

/// The named declarations of `source`, in source order, with class-like members as
/// children. Anonymous classes and closures are left out.
pub fn outline(source: &[u8]) -> Vec<OutlineSymbol> {
    let summary = summarize(source);
    let mut symbols: Vec<OutlineSymbol> = summary
        .declarations
        .iter()
        .map(|declaration| {
            let mut children: Vec<OutlineSymbol> = declaration
                .class
                .iter()
                .flat_map(|shape| shape.members.iter())
                .map(|member| {
                    symbol(
                        member.name.clone(),
                        Kind::Member(member.kind),
                        member.range,
                        (member.name_start, member.name_end),
                        Vec::new(),
                    )
                })
                .collect();
            children.sort_by_key(|c| (c.range.0, c.selection.0));
            let name = declaration
                .fqn
                .rsplit_once('\\')
                .map_or(declaration.fqn.as_str(), |(_, short)| short);
            symbol(
                name.to_string(),
                Kind::Declaration(declaration.kind),
                declaration.range,
                (declaration.name_start, declaration.name_end),
                children,
            )
        })
        .collect();
    symbols.sort_by_key(|s| (s.range.0, s.selection.0));
    symbols
}

fn symbol(
    name: String,
    kind: Kind,
    range: (u32, u32),
    selection: (u32, u32),
    children: Vec<OutlineSymbol>,
) -> OutlineSymbol {
    OutlineSymbol {
        name,
        kind,
        range: (range.0.min(selection.0), range.1.max(selection.1)),
        selection,
        children,
    }
}

/// A declaration or member matching a workspace symbol query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolMatch {
    pub name: String,
    pub kind: Kind,
    /// The namespace of a declaration (`None` when global), the class of a member.
    pub container: Option<String>,
    /// The name.
    pub location: Location,
}

/// The declarations and members of `files` matching `query`, best first, at most
/// `limit`. Matching ignores ASCII case. A query with a backslash matches the fully
/// qualified names of declarations; any other query matches short names. Exact
/// matches come first, then prefixes, substrings and subsequences; then
/// declarations before members, project code before contrib, core and `vendor`,
/// and shorter names first.
pub(crate) fn search<'a>(
    files: impl Iterator<Item = (&'a Path, &'a [Declaration])>,
    query: &str,
    limit: usize,
) -> Vec<SymbolMatch> {
    let query = query.trim();
    let qualified = query.contains('\\');
    let query = query.trim_start_matches('\\').to_ascii_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let query = query.as_bytes();

    let mut hits: Vec<Hit<'a>> = Vec::new();
    for (path, declarations) in files {
        let rank = references::rank(path);
        for declaration in declarations {
            let (namespace, short) = split(&declaration.fqn);
            let text = if qualified {
                declaration.fqn.as_str()
            } else {
                short
            };
            if let Some(tier) = tier(text.as_bytes(), query) {
                hits.push(Hit {
                    key: (tier, false, rank, short.len()),
                    name: short,
                    path,
                    start: declaration.name_start,
                    declaration,
                    member: None,
                    namespace,
                });
            }
            if qualified {
                continue;
            }
            for (i, member) in declaration
                .class
                .iter()
                .flat_map(|c| c.members.iter())
                .enumerate()
            {
                if let Some(tier) = tier(member.name.as_bytes(), query) {
                    hits.push(Hit {
                        key: (tier, true, rank, member.name.len()),
                        name: &member.name,
                        path,
                        start: member.name_start,
                        declaration,
                        member: Some(i),
                        namespace,
                    });
                }
            }
        }
    }
    let order =
        |a: &Hit, b: &Hit| (a.key, a.name, a.path, a.start).cmp(&(b.key, b.name, b.path, b.start));
    if hits.len() > limit {
        hits.select_nth_unstable_by(limit - 1, order);
        hits.truncate(limit);
    }
    hits.sort_by(order);
    hits.into_iter().map(Hit::into_match).collect()
}

struct Hit<'a> {
    /// Tier, member or not, path rank, name length.
    key: (u8, bool, u8, usize),
    name: &'a str,
    path: &'a Path,
    start: u32,
    declaration: &'a Declaration,
    member: Option<usize>,
    namespace: &'a str,
}

impl Hit<'_> {
    fn into_match(self) -> SymbolMatch {
        let members = || self.declaration.class.iter().flat_map(|c| c.members.iter());
        let (kind, end, container) = match self.member.and_then(|i| members().nth(i)) {
            Some(member) => (
                Kind::Member(member.kind),
                member.name_end,
                Some(self.declaration.fqn.clone()),
            ),
            None => (
                Kind::Declaration(self.declaration.kind),
                self.declaration.name_end,
                (!self.namespace.is_empty()).then(|| self.namespace.to_string()),
            ),
        };
        SymbolMatch {
            name: self.name.to_string(),
            kind,
            container,
            location: Location {
                path: self.path.to_path_buf(),
                start: self.start,
                end,
            },
        }
    }
}

/// How `name` matches the lowercase `query`: 0 exact, 1 prefix, 2 substring,
/// 3 subsequence, ignoring ASCII case.
fn tier(name: &[u8], query: &[u8]) -> Option<u8> {
    if name.len() < query.len() {
        return None;
    }
    if name.len() == query.len() && name.eq_ignore_ascii_case(query) {
        return Some(0);
    }
    if name[..query.len()].eq_ignore_ascii_case(query) {
        return Some(1);
    }
    if name
        .windows(query.len())
        .any(|w| w.eq_ignore_ascii_case(query))
    {
        return Some(2);
    }
    let mut wanted = query.iter().peekable();
    for byte in name {
        if wanted
            .peek()
            .is_some_and(|&&w| w == byte.to_ascii_lowercase())
        {
            wanted.next();
        }
    }
    wanted.peek().is_none().then_some(3)
}

/// A fully qualified name split into its namespace and its short name.
fn split(fqn: &str) -> (&str, &str) {
    fqn.rsplit_once('\\').unwrap_or(("", fqn))
}
