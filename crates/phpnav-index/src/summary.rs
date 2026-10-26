//! Declarations, class shapes and resolved names of one PHP file.

use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hasher};

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_names::resolver::NameResolver;
use mago_span::HasSpan;
use mago_syntax::comments::docblock::get_docblock_before_position;
use mago_syntax::cst::{
    AnonymousClass, Argument, Class, ClassConstantAccess, ClassLikeConstantSelector,
    ClassLikeMember, ClassLikeMemberSelector, Constant, ConstantAccess, Enum, EnumCaseItem,
    Expression, Function, FunctionCall, FunctionLikeParameterList, FunctionLikeReturnTypeHint,
    FunctionPartialApplication, Hint, Instantiation, Interface, Literal, Method, MethodCall,
    Modifier, Namespace, NullSafeMethodCall, NullSafePropertyAccess, Program, Property,
    PropertyAccess, PropertyItem, Sequence, StaticMethodCall, StaticMethodPartialApplication,
    StaticPropertyAccess, Trait, Use, UseItems, UseType, Variable,
};
use mago_syntax::parser::parse_file_content;
use mago_syntax::walker::Walker;
use serde::{Deserialize, Serialize};

use crate::phpdoc::{NameContext, TypeExpr, parse_docblock, parse_type};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Class,
    Interface,
    Trait,
    Enum,
    Function,
    Constant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameKind {
    ClassLike,
    Function,
    Constant,
}

/// A symbol whose references can be searched. Constants keep their name's case;
/// everything else is matched case-insensitively.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RefKey {
    Class(String),
    Function(String),
    Constant(String),
    Member(String),
}

impl RefKey {
    /// A stable hash of the normalized key. Two keys may collide; that only adds a
    /// candidate file that verification drops.
    pub(crate) fn hash(&self) -> u64 {
        let (prefix, body) = match self {
            RefKey::Class(fqn) => (b'c', bare(fqn).to_ascii_lowercase()),
            RefKey::Function(fqn) => (b'f', bare(fqn).to_ascii_lowercase()),
            RefKey::Constant(fqn) => (
                b'k',
                match bare(fqn).rsplit_once('\\') {
                    Some((namespace, name)) => {
                        format!("{}\\{name}", namespace.to_ascii_lowercase())
                    }
                    None => bare(fqn).to_string(),
                },
            ),
            RefKey::Member(name) => (b'm', name.to_ascii_lowercase()),
        };
        let mut hasher = DefaultHasher::new();
        hasher.write_u8(prefix);
        hasher.write(body.as_bytes());
        hasher.finish()
    }
}

fn bare(fqn: &str) -> &str {
    fqn.trim_start_matches('\\')
}

/// A declared symbol. `fqn` keeps the source casing and has no leading backslash;
/// `name_start..name_end` covers the declared name token only.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Declaration {
    pub kind: SymbolKind,
    pub fqn: String,
    pub name_start: u32,
    pub name_end: u32,
    /// The declaration head on one line, without the body.
    #[serde(default)]
    pub signature: String,
    /// The raw docblock text.
    #[serde(default)]
    pub doc: Option<String>,
    /// Functions: the return type, PHPDoc first when it names a class.
    #[serde(default)]
    pub returns: Option<TypeExpr>,
    /// Class-likes: parent, interfaces, traits and members.
    #[serde(default)]
    pub class: Option<ClassShape>,
    /// The whole declaration, body included.
    #[serde(default)]
    pub range: (u32, u32),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClassShape {
    pub parent: Option<String>,
    /// `implements` for classes and enums, `extends` for interfaces.
    pub interfaces: Vec<String>,
    pub traits: Vec<String>,
    pub members: Vec<Member>,
    /// An abstract class or an interface.
    #[serde(default)]
    pub is_abstract: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberKind {
    Method,
    Property,
    Constant,
    EnumCase,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Member {
    pub kind: MemberKind,
    /// Properties without `$`.
    pub name: String,
    pub is_static: bool,
    pub name_start: u32,
    pub name_end: u32,
    pub signature: String,
    pub doc: Option<String>,
    /// Property type or method return type, PHPDoc first when it names a class.
    pub ty: Option<TypeExpr>,
    /// Methods: parameter names without `$`, with their types.
    pub params: Vec<(String, Option<TypeExpr>)>,
    /// An abstract method or an interface method.
    #[serde(default)]
    pub is_abstract: bool,
    /// Declared `private`: not inherited by subclasses.
    #[serde(default)]
    pub is_private: bool,
    /// The whole member, body included.
    #[serde(default)]
    pub range: (u32, u32),
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

/// The namespaces and class imports of a file, for resolving names written in
/// docblocks and type hints.
#[derive(Clone, Debug)]
pub struct FileNames {
    scopes: Vec<NameScope>,
}

#[derive(Clone, Debug, Default)]
struct NameScope {
    start: u32,
    end: u32,
    namespace: String,
    /// Lowercased alias to fully qualified name.
    imports: HashMap<String, String>,
}

impl Default for FileNames {
    fn default() -> Self {
        FileNames {
            scopes: vec![NameScope {
                end: u32::MAX,
                ..NameScope::default()
            }],
        }
    }
}

impl FileNames {
    /// The names in effect at `offset`.
    pub fn at(&self, offset: u32) -> ScopeNames<'_> {
        ScopeNames {
            scope: self.scope_index(offset).map(|i| &self.scopes[i]),
        }
    }

    fn scope_index(&self, offset: u32) -> Option<usize> {
        self.scopes
            .iter()
            .rposition(|s| s.start <= offset && offset <= s.end)
    }
}

pub struct ScopeNames<'a> {
    scope: Option<&'a NameScope>,
}

impl ScopeNames<'_> {
    pub fn namespace(&self) -> &str {
        self.scope.map_or("", |s| s.namespace.as_str())
    }
}

