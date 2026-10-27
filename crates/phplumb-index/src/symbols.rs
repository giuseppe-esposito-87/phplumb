//! Symbols of a file (its outline) and of the whole workspace.

use std::path::Path;

use crate::index::Location;
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
/// matches come first, then names whose words start with the query's pieces from
/// the first letter (`gEnt` for `getEntity`), prefixes, the same pieces from a
/// later word, substrings and subsequences. When the query has a capital letter,
/// names whose first letter has the query's case come first within a tier. Then
/// declarations before members, lower `rank` of the path first, and shorter names
/// first.
pub(crate) fn search<'a>(
    files: impl Iterator<Item = (&'a Path, &'a [Declaration])>,
    query: &str,
    limit: usize,
    rank: impl Fn(&Path) -> u8,
) -> Vec<SymbolMatch> {
    let query = query.trim();
    let qualified = query.contains('\\');
    let original = query.trim_start_matches('\\');
    if original.is_empty() || limit == 0 {
        return Vec::new();
    }
    let lower = original.to_ascii_lowercase();
    let query = Query {
        lower: lower.as_bytes(),
        segments: segments(original.as_bytes()),
        first: original
            .bytes()
            .any(|b| b.is_ascii_uppercase())
            .then(|| original.as_bytes()[0]),
    };
    let query = &query;

    let mut hits: Vec<Hit<'a>> = Vec::new();
    for (path, declarations) in files {
        let rank = rank(path);
        for declaration in declarations {
            let (namespace, short) = split(&declaration.fqn);
            let text = if qualified {
                declaration.fqn.as_str()
            } else {
                short
            };
            if let Some(tier) = tier(text.as_bytes(), query) {
                hits.push(Hit {
                    key: (
                        tier,
                        query.case_mismatch(short.as_bytes()),
                        false,
                        rank,
                        short.len(),
                    ),
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
                        key: (
                            tier,
                            query.case_mismatch(member.name.as_bytes()),
                            true,
                            rank,
                            member.name.len(),
                        ),
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
    /// Tier, first-letter case mismatch, member or not, path rank, name length.
    key: (u8, u8, bool, u8, usize),
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

/// A search query: lowercased, and split into the pieces that match the starts
/// of words in a name (`gEnt` into `g` and `Ent`, `ESI` into `E`, `S` and `I`).
struct Query<'a> {
    lower: &'a [u8],
    segments: Vec<&'a [u8]>,
    /// The first character, when the query's case is meaningful.
    first: Option<u8>,
}

impl Query<'_> {
    /// 1 when the query's case is meaningful and `name` starts with the other case.
    fn case_mismatch(&self, name: &[u8]) -> u8 {
        match (self.first, name.first()) {
            (Some(q), Some(&n)) if q.is_ascii_alphabetic() && n.is_ascii_alphabetic() => {
                u8::from(q.is_ascii_uppercase() != n.is_ascii_uppercase())
            }
            _ => 0,
        }
    }
}

/// The pieces of `query`: a new piece starts at each uppercase letter and after
/// each `_` or `\\`, which are dropped.
fn segments(query: &[u8]) -> Vec<&[u8]> {
    let mut pieces = Vec::new();
    let mut start = 0;
    for (i, &byte) in query.iter().enumerate() {
        if byte == b'_' || byte == b'\\' {
            if start < i {
                pieces.push(&query[start..i]);
            }
            start = i + 1;
        } else if byte.is_ascii_uppercase() && start < i {
            pieces.push(&query[start..i]);
            start = i;
        }
    }
    if start < query.len() {
        pieces.push(&query[start..]);
    }
    pieces
}

/// How `name` matches `query`, ignoring ASCII case: 0 exact, 1 every piece at the
/// start of a word, in order, from the first letter, 2 prefix, 3 every piece at
/// the start of a word from a later word, 4 substring, 5 subsequence.
fn tier(name: &[u8], query: &Query) -> Option<u8> {
    let lower = query.lower;
    if name.len() < lower.len() {
        return None;
    }
    if name.len() == lower.len() && name.eq_ignore_ascii_case(lower) {
        return Some(0);
    }
    if let Some((first, rest)) = query.segments.split_first()
        && name[..first.len()].eq_ignore_ascii_case(first)
        && humps(name, rest, first.len())
    {
        return Some(1);
    }
    if name[..lower.len()].eq_ignore_ascii_case(lower) {
        return Some(2);
    }
    if humps(name, &query.segments, 0) {
        return Some(3);
    }
    if name
        .windows(lower.len())
        .any(|w| w.eq_ignore_ascii_case(lower))
    {
        return Some(4);
    }
    let mut wanted = lower.iter().peekable();
    for byte in name {
        if wanted
            .peek()
            .is_some_and(|&&w| w == byte.to_ascii_lowercase())
        {
            wanted.next();
        }
    }
    wanted.peek().is_none().then_some(5)
}

/// Whether each of `pieces` starts a word of `name`, in order, from `from` on.
fn humps(name: &[u8], pieces: &[&[u8]], from: usize) -> bool {
    let Some((piece, rest)) = pieces.split_first() else {
        return true;
    };
    (from..name.len()).any(|i| {
        word_start(name, i)
            && name[i..].len() >= piece.len()
            && name[i..i + piece.len()].eq_ignore_ascii_case(piece)
            && humps(name, rest, i + piece.len())
    })
}

/// Whether a word starts at `i`: the first character, a character after `_` or
/// `\\`, an uppercase letter after a lowercase letter or digit, or the last
/// uppercase letter of a run followed by a lowercase one (`P` in `HTMLParser`).
fn word_start(name: &[u8], i: usize) -> bool {
    let Some(&current) = name.get(i) else {
        return false;
    };
    if i == 0 {
        return true;
    }
    let previous = name[i - 1];
    if previous == b'_' || previous == b'\\' {
        return current != b'_' && current != b'\\';
    }
    current.is_ascii_uppercase()
        && (previous.is_ascii_lowercase()
            || previous.is_ascii_digit()
            || (previous.is_ascii_uppercase()
                && name.get(i + 1).is_some_and(|n| n.is_ascii_lowercase())))
}

/// A fully qualified name split into its namespace and its short name.
fn split(fqn: &str) -> (&str, &str) {
    fqn.rsplit_once('\\').unwrap_or(("", fqn))
}
