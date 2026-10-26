//! Symbols of a file (its outline) and of the whole workspace.

use crate::summary::{MemberKind, SymbolKind, summarize};

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