impl NameContext for ScopeNames<'_> {
    fn resolve_class(&self, name: &str) -> String {
        let Some(scope) = self.scope else {
            return name.to_string();
        };
        let (first, rest) = match name.split_once('\\') {
            Some((first, rest)) => (first, Some(rest)),
            None => (name, None),
        };
        if let Some(target) = scope.imports.get(&first.to_ascii_lowercase()) {
            return match rest {
                Some(rest) => format!("{target}\\{rest}"),
                None => target.clone(),
            };
        }
        if scope.namespace.is_empty() {
            name.to_string()
        } else {
            format!("{}\\{name}", scope.namespace)
        }
    }
}

/// The class a static access or class keyword refers to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClassSel {
    Named(String),
    SelfKw,
    StaticKw,
    ParentKw,
}

/// `X::method()`, `X::CONST`, `X::Case` or `X::$prop`. `kind` is `Constant` for
/// both constants and enum cases; `name` has no `$`; the range covers the member
/// name as written, `$` included.
#[derive(Clone, Debug)]
pub struct StaticRef {
    pub kind: MemberKind,
    pub name: String,
    pub start: u32,
    pub end: u32,
    pub class: ClassSel,
}

/// An occurrence of `self`, `static` or `parent` naming a class.
#[derive(Clone, Debug)]
pub struct KeywordRef {
    pub start: u32,
    pub end: u32,
    pub class: ClassSel,
}

/// The source range of a class-like declaration, for finding the enclosing class.
/// Anonymous classes have no `fqn` and carry their own `members`, since the index
/// cannot name them; named classes leave `members` empty.
#[derive(Clone, Debug)]
pub struct ClassRange {
    pub start: u32,
    pub end: u32,
    pub fqn: Option<String>,
    pub parent: Option<String>,
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, Default)]
pub struct FileSummary {
    pub declarations: Vec<Declaration>,
    pub names: Vec<NameRef>,
    pub has_parse_errors: bool,
    pub file_names: FileNames,
    pub static_refs: Vec<StaticRef>,
    pub keywords: Vec<KeywordRef>,
    pub class_ranges: Vec<ClassRange>,
    /// Hashes of every `RefKey` the file mentions or declares, sorted and unique.
    pub ref_keys: Vec<u64>,
}

impl FileSummary {
    /// The innermost class-like declaration containing `offset`.
    pub fn enclosing_class(&self, offset: u32) -> Option<&ClassRange> {
        self.class_ranges
            .iter()
            .filter(|r| r.start <= offset && offset <= r.end)
            .max_by_key(|r| r.start)
    }

    /// The named class a `self`, `static` or `parent` keyword stands for, from the
    /// class around it.
    pub fn keyword_class(&self, keyword: &KeywordRef) -> Option<String> {
        let range = self.enclosing_class(keyword.start)?;
        match keyword.class {
            ClassSel::ParentKw => range.parent.clone(),
            _ => range.fqn.clone(),
        }
    }
}

