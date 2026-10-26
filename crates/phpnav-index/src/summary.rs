//! Declarations and resolved names of one PHP file.

use std::collections::{HashMap, HashSet};

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_names::resolver::NameResolver;
use mago_span::HasSpan;
use mago_syntax::cst::{
    Class, Constant, ConstantAccess, Enum, Expression, Function, FunctionCall,
    FunctionPartialApplication, Interface, Namespace, Trait, Use, UseItems, UseType,
};
use mago_syntax::parser::parse_file_content;
use mago_syntax::walker::Walker;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    ClassLike,
    Function,
    Constant,
}

/// A declared symbol. `fqn` keeps the source casing and has no leading backslash;
/// `name_start..name_end` covers the declared name token only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Declaration {
    pub kind: SymbolKind,
    pub fqn: String,
    pub name_start: u32,
    pub name_end: u32,
}

/// A use of a name, resolved against the file's namespace and imports.
/// `qualified` is true when the source text of the name contains a backslash;
/// `imported` is true when a `use` statement resolved it.
#[derive(Clone, Debug)]
pub struct NameRef {
    pub kind: NameKind,
    pub fqn: String,
    pub start: u32,
    pub end: u32,
    pub qualified: bool,
    pub imported: bool,
}

#[derive(Clone, Debug, Default)]
pub struct FileSummary {
    pub declarations: Vec<Declaration>,
    pub names: Vec<NameRef>,
    pub has_parse_errors: bool,
}

/// Parses `source` and returns its declarations and resolved names. Parsing is
/// tolerant: a syntax error does not hide the declarations around it.
pub fn summarize(source: &[u8]) -> FileSummary {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let resolved = NameResolver::new(&arena).resolve(program);

    let mut collector = Collector::default();
    SyntaxVisitor.walk_program(program, &mut collector);

    let resolved: HashMap<u32, (u32, &[u8], bool)> = resolved
        .iter()
        .map(|(start, end, fqn, imported)| (start, (end, fqn, imported)))
        .collect();

    let declarations: Vec<Declaration> = collector
        .declarations
        .iter()
        .filter_map(|&(kind, start, end)| {
            let (_, fqn, _) = resolved.get(&start)?;
            Some(Declaration {
                kind,
                fqn: to_fqn(fqn),
                name_start: start,
                name_end: end,
            })
        })
        .collect();
    let declaration_starts: HashSet<u32> = collector
        .declarations
        .iter()
        .map(|&(_, start, _)| start)
        .collect();

    let names = resolved
        .iter()
        .filter(|(start, _)| {
            !collector.not_refs.contains(start) && !declaration_starts.contains(start)
        })
        .map(|(&start, &(end, fqn, imported))| {
            let kind = if collector.functions.contains(&start) {
                NameKind::Function
            } else if collector.constants.contains(&start) {
                NameKind::Constant
            } else {
                NameKind::ClassLike
            };
            let text = source.get(start as usize..end as usize).unwrap_or_default();
            NameRef {
                kind,
                fqn: to_fqn(fqn),
                start,
                end,
                qualified: text.contains(&b'\\'),
                imported,
            }
        })
        .collect();

    FileSummary {
        declarations,
        names,
        has_parse_errors: program.has_errors(),
    }
}

fn to_fqn(name: &[u8]) -> String {
    let name = name.strip_prefix(b"\\").unwrap_or(name);
    String::from_utf8_lossy(name).into_owned()
}

/// Facts the name resolver does not expose: which resolved names are declarations,
/// functions, constants, or not symbol references at all. Keyed by start offset.
#[derive(Default)]
struct Collector {
    declarations: Vec<(SymbolKind, u32, u32)>,
    functions: HashSet<u32>,
    constants: HashSet<u32>,
    /// Namespace names, which the resolver records but which name no symbol.
    not_refs: HashSet<u32>,
}

struct SyntaxVisitor;

impl<'ast, 'arena> Walker<'ast, 'arena, Collector> for SyntaxVisitor {
    fn walk_in_namespace(&self, namespace: &'ast Namespace<'arena>, c: &mut Collector) {
        if let Some(name) = &namespace.name {
            c.not_refs.insert(name.span().start.offset);
        }
    }

    fn walk_in_class(&self, class: &'ast Class<'arena>, c: &mut Collector) {
        let span = class.name.span;
        c.declarations
            .push((SymbolKind::Class, span.start.offset, span.end.offset));
    }

    fn walk_in_interface(&self, interface: &'ast Interface<'arena>, c: &mut Collector) {
        let span = interface.name.span;
        c.declarations
            .push((SymbolKind::Interface, span.start.offset, span.end.offset));
    }

    fn walk_in_trait(&self, r#trait: &'ast Trait<'arena>, c: &mut Collector) {
        let span = r#trait.name.span;
        c.declarations
            .push((SymbolKind::Trait, span.start.offset, span.end.offset));
    }

    fn walk_in_enum(&self, r#enum: &'ast Enum<'arena>, c: &mut Collector) {
        let span = r#enum.name.span;
        c.declarations
            .push((SymbolKind::Enum, span.start.offset, span.end.offset));
    }

    fn walk_in_function(&self, function: &'ast Function<'arena>, c: &mut Collector) {
        let span = function.name.span;
        c.declarations
            .push((SymbolKind::Function, span.start.offset, span.end.offset));
    }

    fn walk_in_constant(&self, constant: &'ast Constant<'arena>, c: &mut Collector) {
        for item in &constant.items {
            c.constants.insert(item.name.span.start.offset);
        }
    }

    fn walk_in_function_call(&self, call: &'ast FunctionCall<'arena>, c: &mut Collector) {
        if let Expression::Identifier(identifier) = call.function {
            c.functions.insert(identifier.span().start.offset);
        }
    }

    fn walk_in_function_partial_application(
        &self,
        application: &'ast FunctionPartialApplication<'arena>,
        c: &mut Collector,
    ) {
        if let Expression::Identifier(identifier) = application.function {
            c.functions.insert(identifier.span().start.offset);
        }
    }

    fn walk_in_constant_access(&self, access: &'ast ConstantAccess<'arena>, c: &mut Collector) {
        c.constants.insert(access.name.span().start.offset);
    }

    fn walk_in_use(&self, r#use: &'ast Use<'arena>, c: &mut Collector) {
        match &r#use.items {
            UseItems::Sequence(_) => {}
            UseItems::TypedSequence(seq) => {
                for item in &seq.items {
                    mark_use_item(c, Some(&seq.r#type), item.name.span().start.offset);
                }
            }
            UseItems::TypedList(list) => {
                c.not_refs.insert(list.namespace.span().start.offset);
                for item in &list.items {
                    mark_use_item(c, Some(&list.r#type), item.name.span().start.offset);
                }
            }
            UseItems::MixedList(list) => {
                c.not_refs.insert(list.namespace.span().start.offset);
                for mixed in &list.items {
                    mark_use_item(
                        c,
                        mixed.r#type.as_ref(),
                        mixed.item.name.span().start.offset,
                    );
                }
            }
        }
    }
}

fn mark_use_item(c: &mut Collector, use_type: Option<&UseType<'_>>, start: u32) {
    match use_type {
        Some(UseType::Function(_)) => {
            c.functions.insert(start);
        }
        Some(UseType::Const(_)) => {
            c.constants.insert(start);
        }
        None => {}
    }
}