/// Parses `source` and returns its declarations and resolved names. Parsing is
/// tolerant: a syntax error does not hide the declarations around it.
pub fn summarize(source: &[u8]) -> FileSummary {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let resolved = NameResolver::new(&arena).resolve(program);

    let mut collector = Collector {
        source,
        program,
        names: FileNames::default(),
        pending: Vec::new(),
        functions: HashSet::new(),
        constants: HashSet::new(),
        not_refs: HashSet::new(),
        static_refs: Vec::new(),
        keywords: Vec::new(),
        anonymous: Vec::new(),
        member_names: HashSet::new(),
    };
    SyntaxVisitor.walk_program(program, &mut collector);

    let resolved: HashMap<u32, (u32, &[u8], bool)> = resolved
        .iter()
        .map(|(start, end, fqn, imported)| (start, (end, fqn, imported)))
        .collect();
    let fqn_at = |start: u32| resolved.get(&start).map(|(_, fqn, _)| to_fqn(fqn));

    let declaration_starts: HashSet<u32> = collector.pending.iter().map(|p| p.start).collect();
    let resolve_sel = |sel: PendingSel| match sel {
        PendingSel::Named(start) => fqn_at(start).map(ClassSel::Named),
        PendingSel::Keyword(sel) => Some(sel),
    };
    let static_refs: Vec<StaticRef> = collector
        .static_refs
        .into_iter()
        .filter_map(|(kind, name, start, end, sel)| {
            Some(StaticRef {
                kind,
                name,
                start,
                end,
                class: resolve_sel(sel)?,
            })
        })
        .collect();
    let mut class_ranges = Vec::new();
    let declarations: Vec<Declaration> = collector
        .pending
        .into_iter()
        .filter_map(|pending| {
            let fqn = pending.fqn.or_else(|| fqn_at(pending.start))?;
            if let (Some((start, end)), Some(shape)) = (pending.range, &pending.shape) {
                class_ranges.push(ClassRange {
                    start,
                    end,
                    fqn: Some(fqn.clone()),
                    parent: shape.parent.and_then(fqn_at),
                    members: Vec::new(),
                });
            }
            let class = pending.shape.map(|shape| ClassShape {
                parent: shape.parent.and_then(fqn_at),
                interfaces: shape.interfaces.into_iter().filter_map(fqn_at).collect(),
                traits: shape.traits.into_iter().filter_map(fqn_at).collect(),
                members: shape.members,
                is_abstract: shape.is_abstract,
            });
            Some(Declaration {
                kind: pending.kind,
                fqn,
                name_start: pending.start,
                name_end: pending.end,
                signature: pending.signature,
                doc: pending.doc,
                returns: pending.returns,
                class,
                range: pending.span,
            })
        })
        .collect();

    class_ranges.extend(collector.anonymous.into_iter().map(|anonymous| ClassRange {
        start: anonymous.start,
        end: anonymous.end,
        fqn: None,
        parent: anonymous.parent.and_then(fqn_at),
        members: anonymous.members,
    }));

    let names: Vec<NameRef> = resolved
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

    let mut keys: HashSet<u64> = HashSet::new();
    for name in &names {
        let fallback = !name.qualified && !name.imported;
        let short = name
            .fqn
            .rsplit_once('\\')
            .map(|(_, short)| short.to_string());
        let key = |fqn: String| match name.kind {
            NameKind::ClassLike => RefKey::Class(fqn),
            NameKind::Function => RefKey::Function(fqn),
            NameKind::Constant => RefKey::Constant(fqn),
        };
        keys.insert(key(name.fqn.clone()).hash());
        // Unqualified functions and constants may reach the global one.
        if fallback
            && name.kind != NameKind::ClassLike
            && let Some(short) = short
        {
            keys.insert(key(short).hash());
        }
    }
    for declaration in &declarations {
        let key = match declaration.kind {
            SymbolKind::Function => RefKey::Function(declaration.fqn.clone()),
            SymbolKind::Constant => RefKey::Constant(declaration.fqn.clone()),
            _ => RefKey::Class(declaration.fqn.clone()),
        };
        keys.insert(key.hash());
        for member in declaration.class.iter().flat_map(|c| c.members.iter()) {
            keys.insert(RefKey::Member(member.name.clone()).hash());
        }
    }
    let accessed = collector.member_names.iter();
    for name in accessed.chain(static_refs.iter().map(|r: &StaticRef| &r.name)) {
        keys.insert(RefKey::Member(name.clone()).hash());
    }
    let mut ref_keys: Vec<u64> = keys.into_iter().collect();
    ref_keys.sort_unstable();

    FileSummary {
        declarations,
        names,
        has_parse_errors: program.has_errors(),
        file_names: collector.names,
        static_refs,
        keywords: collector.keywords,
        class_ranges,
        ref_keys,
    }
}

fn to_fqn(name: &[u8]) -> String {
    let name = name.strip_prefix(b"\\").unwrap_or(name);
    String::from_utf8_lossy(name).into_owned()
}

/// PHPDoc wins when it names a class; otherwise the declared type, when it names
/// anything navigable.
pub(crate) fn pick_type(doc: Option<TypeExpr>, declared: Option<TypeExpr>) -> Option<TypeExpr> {
    match doc {
        Some(doc) if names_a_class(&doc) => Some(doc),
        _ => declared.filter(|ty| *ty != TypeExpr::Other),
    }
}

fn names_a_class(ty: &TypeExpr) -> bool {
    match ty {
        TypeExpr::Named(_) | TypeExpr::SelfType | TypeExpr::StaticType | TypeExpr::ParentType => {
            true
        }
        TypeExpr::ArrayOf(element) => names_a_class(element),
        TypeExpr::Union(members) => members.iter().any(names_a_class),
        TypeExpr::Other => false,
    }
}

/// A declaration found during the walk, before its names are resolved.
struct Pending {
    kind: SymbolKind,
    start: u32,
    end: u32,
    /// Class-likes: the whole declaration.
    range: Option<(u32, u32)>,
    /// The whole declaration, for every kind.
    span: (u32, u32),
    /// Set when the resolver does not know the name, as for `define()`.
    fqn: Option<String>,
    signature: String,
    doc: Option<String>,
    returns: Option<TypeExpr>,
    shape: Option<PendingShape>,
}

/// Parent, interfaces and traits as offsets of names the resolver resolved.
struct PendingShape {
    parent: Option<u32>,
    interfaces: Vec<u32>,
    traits: Vec<u32>,
    members: Vec<Member>,
    is_abstract: bool,
}

/// The class of a static access before names are resolved: the offset of a name the
/// resolver resolved, or a keyword.
enum PendingSel {
    Named(u32),
    Keyword(ClassSel),
}

/// Where a class-like declaration's parts are, as offsets.
struct ClassHead {
    kind: SymbolKind,
    is_abstract: bool,
    node_start: u32,
    node_end: u32,
    head_start: u32,
    name: (u32, u32),
    left_brace: u32,
    parent: Option<u32>,
    interfaces: Vec<u32>,
}

struct Collector<'a> {
    source: &'a [u8],
    program: &'a Program<'a>,
    names: FileNames,
    pending: Vec<Pending>,
    functions: HashSet<u32>,
    constants: HashSet<u32>,
    /// Namespace names, which the resolver records but which name no symbol.
    not_refs: HashSet<u32>,
    static_refs: Vec<(MemberKind, String, u32, u32, PendingSel)>,
    keywords: Vec<KeywordRef>,
    anonymous: Vec<PendingAnonymous>,
    /// Names of members accessed with `->` or `?->`.
    member_names: HashSet<String>,
}

/// An anonymous class: its range, the offset of its parent's name, and its members.
struct PendingAnonymous {
    start: u32,
    end: u32,
    parent: Option<u32>,
    members: Vec<Member>,
}

impl Collector<'_> {
    /// The class an expression before `::` or after `new` names, recording `self`,
    /// `static` and `parent` as keyword references. Dynamic classes give nothing.
    fn class_sel(&mut self, expression: &Expression<'_>) -> Option<PendingSel> {
        let (keyword, sel) = match expression {
            Expression::Identifier(identifier) => {
                return Some(PendingSel::Named(identifier.span().start.offset));
            }
            Expression::Self_(keyword) => (keyword, ClassSel::SelfKw),
            Expression::Static(keyword) => (keyword, ClassSel::StaticKw),
            Expression::Parent(keyword) => (keyword, ClassSel::ParentKw),
            _ => return None,
        };
        self.keywords.push(KeywordRef {
            start: keyword.span.start.offset,
            end: keyword.span.end.offset,
            class: sel.clone(),
        });
        Some(PendingSel::Keyword(sel))
    }

    fn static_member(
        &mut self,
        class: &Expression<'_>,
        kind: MemberKind,
        name: &[u8],
        start: u32,
        end: u32,
    ) {
        let Some(sel) = self.class_sel(class) else {
            return;
        };
        let name = String::from_utf8_lossy(name)
            .trim_start_matches('$')
            .to_string();
        if name.eq_ignore_ascii_case("class") && kind == MemberKind::Constant {
            return;
        }
        self.static_refs.push((kind, name, start, end, sel));
    }

    fn member_name(&mut self, selector: &ClassLikeMemberSelector<'_>) {
        if let ClassLikeMemberSelector::Identifier(name) = selector {
            self.member_names
                .insert(String::from_utf8_lossy(name.value).into_owned());
        }
    }

    fn method_selector(&mut self, class: &Expression<'_>, selector: &ClassLikeMemberSelector<'_>) {
        if let ClassLikeMemberSelector::Identifier(name) = selector {
            self.static_member(
                class,
                MemberKind::Method,
                name.value,
                name.span.start.offset,
                name.span.end.offset,
            );
        } else {
            self.class_sel(class);
        }
    }

    fn text(&self, start: u32, end: u32) -> String {
        let bytes = self
            .source
            .get(start as usize..end as usize)
            .unwrap_or_default();
        collapse(&String::from_utf8_lossy(bytes))
    }

    fn doc(&self, start: u32) -> Option<String> {
        get_docblock_before_position(self.program.trivia.as_slice(), start)
            .map(|trivia| String::from_utf8_lossy(trivia.value).into_owned())
    }

    fn hint_type(&self, hint: &Hint<'_>) -> TypeExpr {
        let span = hint.span();
        let text = self.text(span.start.offset, span.end.offset);
        parse_type(&text, &self.names.at(span.start.offset))
    }

    fn doc_tags(&self, doc: Option<&str>, offset: u32) -> crate::phpdoc::DocBlock {
        doc.map(|doc| parse_docblock(doc, &self.names.at(offset)))
            .unwrap_or_default()
    }

    fn class_like(&mut self, head: ClassHead, members: &Sequence<'_, ClassLikeMember<'_>>) {
        let (members, traits) = self.members(members, head.kind == SymbolKind::Interface);
        self.pending.push(Pending {
            kind: head.kind,
            start: head.name.0,
            end: head.name.1,
            range: Some((head.node_start, head.node_end)),
            span: (head.node_start, head.node_end),
            fqn: None,
            signature: self.text(head.head_start, head.left_brace),
            doc: self.doc(head.node_start),
            returns: None,
            shape: Some(PendingShape {
                parent: head.parent,
                interfaces: head.interfaces,
                traits,
                members,
                is_abstract: head.is_abstract || head.kind == SymbolKind::Interface,
            }),
        });
    }

    fn members(
        &self,
        members: &Sequence<'_, ClassLikeMember<'_>>,
        in_interface: bool,
    ) -> (Vec<Member>, Vec<u32>) {
        let mut out = Vec::new();
        let mut traits = Vec::new();
        for member in members.iter() {
            match member {
                ClassLikeMember::TraitUse(trait_use) => {
                    traits.extend(trait_use.trait_names.iter().map(|n| n.span().start.offset));
                }
                ClassLikeMember::Constant(constant) => {
                    let start = constant.span().start.offset;
                    let doc = self.doc(start);
                    for item in constant.items.iter() {
                        let name = String::from_utf8_lossy(item.name.value).into_owned();
                        out.push(Member {
                            kind: MemberKind::Constant,
                            signature: format!("const {name}"),
                            name,
                            is_static: true,
                            name_start: item.name.span.start.offset,
                            name_end: item.name.span.end.offset,
                            doc: doc.clone(),
                            ty: None,
                            params: Vec::new(),
                            is_abstract: false,
                            is_private: false,
                            range: (start, constant.span().end.offset),
                        });
                    }
                }
                ClassLikeMember::Property(property) => self.property(property, &mut out),
                ClassLikeMember::EnumCase(case) => {
                    let name = match &case.item {
                        EnumCaseItem::Unit(item) => &item.name,
                        EnumCaseItem::Backed(item) => &item.name,
                    };
                    let text = String::from_utf8_lossy(name.value).into_owned();
                    out.push(Member {
                        kind: MemberKind::EnumCase,
                        signature: format!("case {text}"),
                        name: text,
                        is_static: true,
                        name_start: name.span.start.offset,
                        name_end: name.span.end.offset,
                        doc: self.doc(case.span().start.offset),
                        ty: None,
                        params: Vec::new(),
                        is_abstract: false,
                        is_private: false,
                        range: (case.span().start.offset, case.span().end.offset),
                    });
                }
                ClassLikeMember::Method(method) => self.method(method, in_interface, &mut out),
            }
        }
        (out, traits)
    }

    fn property(&self, property: &Property<'_>, out: &mut Vec<Member>) {
        let (modifiers, hint, items): (_, _, Vec<&PropertyItem<'_>>) = match property {
            Property::Plain(plain) => (
                &plain.modifiers,
                plain.hint.as_ref(),
                plain.items.iter().collect(),
            ),
            Property::Hooked(hooked) => {
                (&hooked.modifiers, hooked.hint.as_ref(), vec![&hooked.item])
            }
        };
        let start = property.span().start.offset;
        let doc = self.doc(start);
        let tags = self.doc_tags(doc.as_deref(), start);
        let declared = hint.map(|h| self.hint_type(h));
        let first_item = items
            .first()
            .map(|item| variable_of(item).span.start.offset);
        let prefix = first_item.map_or_else(String::new, |end| {
            let head = modifiers
                .first()
                .map(|m| m.span().start.offset)
                .unwrap_or(start);
            self.text(head, end)
        });
        for item in items {
            let variable = variable_of(item);
            let name = String::from_utf8_lossy(variable.name)
                .trim_start_matches('$')
                .to_string();
            let doc_type = tags
                .var
                .iter()
                .find(|(var, _)| var.as_deref().is_none_or(|v| v == name))
                .map(|(_, ty)| ty.clone());
            out.push(Member {
                kind: MemberKind::Property,
                signature: format!("{prefix} ${name}").trim().to_string(),
                is_static: has_static(modifiers),
                name_start: variable.span.start.offset + 1,
                name_end: variable.span.end.offset,
                doc: doc.clone(),
                ty: pick_type(doc_type, declared.clone()),
                params: Vec::new(),
                is_abstract: false,
                is_private: has_private(modifiers),
                range: (start, property.span().end.offset),
                name,
            });
        }
    }

    fn method(&self, method: &Method<'_>, in_interface: bool, out: &mut Vec<Member>) {
        let start = method.span().start.offset;
        let doc = self.doc(start);
        let tags = self.doc_tags(doc.as_deref(), start);
        let head = method
            .modifiers
            .first()
            .map(|m| m.span().start.offset)
            .unwrap_or(method.function.span.start.offset);
        let end = head_end(&method.parameter_list, method.return_type_hint.as_ref());
        let params = self.params(&method.parameter_list, &tags);
        let name = String::from_utf8_lossy(method.name.value).into_owned();

        if name.eq_ignore_ascii_case("__construct") {
            for (param, (param_name, ty)) in method.parameter_list.parameters.iter().zip(&params) {
                if param.modifiers.is_empty() {
                    continue;
                }
                let head = param.span().start.offset;
                out.push(Member {
                    kind: MemberKind::Property,
                    name: param_name.clone(),
                    is_static: false,
                    name_start: param.variable.span.start.offset + 1,
                    name_end: param.variable.span.end.offset,
                    signature: self.text(head, param.variable.span.end.offset),
                    doc: None,
                    ty: ty.clone(),
                    params: Vec::new(),
                    is_abstract: false,
                    is_private: has_private(&param.modifiers),
                    range: (param.span().start.offset, param.span().end.offset),
                });
            }
        }

        out.push(Member {
            kind: MemberKind::Method,
            is_static: has_static(&method.modifiers),
            name_start: method.name.span.start.offset,
            name_end: method.name.span.end.offset,
            signature: self.text(head, end),
            ty: pick_type(
                tags.returns.clone(),
                method
                    .return_type_hint
                    .as_ref()
                    .map(|r| self.hint_type(&r.hint)),
            ),
            doc,
            params,
            is_abstract: in_interface || has_abstract(&method.modifiers),
            is_private: has_private(&method.modifiers),
            range: (start, method.span().end.offset),
            name,
        });
    }

    fn params(
        &self,
        list: &FunctionLikeParameterList<'_>,
        tags: &crate::phpdoc::DocBlock,
    ) -> Vec<(String, Option<TypeExpr>)> {
        list.parameters
            .iter()
            .map(|param| {
                let name = String::from_utf8_lossy(param.variable.name)
                    .trim_start_matches('$')
                    .to_string();
                let doc_type = tags
                    .params
                    .iter()
                    .find(|(n, _)| *n == name)
                    .map(|(_, ty)| ty.clone());
                let declared = param.hint.as_ref().map(|h| self.hint_type(h));
                let ty = pick_type(doc_type, declared);
                (name, ty)
            })
            .collect()
    }
}

fn variable_of<'a>(item: &'a PropertyItem<'_>) -> &'a mago_syntax::cst::DirectVariable<'a> {
    match item {
        PropertyItem::Abstract(item) => &item.variable,
        PropertyItem::Concrete(item) => &item.variable,
    }
}

fn has_private(modifiers: &Sequence<'_, Modifier<'_>>) -> bool {
    modifiers.iter().any(|m| matches!(m, Modifier::Private(_)))
}

fn has_abstract(modifiers: &Sequence<'_, Modifier<'_>>) -> bool {
    modifiers.iter().any(|m| matches!(m, Modifier::Abstract(_)))
}

fn has_static(modifiers: &Sequence<'_, Modifier<'_>>) -> bool {
    modifiers.iter().any(|m| matches!(m, Modifier::Static(_)))
}

/// Where a function-like head ends: after the return type, or after the parameters.
fn head_end(
    params: &FunctionLikeParameterList<'_>,
    returns: Option<&FunctionLikeReturnTypeHint<'_>>,
) -> u32 {
    returns
        .map(|r| r.span().end.offset)
        .unwrap_or(params.right_parenthesis.end.offset)
}

/// Collapses runs of whitespace to single spaces.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct SyntaxVisitor;

impl<'ast, 'arena> Walker<'ast, 'arena, Collector<'arena>> for SyntaxVisitor {
    fn walk_in_namespace(&self, namespace: &'ast Namespace<'arena>, c: &mut Collector<'arena>) {
        let span = namespace.span();
        let name = namespace
            .name
            .as_ref()
            .map(|name| to_fqn(name.value()))
            .unwrap_or_default();
        if let Some(name) = &namespace.name {
            c.not_refs.insert(name.span().start.offset);
        }
        c.names.scopes.push(NameScope {
            start: span.start.offset,
            end: span.end.offset,
            namespace: name,
            imports: HashMap::new(),
        });
    }

    fn walk_in_use(&self, r#use: &'ast Use<'arena>, c: &mut Collector<'arena>) {
        let offset = r#use.span().start.offset;
        let mut imports: Vec<(String, String)> = Vec::new();
        match &r#use.items {
            UseItems::Sequence(seq) => {
                for item in seq.items.iter() {
                    imports.push(import(
                        item.name.value(),
                        item.alias.as_ref().map(|a| a.identifier.value),
                        None,
                    ));
                }
            }
            UseItems::TypedSequence(seq) => {
                for item in seq.items.iter() {
                    mark_use_item(c, Some(&seq.r#type), item.name.span().start.offset);
                }
            }
            UseItems::TypedList(list) => {
                c.not_refs.insert(list.namespace.span().start.offset);
                for item in list.items.iter() {
                    mark_use_item(c, Some(&list.r#type), item.name.span().start.offset);
                }
            }
            UseItems::MixedList(list) => {
                c.not_refs.insert(list.namespace.span().start.offset);
                for mixed in list.items.iter() {
                    mark_use_item(
                        c,
                        mixed.r#type.as_ref(),
                        mixed.item.name.span().start.offset,
                    );
                    if mixed.r#type.is_none() {
                        imports.push(import(
                            mixed.item.name.value(),
                            mixed.item.alias.as_ref().map(|a| a.identifier.value),
                            Some(list.namespace.value()),
                        ));
                    }
                }
            }
        }
        if let Some(index) = c.names.scope_index(offset) {
            c.names.scopes[index].imports.extend(imports);
        }
    }

    fn walk_in_class(&self, class: &'ast Class<'arena>, c: &mut Collector<'arena>) {
        let head = class
            .modifiers
            .first()
            .map(|m| m.span().start.offset)
            .unwrap_or(class.class.span.start.offset);
        let parent = class
            .extends
            .as_ref()
            .and_then(|e| e.types.iter().next())
            .map(|t| t.span().start.offset);
        let interfaces = class
            .implements
            .as_ref()
            .map(|i| i.types.iter().map(|t| t.span().start.offset).collect())
            .unwrap_or_default();
        c.class_like(
            ClassHead {
                kind: SymbolKind::Class,
                is_abstract: has_abstract(&class.modifiers),
                node_start: class.span().start.offset,
                node_end: class.span().end.offset,
                head_start: head,
                name: (class.name.span.start.offset, class.name.span.end.offset),
                left_brace: class.left_brace.start.offset,
                parent,
                interfaces,
            },
            &class.members,
        );
    }

    fn walk_in_method_call(&self, call: &'ast MethodCall<'arena>, c: &mut Collector<'arena>) {
        c.member_name(&call.method);
    }

    fn walk_in_null_safe_method_call(
        &self,
        call: &'ast NullSafeMethodCall<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.member_name(&call.method);
    }

    fn walk_in_property_access(
        &self,
        access: &'ast PropertyAccess<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.member_name(&access.property);
    }

    fn walk_in_null_safe_property_access(
        &self,
        access: &'ast NullSafePropertyAccess<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.member_name(&access.property);
    }

    fn walk_in_anonymous_class(
        &self,
        class: &'ast AnonymousClass<'arena>,
        c: &mut Collector<'arena>,
    ) {
        let span = class.span();
        let (members, _) = c.members(&class.members, false);
        c.anonymous.push(PendingAnonymous {
            start: span.start.offset,
            end: span.end.offset,
            parent: class
                .extends
                .as_ref()
                .and_then(|e| e.types.iter().next())
                .map(|t| t.span().start.offset),
            members,
        });
    }

    fn walk_in_interface(&self, interface: &'ast Interface<'arena>, c: &mut Collector<'arena>) {
        let interfaces = interface
            .extends
            .as_ref()
            .map(|e| e.types.iter().map(|t| t.span().start.offset).collect())
            .unwrap_or_default();
        c.class_like(
            ClassHead {
                kind: SymbolKind::Interface,
                is_abstract: false,
                node_start: interface.span().start.offset,
                node_end: interface.span().end.offset,
                head_start: interface.interface.span.start.offset,
                name: (
                    interface.name.span.start.offset,
                    interface.name.span.end.offset,
                ),
                left_brace: interface.left_brace.start.offset,
                parent: None,
                interfaces,
            },
            &interface.members,
        );
    }

    fn walk_in_trait(&self, r#trait: &'ast Trait<'arena>, c: &mut Collector<'arena>) {
        c.class_like(
            ClassHead {
                kind: SymbolKind::Trait,
                is_abstract: false,
                node_start: r#trait.span().start.offset,
                node_end: r#trait.span().end.offset,
                head_start: r#trait.r#trait.span.start.offset,
                name: (r#trait.name.span.start.offset, r#trait.name.span.end.offset),
                left_brace: r#trait.left_brace.start.offset,
                parent: None,
                interfaces: Vec::new(),
            },
            &r#trait.members,
        );
    }

    fn walk_in_enum(&self, r#enum: &'ast Enum<'arena>, c: &mut Collector<'arena>) {
        let interfaces = r#enum
            .implements
            .as_ref()
            .map(|i| i.types.iter().map(|t| t.span().start.offset).collect())
            .unwrap_or_default();
        c.class_like(
            ClassHead {
                kind: SymbolKind::Enum,
                is_abstract: false,
                node_start: r#enum.span().start.offset,
                node_end: r#enum.span().end.offset,
                head_start: r#enum.r#enum.span.start.offset,
                name: (r#enum.name.span.start.offset, r#enum.name.span.end.offset),
                left_brace: r#enum.left_brace.start.offset,
                parent: None,
                interfaces,
            },
            &r#enum.members,
        );
    }

    fn walk_in_function(&self, function: &'ast Function<'arena>, c: &mut Collector<'arena>) {
        let start = function.span().start.offset;
        let doc = c.doc(start);
        let tags = c.doc_tags(doc.as_deref(), start);
        let end = head_end(&function.parameter_list, function.return_type_hint.as_ref());
        let returns = pick_type(
            tags.returns,
            function
                .return_type_hint
                .as_ref()
                .map(|r| c.hint_type(&r.hint)),
        );
        c.pending.push(Pending {
            kind: SymbolKind::Function,
            start: function.name.span.start.offset,
            end: function.name.span.end.offset,
            range: None,
            span: (start, function.span().end.offset),
            fqn: None,
            signature: c.text(function.function.span.start.offset, end),
            doc,
            returns,
            shape: None,
        });
    }

    fn walk_in_constant(&self, constant: &'ast Constant<'arena>, c: &mut Collector<'arena>) {
        let doc = c.doc(constant.span().start.offset);
        for item in constant.items.iter() {
            c.constants.insert(item.name.span.start.offset);
            c.pending.push(Pending {
                kind: SymbolKind::Constant,
                start: item.name.span.start.offset,
                end: item.name.span.end.offset,
                range: None,
                span: (constant.span().start.offset, constant.span().end.offset),
                fqn: None,
                signature: format!("const {}", String::from_utf8_lossy(item.name.value)),
                doc: doc.clone(),
                returns: None,
                shape: None,
            });
        }
    }

    fn walk_in_function_call(&self, call: &'ast FunctionCall<'arena>, c: &mut Collector<'arena>) {
        let Expression::Identifier(identifier) = call.function else {
            return;
        };
        c.functions.insert(identifier.span().start.offset);
        if !identifier.value().eq_ignore_ascii_case(b"define") {
            return;
        }
        let first = call.argument_list.arguments.iter().next();
        let Some(Argument::Positional(argument)) = first else {
            return;
        };
        let Expression::Literal(Literal::String(literal)) = argument.value else {
            return;
        };
        let Some(value) = literal.value else {
            return;
        };
        let name = String::from_utf8_lossy(value).into_owned();
        c.pending.push(Pending {
            kind: SymbolKind::Constant,
            range: None,
            span: (call.span().start.offset, call.span().end.offset),
            start: literal.span.start.offset + 1,
            end: literal.span.end.offset.saturating_sub(1),
            signature: format!("define('{name}')"),
            fqn: Some(name.trim_start_matches('\\').to_string()),
            doc: c.doc(call.span().start.offset),
            returns: None,
            shape: None,
        });
    }

    fn walk_in_function_partial_application(
        &self,
        application: &'ast FunctionPartialApplication<'arena>,
        c: &mut Collector<'arena>,
    ) {
        if let Expression::Identifier(identifier) = application.function {
            c.functions.insert(identifier.span().start.offset);
        }
    }

    fn walk_in_static_method_call(
        &self,
        call: &'ast StaticMethodCall<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.method_selector(call.class, &call.method);
    }

    fn walk_in_static_method_partial_application(
        &self,
        application: &'ast StaticMethodPartialApplication<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.method_selector(application.class, &application.method);
    }

    fn walk_in_class_constant_access(
        &self,
        access: &'ast ClassConstantAccess<'arena>,
        c: &mut Collector<'arena>,
    ) {
        if let ClassLikeConstantSelector::Identifier(name) = &access.constant {
            c.static_member(
                access.class,
                MemberKind::Constant,
                name.value,
                name.span.start.offset,
                name.span.end.offset,
            );
        } else {
            c.class_sel(access.class);
        }
    }

    fn walk_in_static_property_access(
        &self,
        access: &'ast StaticPropertyAccess<'arena>,
        c: &mut Collector<'arena>,
    ) {
        if let Variable::Direct(variable) = &access.property {
            c.static_member(
                access.class,
                MemberKind::Property,
                variable.name,
                variable.span.start.offset,
                variable.span.end.offset,
            );
        } else {
            c.class_sel(access.class);
        }
    }

    fn walk_in_instantiation(
        &self,
        instantiation: &'ast Instantiation<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.class_sel(instantiation.class);
    }

    fn walk_in_hint(&self, hint: &'ast Hint<'arena>, c: &mut Collector<'arena>) {
        let (keyword, class) = match hint {
            Hint::Self_(keyword) => (keyword, ClassSel::SelfKw),
            Hint::Static(keyword) => (keyword, ClassSel::StaticKw),
            Hint::Parent(keyword) => (keyword, ClassSel::ParentKw),
            _ => return,
        };
        c.keywords.push(KeywordRef {
            start: keyword.span.start.offset,
            end: keyword.span.end.offset,
            class,
        });
    }

    fn walk_in_constant_access(
        &self,
        access: &'ast ConstantAccess<'arena>,
        c: &mut Collector<'arena>,
    ) {
        c.constants.insert(access.name.span().start.offset);
    }
}

/// An import as (lowercased alias, fully qualified name).
fn import(name: &[u8], alias: Option<&[u8]>, prefix: Option<&[u8]>) -> (String, String) {
    let name = to_fqn(name);
    let fqn = match prefix {
        Some(prefix) => format!("{}\\{name}", to_fqn(prefix)),
        None => name,
    };
    let alias = match alias {
        Some(alias) => String::from_utf8_lossy(alias).into_owned(),
        None => fqn.rsplit('\\').next().unwrap_or(&fqn).to_string(),
    };
    (alias.to_ascii_lowercase(), fqn)
}

fn mark_use_item(c: &mut Collector<'_>, use_type: Option<&UseType<'_>>, start: u32) {
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
