//! Type inference for the expression under the cursor.
//!
//! One walk of the cursor file collects the facts inference needs, as references
//! into the syntax tree: member accesses, function scopes, control-flow branches,
//! assignments and other rebindings, `foreach` and `catch` bindings, `instanceof`
//! checks and inline `@var` docblocks. Types are inferred on demand, before the
//! tree's arena is dropped.
//!
//! A variable's facts count only in the function, closure or arrow function that
//! holds them. Among them the latest before the use decides, except that a binding
//! inside a branch the use is not in either cannot reach it (a sibling branch) or
//! merges with what came before (code after the branch statement). A binding whose
//! type is unknown makes the merged type unknown: the result is then nothing rather
//! than a guess.

use std::cell::RefCell;
use std::collections::HashMap;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::comments::docblock::get_docblock_before_position;
use mago_syntax::cst::{
    Access, AnonymousClass, Argument, ArgumentList, ArrayElement, ArrowFunction, Assignment,
    BinaryOperator, Call, ClassLikeConstantSelector, ClassLikeMemberSelector, Closure, DoWhile,
    Expression, For, Foreach, ForeachTarget, Function, FunctionLikeParameterList, Global, Hint, If,
    IfBody, Literal, Method, MethodCall, NullSafeMethodCall, NullSafePropertyAccess, Program,
    PropertyAccess, Static, StaticItem, Switch, TriviaKind, Try, TryCatchClause, Variable, While,
};
use mago_syntax::parser::parse_file_content;
use mago_syntax::walker::Walker;

use crate::frameworks::{CallTarget, FrameworkCall};
use crate::index::Index;
use crate::phpdoc::{TypeExpr, parse_docblock, parse_type};
use crate::summary::{FileSummary, MemberKind, NameKind, NameRef, SymbolKind, pick_type};

const MAX_DEPTH: u32 = 16;

/// What a member access's receiver is.
pub(crate) enum Receiver {
    /// The classes the receiver can be; empty when its type is unknown.
    Classes(Vec<String>),
    /// `$this` inside the anonymous class at this index of the summary's class ranges.
    Anonymous(usize),
}

/// The receiver, kind and name of the instance member access under `offset`, when
/// the cursor is on the member name of `$x->name` or `$x?->name()`.
pub(crate) fn member_at(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
    offset: u32,
) -> Option<(Receiver, MemberKind, String)> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    let access = facts
        .accesses
        .iter()
        .find(|a| a.start <= offset && offset <= a.end)?;
    let ctx = Ctx::new(index, summary, program, source, &facts);
    let receiver = match ctx.anonymous_this(access.object, access.start) {
        Some(range) => Receiver::Anonymous(range),
        None => Receiver::Classes(class_names(&ctx.expr(access.object, access.start, 0))),
    };
    Some((receiver, access.kind, access.name.clone()))
}

/// The variable whose name covers `offset` and its inferred type.
pub(crate) fn variable_at(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
    offset: u32,
) -> Option<(String, TypeExpr)> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    let (name, start) = facts
        .variables
        .iter()
        .find(|(_, start, end)| *start <= offset && offset <= *end)
        .map(|(name, start, _)| (name.clone(), *start))?;
    let ctx = Ctx::new(index, summary, program, source, &facts);
    let ty = ctx.var(&name, start, 0);
    Some((name, ty))
}

/// Every instance member access named `name` in the file, resolved with one parse:
/// start and end of the member name, receiver, and kind.
pub(crate) fn member_accesses_named(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
    name: &str,
) -> Vec<(u32, u32, Receiver, MemberKind)> {
    accesses(index, summary, source, Some(name))
        .into_iter()
        .map(|(start, end, _, receiver, kind)| (start, end, receiver, kind))
        .collect()
}

/// Every instance member access in the file, resolved with one parse: start and
/// end of the member name, the name, receiver, and kind.
pub(crate) fn member_accesses(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
) -> Vec<(u32, u32, String, Receiver, MemberKind)> {
    accesses(index, summary, source, None)
}

fn accesses(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
    name: Option<&str>,
) -> Vec<(u32, u32, String, Receiver, MemberKind)> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    let ctx = Ctx::new(index, summary, program, source, &facts);
    facts
        .accesses
        .iter()
        .filter(|a| name.is_none_or(|name| crate::references::same_name(&a.name, name, a.kind)))
        .map(|access| {
            let receiver = match ctx.anonymous_this(access.object, access.start) {
                Some(range) => Receiver::Anonymous(range),
                None => Receiver::Classes(class_names(&ctx.expr(access.object, access.start, 0))),
            };
            (
                access.start,
                access.end,
                access.name.clone(),
                receiver,
                access.kind,
            )
        })
        .collect()
}

/// Every instance member access in the file: start and end of the name, and kind.
pub(crate) fn member_access_spans(source: &[u8]) -> Vec<(u32, u32, MemberKind)> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    facts
        .accesses
        .iter()
        .map(|a| (a.start, a.end, a.kind))
        .collect()
}

/// The start offsets of every instance member name accessed in the file.
pub(crate) fn member_access_starts(source: &[u8]) -> Vec<u32> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    facts.accesses.iter().map(|a| a.start).collect()
}

struct AccessFact<'arena> {
    start: u32,
    end: u32,
    kind: MemberKind,
    name: String,
    object: &'arena Expression<'arena>,
}

enum ScopeKind {
    Function,
    /// A closure sees the variables of its `use` clause from the enclosing scope.
    Closure(Vec<String>),
    /// An arrow function sees every variable of the enclosing scope.
    Arrow,
}

struct ScopeFact<'ast, 'arena> {
    start: u32,
    end: u32,
    kind: ScopeKind,
    params: &'ast FunctionLikeParameterList<'arena>,
}

/// `$var = rhs`, or with no `rhs` a rebinding to an unknown value: destructuring,
/// `global`, `static`.
struct AssignFact<'arena> {
    start: u32,
    end: u32,
    var: String,
    rhs: Option<&'arena Expression<'arena>>,
}

/// A `foreach`: its value variable takes the iterable's element type; its key and
/// destructured variables become unknown.
struct ForeachFact<'arena> {
    start: u32,
    body_start: u32,
    value: Option<String>,
    unknown: Vec<String>,
    iterable: &'arena Expression<'arena>,
}

struct CatchFact<'ast, 'arena> {
    start: u32,
    var: String,
    hint: &'ast Hint<'arena>,
}

/// `$var instanceof Class` holds between `start` and `end`; `class` is the offset of
/// the class name, which the name resolver resolved.
struct NarrowFact {
    start: u32,
    end: u32,
    var: String,
    class: u32,
}

/// One branch of a control-flow statement, and where that statement ends. Loops,
/// `switch` and anonymous class bodies count as a single branch spanning the whole
/// statement.
struct BranchFact {
    start: u32,
    end: u32,
    statement_end: u32,
}

#[derive(Default)]
struct Facts<'ast, 'arena> {
    accesses: Vec<AccessFact<'arena>>,
    scopes: Vec<ScopeFact<'ast, 'arena>>,
    assignments: Vec<AssignFact<'arena>>,
    foreaches: Vec<ForeachFact<'arena>>,
    catches: Vec<CatchFact<'ast, 'arena>>,
    narrowings: Vec<NarrowFact>,
    branches: Vec<BranchFact>,
    /// Every direct variable: name without `$`, start, end.
    variables: Vec<(String, u32, u32)>,
}

impl<'ast, 'arena> Facts<'ast, 'arena> {
    fn access(
        &mut self,
        object: &'arena Expression<'arena>,
        selector: &ClassLikeMemberSelector<'_>,
        kind: MemberKind,
    ) {
        if let ClassLikeMemberSelector::Identifier(name) = selector {
            self.accesses.push(AccessFact {
                start: name.span.start.offset,
                end: name.span.end.offset,
                kind,
                name: String::from_utf8_lossy(name.value).into_owned(),
                object,
            });
        }
    }

    fn scope(
        &mut self,
        node: &impl HasSpan,
        kind: ScopeKind,
        params: &'ast FunctionLikeParameterList<'arena>,
    ) {
        let span = node.span();
        self.scopes.push(ScopeFact {
            start: span.start.offset,
            end: span.end.offset,
            kind,
            params,
        });
    }

    fn branch(&mut self, node: &impl HasSpan, statement_end: u32) {
        let span = node.span();
        self.branches.push(BranchFact {
            start: span.start.offset,
            end: span.end.offset,
            statement_end,
        });
    }

    /// A whole statement as its own single branch.
    fn whole(&mut self, node: &impl HasSpan) {
        let end = node.span().end.offset;
        self.branch(node, end);
    }

    fn unknown(&mut self, start: u32, end: u32, var: String) {
        self.assignments.push(AssignFact {
            start,
            end,
            var,
            rhs: None,
        });
    }
}

struct Collect;

impl<'ast, 'arena> Walker<'ast, 'arena, Facts<'ast, 'arena>> for Collect {
    fn walk_in_method_call(&self, call: &'ast MethodCall<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.access(call.object, &call.method, MemberKind::Method);
    }

    fn walk_in_null_safe_method_call(
        &self,
        call: &'ast NullSafeMethodCall<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        f.access(call.object, &call.method, MemberKind::Method);
    }

    fn walk_in_property_access(
        &self,
        access: &'ast PropertyAccess<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        f.access(access.object, &access.property, MemberKind::Property);
    }

    fn walk_in_null_safe_property_access(
        &self,
        access: &'ast NullSafePropertyAccess<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        f.access(access.object, &access.property, MemberKind::Property);
    }

    fn walk_in_method(&self, method: &'ast Method<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.scope(method, ScopeKind::Function, &method.parameter_list);
    }

    fn walk_in_function(&self, function: &'ast Function<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.scope(function, ScopeKind::Function, &function.parameter_list);
    }

    fn walk_in_closure(&self, closure: &'ast Closure<'arena>, f: &mut Facts<'ast, 'arena>) {
        let uses = closure
            .use_clause
            .as_ref()
            .map(|clause| {
                clause
                    .variables
                    .iter()
                    .map(|v| variable_name(v.variable.name))
                    .collect()
            })
            .unwrap_or_default();
        f.scope(closure, ScopeKind::Closure(uses), &closure.parameter_list);
    }

    fn walk_in_arrow_function(
        &self,
        arrow: &'ast ArrowFunction<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        f.scope(arrow, ScopeKind::Arrow, &arrow.parameter_list);
    }

    fn walk_in_anonymous_class(
        &self,
        class: &'ast AnonymousClass<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        f.whole(class);
    }

    fn walk_in_assignment(
        &self,
        assignment: &'ast Assignment<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        if !assignment.operator.is_assign() {
            return;
        }
        let span = assignment.span();
        let (start, end) = (span.start.offset, span.end.offset);
        match assignment.lhs {
            Expression::Variable(Variable::Direct(variable)) => {
                f.assignments.push(AssignFact {
                    start,
                    end,
                    var: variable_name(variable.name),
                    rhs: Some(assignment.rhs),
                });
            }
            destructuring => {
                for var in destructured(destructuring) {
                    f.unknown(start, end, var);
                }
            }
        }
    }

    fn walk_in_global(&self, global: &'ast Global<'arena>, f: &mut Facts<'ast, 'arena>) {
        let span = global.span();
        for variable in global.variables.iter() {
            if let Variable::Direct(direct) = variable {
                f.unknown(
                    span.start.offset,
                    span.end.offset,
                    variable_name(direct.name),
                );
            }
        }
    }

    fn walk_in_static(&self, r#static: &'ast Static<'arena>, f: &mut Facts<'ast, 'arena>) {
        let span = r#static.span();
        for item in r#static.items.iter() {
            let variable = match item {
                StaticItem::Abstract(item) => &item.variable,
                StaticItem::Concrete(item) => &item.variable,
            };
            f.unknown(
                span.start.offset,
                span.end.offset,
                variable_name(variable.name),
            );
        }
    }

    fn walk_in_foreach(&self, foreach: &'ast Foreach<'arena>, f: &mut Facts<'ast, 'arena>) {
        let (key, value) = match &foreach.target {
            ForeachTarget::Value(target) => (None, target.value),
            ForeachTarget::KeyValue(target) => (Some(target.key), target.value),
        };
        let mut unknown: Vec<String> = key.into_iter().flat_map(destructured).collect();
        let value = match value {
            Expression::Variable(Variable::Direct(variable)) => Some(variable_name(variable.name)),
            destructuring => {
                unknown.extend(destructured(destructuring));
                None
            }
        };
        f.foreaches.push(ForeachFact {
            start: foreach.span().start.offset,
            body_start: foreach.right_parenthesis.end.offset,
            value,
            unknown,
            iterable: foreach.expression,
        });
        f.whole(foreach);
    }

    fn walk_in_for(&self, r#for: &'ast For<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.whole(r#for);
    }

    fn walk_in_while(&self, r#while: &'ast While<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.whole(r#while);
    }

    fn walk_in_do_while(&self, do_while: &'ast DoWhile<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.whole(do_while);
    }

    fn walk_in_switch(&self, switch: &'ast Switch<'arena>, f: &mut Facts<'ast, 'arena>) {
        f.whole(switch);
    }

    fn walk_in_try(&self, r#try: &'ast Try<'arena>, f: &mut Facts<'ast, 'arena>) {
        let end = r#try.span().end.offset;
        f.branch(&r#try.block, end);
        for clause in r#try.catch_clauses.iter() {
            f.branch(&clause.block, end);
        }
        if let Some(finally) = &r#try.finally_clause {
            f.branch(&finally.block, end);
        }
    }

    fn walk_in_try_catch_clause(
        &self,
        clause: &'ast TryCatchClause<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        if let Some(variable) = &clause.variable {
            f.catches.push(CatchFact {
                start: clause.block.span().start.offset,
                var: variable_name(variable.name),
                hint: &clause.hint,
            });
        }
    }

    fn walk_in_if(&self, r#if: &'ast If<'arena>, f: &mut Facts<'ast, 'arena>) {
        let end = r#if.span().end.offset;
        let then = match &r#if.body {
            IfBody::Statement(body) => {
                f.branch(body.statement, end);
                for clause in body.else_if_clauses.iter() {
                    f.branch(clause.statement, end);
                }
                if let Some(clause) = &body.else_clause {
                    f.branch(clause.statement, end);
                }
                let span = body.statement.span();
                (span.start.offset, span.end.offset)
            }
            IfBody::ColonDelimited(body) => {
                f.whole(r#if);
                let start = body.colon.end.offset;
                let end = body
                    .statements
                    .iter()
                    .last()
                    .map_or(start, |s| s.span().end.offset);
                (start, end)
            }
        };
        let mut checks = Vec::new();
        instanceof_checks(r#if.condition, &mut checks);
        for (var, class) in checks {
            f.narrowings.push(NarrowFact {
                start: then.0,
                end: then.1,
                var,
                class,
            });
        }
    }

    fn walk_in_variable(&self, variable: &'ast Variable<'arena>, f: &mut Facts<'ast, 'arena>) {
        if let Variable::Direct(direct) = variable {
            f.variables.push((
                variable_name(direct.name),
                direct.span.start.offset,
                direct.span.end.offset,
            ));
        }
    }
}

/// The direct variables a destructuring target (`[$a, $b]`, `list($a)`, nested)
/// assigns, or a lone variable.
fn destructured(target: &Expression<'_>) -> Vec<String> {
    let elements = match target {
        Expression::Variable(Variable::Direct(variable)) => {
            return vec![variable_name(variable.name)];
        }
        Expression::Array(array) => &array.elements,
        Expression::List(list) => &list.elements,
        Expression::LegacyArray(array) => &array.elements,
        _ => return Vec::new(),
    };
    elements
        .iter()
        .flat_map(|element| match element {
            ArrayElement::KeyValue(element) => destructured(element.value),
            ArrayElement::Value(element) => destructured(element.value),
            _ => Vec::new(),
        })
        .collect()
}

/// `$v instanceof C` checks that hold when `condition` is true: alone, inside
/// parentheses, or in an `&&` chain.
fn instanceof_checks(condition: &Expression<'_>, out: &mut Vec<(String, u32)>) {
    match condition {
        Expression::Parenthesized(inner) => instanceof_checks(inner.expression, out),
        Expression::Binary(binary) => match binary.operator {
            BinaryOperator::Instanceof(_) => {
                if let (
                    Expression::Variable(Variable::Direct(variable)),
                    Expression::Identifier(class),
                ) = (binary.lhs, binary.rhs)
                {
                    out.push((variable_name(variable.name), class.span().start.offset));
                }
            }
            BinaryOperator::And(_) | BinaryOperator::LowAnd(_) => {
                instanceof_checks(binary.lhs, out);
                instanceof_checks(binary.rhs, out);
            }
            _ => {}
        },
        _ => {}
    }
}

fn variable_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name)
        .trim_start_matches('$')
        .to_string()
}

/// How a binding affects a use of the variable.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The binding is certainly in effect.
    Replace,
    /// The binding may or may not have happened: merge it with what came before.
    Merge,
    /// The binding cannot reach the use.
    Ignore,
}

/// A source of a variable's type.
enum Binding<'f, 'ast, 'arena> {
    Param(&'f ScopeFact<'ast, 'arena>, usize),
    Assign(&'f AssignFact<'arena>),
    ForeachValue(&'f ForeachFact<'arena>),
    Unknown,
    Catch(&'f CatchFact<'ast, 'arena>),
    Narrow(u32),
    Doc(TypeExpr),
}

/// A variable's type while folding its bindings in order.
enum State {
    Unset,
    Null,
    Known(TypeExpr),
    Unknown,
}

impl State {
    fn of(ty: TypeExpr) -> State {
        match ty {
            TypeExpr::Other => State::Unknown,
            ty => State::Known(ty),
        }
    }

    fn merge(self, other: State) -> State {
        match (self, other) {
            (State::Unknown, _) | (_, State::Unknown) => State::Unknown,
            (State::Unset | State::Null, other) => other,
            (known, State::Unset | State::Null) => known,
            (State::Known(a), State::Known(b)) => State::Known(union(vec![a, b])),
        }
    }

    fn into_type(self) -> TypeExpr {
        match self {
            State::Known(ty) => ty,
            _ => TypeExpr::Other,
        }
    }
}

struct Ctx<'a, 'ast, 'arena> {
    index: &'a Index,
    summary: &'a FileSummary,
    program: &'arena Program<'arena>,
    source: &'a [u8],
    facts: &'a Facts<'ast, 'arena>,
    names: HashMap<u32, &'a NameRef>,
    /// Inline `@var Type $name` docblocks: position, variable, type.
    var_docs: Vec<(u32, String, TypeExpr)>,
    /// Variable types already inferred, by name and position.
    memo: RefCell<HashMap<(String, u32), TypeExpr>>,
}

impl<'a, 'ast, 'arena> Ctx<'a, 'ast, 'arena> {
    fn new(
        index: &'a Index,
        summary: &'a FileSummary,
        program: &'arena Program<'arena>,
        source: &'a [u8],
        facts: &'a Facts<'ast, 'arena>,
    ) -> Self {
        let names = summary.names.iter().map(|n| (n.start, n)).collect();
        let var_docs = program
            .trivia
            .iter()
            .filter(|t| t.kind == TriviaKind::DocBlockComment)
            .flat_map(|t| {
                let start = t.span.start.offset;
                let doc = parse_docblock(
                    &String::from_utf8_lossy(t.value),
                    &summary.file_names.at(start),
                );
                doc.var
                    .into_iter()
                    .filter_map(move |(var, ty)| Some((start, var?, ty)))
            })
            .collect();
        Ctx {
            index,
            summary,
            program,
            source,
            facts,
            names,
            var_docs,
            memo: RefCell::new(HashMap::new()),
        }
    }

    fn text(&self, start: u32, end: u32) -> String {
        String::from_utf8_lossy(
            self.source
                .get(start as usize..end as usize)
                .unwrap_or_default(),
        )
        .into_owned()
    }

    fn hint_type(&self, hint: &Hint<'_>) -> TypeExpr {
        let span = hint.span();
        parse_type(
            &self.text(span.start.offset, span.end.offset),
            &self.summary.file_names.at(span.start.offset),
        )
    }

    /// The named class enclosing `at`; nothing inside an anonymous class.
    fn enclosing(&self, at: u32) -> Option<String> {
        self.summary.enclosing_class(at).and_then(|c| c.fqn.clone())
    }

    fn enclosing_parent(&self, at: u32) -> Option<String> {
        self.summary.enclosing_class(at)?.parent.clone()
    }

    /// The index of the anonymous class enclosing `at` when `object` is `$this`.
    fn anonymous_this(&self, object: &Expression<'_>, at: u32) -> Option<usize> {
        let Expression::Variable(Variable::Direct(variable)) = object else {
            return None;
        };
        if variable.name != b"$this" {
            return None;
        }
        let (i, range) = self
            .summary
            .class_ranges
            .iter()
            .enumerate()
            .filter(|(_, r)| r.start <= at && at <= r.end)
            .max_by_key(|(_, r)| r.start)?;
        range.fqn.is_none().then_some(i)
    }

    /// The class an expression before `::` or after `new` names.
    fn class_of(&self, expression: &Expression<'_>, at: u32) -> Option<String> {
        match expression {
            Expression::Identifier(identifier) => self
                .names
                .get(&identifier.span().start.offset)
                .filter(|n| n.kind == NameKind::ClassLike)
                .map(|n| n.fqn.clone()),
            Expression::Self_(_) | Expression::Static(_) => self.enclosing(at),
            Expression::Parent(_) => self.enclosing_parent(at),
            _ => None,
        }
    }

    fn expr(&self, expression: &Expression<'_>, at: u32, depth: u32) -> TypeExpr {
        if depth > MAX_DEPTH {
            return TypeExpr::Other;
        }
        let depth = depth + 1;
        match expression {
            Expression::Variable(Variable::Direct(variable)) => self.var(
                &variable_name(variable.name),
                variable.span.start.offset,
                depth,
            ),
            Expression::Parenthesized(inner) => self.expr(inner.expression, at, depth),
            Expression::Clone(clone) => self.expr(clone.object, at, depth),
            Expression::Assignment(assignment) => self.expr(assignment.rhs, at, depth),
            Expression::Instantiation(instantiation) => self
                .class_of(instantiation.class, at)
                .map_or(TypeExpr::Other, TypeExpr::Named),
            Expression::Binary(binary) if binary.operator.is_null_coalesce() => union(vec![
                self.expr(binary.lhs, at, depth),
                self.expr(binary.rhs, at, depth),
            ]),
            Expression::Conditional(conditional) => union(vec![
                self.expr(conditional.then.unwrap_or(conditional.condition), at, depth),
                self.expr(conditional.r#else, at, depth),
            ]),
            Expression::Call(Call::Method(call)) => {
                let receiver = self.expr(call.object, at, depth);
                self.framework_type(&receiver, &call.method, &call.argument_list, at)
                    .unwrap_or_else(|| {
                        self.member_on(&receiver, &call.method, MemberKind::Method, None)
                    })
            }
            Expression::Call(Call::NullSafeMethod(call)) => {
                let receiver = self.expr(call.object, at, depth);
                self.framework_type(&receiver, &call.method, &call.argument_list, at)
                    .unwrap_or_else(|| {
                        self.member_on(&receiver, &call.method, MemberKind::Method, None)
                    })
            }
            Expression::Call(Call::StaticMethod(call)) => {
                let Some(class) = self.class_of(call.class, at) else {
                    return TypeExpr::Other;
                };
                // `self::`, `static::` and `parent::` forward late static binding to
                // the calling class.
                let forwarded = match call.class {
                    Expression::Self_(_) | Expression::Static(_) | Expression::Parent(_) => {
                        self.enclosing(at)
                    }
                    _ => None,
                };
                let receiver = TypeExpr::Named(class);
                self.framework_type(&receiver, &call.method, &call.argument_list, at)
                    .unwrap_or_else(|| {
                        self.member_on(
                            &receiver,
                            &call.method,
                            MemberKind::Method,
                            forwarded.as_deref(),
                        )
                    })
            }
            Expression::Call(Call::Function(call)) => self.function_returns(call.function),
            Expression::Access(Access::Property(access)) => self.member_on(
                &self.expr(access.object, at, depth),
                &access.property,
                MemberKind::Property,
                None,
            ),
            Expression::Access(Access::NullSafeProperty(access)) => self.member_on(
                &self.expr(access.object, at, depth),
                &access.property,
                MemberKind::Property,
                None,
            ),
            Expression::Access(Access::StaticProperty(access)) => {
                match (self.class_of(access.class, at), &access.property) {
                    (Some(class), Variable::Direct(variable)) => self.member_type(
                        &TypeExpr::Named(class),
                        MemberKind::Property,
                        &variable_name(variable.name),
                        None,
                    ),
                    _ => TypeExpr::Other,
                }
            }
            _ => TypeExpr::Other,
        }
    }

    /// The type the active frameworks give a method call on `receiver`, when one of
    /// them knows better than the method's declaration.
    fn framework_type(
        &self,
        receiver: &TypeExpr,
        selector: &ClassLikeMemberSelector<'_>,
        arguments: &ArgumentList<'_>,
        at: u32,
    ) -> Option<TypeExpr> {
        if self.index.frameworks().is_empty() {
            return None;
        }
        let ClassLikeMemberSelector::Identifier(name) = selector else {
            return None;
        };
        let name = String::from_utf8_lossy(name.value);
        let targets: Vec<CallTarget> = class_names(receiver)
            .iter()
            .flat_map(|class| self.index.find_member(class, MemberKind::Method, &name))
            .map(|(declaring, member)| CallTarget::Method {
                class: declaring.declaration.fqn.clone(),
                name: member.name.clone(),
            })
            .collect();
        if targets.is_empty() {
            return None;
        }
        let strings = self.string_arguments(arguments, at);
        let call = FrameworkCall {
            targets: &targets,
            strings: &strings,
        };
        self.index
            .frameworks()
            .iter()
            .find_map(|framework| framework.call_type(self.index, &call))
    }

    /// Positional arguments that are string literals or `X::class`, by position.
    fn string_arguments(&self, arguments: &ArgumentList<'_>, at: u32) -> Vec<Option<String>> {
        arguments
            .arguments
            .iter()
            .map(|argument| match argument {
                Argument::Positional(argument) => self.string_value(argument.value, at),
                Argument::Named(_) => None,
            })
            .collect()
    }

    fn string_value(&self, expression: &Expression<'_>, at: u32) -> Option<String> {
        match expression {
            Expression::Literal(Literal::String(literal)) => literal
                .value
                .map(|v| String::from_utf8_lossy(v).into_owned()),
            Expression::Access(Access::ClassConstant(access)) => match &access.constant {
                ClassLikeConstantSelector::Identifier(name)
                    if name.value.eq_ignore_ascii_case(b"class") =>
                {
                    self.class_of(access.class, at)
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn member_on(
        &self,
        receiver: &TypeExpr,
        selector: &ClassLikeMemberSelector<'_>,
        kind: MemberKind,
        static_class: Option<&str>,
    ) -> TypeExpr {
        match selector {
            ClassLikeMemberSelector::Identifier(name) => self.member_type(
                receiver,
                kind,
                &String::from_utf8_lossy(name.value),
                static_class,
            ),
            _ => TypeExpr::Other,
        }
    }

    /// The type of member `name` on every class of `receiver`. `static` and `$this`
    /// become the receiver class (or `static_class` when late static binding is
    /// forwarded), `self` the declaring class (the using class for a trait), and
    /// `parent` the declaring class's parent.
    fn member_type(
        &self,
        receiver: &TypeExpr,
        kind: MemberKind,
        name: &str,
        static_class: Option<&str>,
    ) -> TypeExpr {
        let mut types = Vec::new();
        for class in class_names(receiver) {
            let static_class = static_class.unwrap_or(&class);
            for (found, member) in self.index.find_member(&class, kind, name) {
                let typed = match &member.ty {
                    Some(_) => Some((found, member)),
                    None => self
                        .index
                        .typed_ancestor(&found.declaration.fqn, kind, name),
                };
                let Some((declaring, member)) = typed else {
                    continue;
                };
                let Some(ty) = &member.ty else {
                    continue;
                };
                let declaration = declaring.declaration;
                let self_class = if declaration.kind == SymbolKind::Trait {
                    class.as_str()
                } else {
                    declaration.fqn.as_str()
                };
                let parent = declaration.class.as_ref().and_then(|s| s.parent.as_deref());
                types.push(bind(ty, static_class, self_class, parent));
            }
        }
        union(types)
    }

    fn function_returns(&self, function: &Expression<'_>) -> TypeExpr {
        let Expression::Identifier(identifier) = function else {
            return TypeExpr::Other;
        };
        let Some(name) = self
            .names
            .get(&identifier.span().start.offset)
            .filter(|n| n.kind == NameKind::Function)
        else {
            return TypeExpr::Other;
        };
        union(self.index.function_returns(name))
    }

    /// The innermost function-like scope containing `at`, as an index into the scopes.
    fn scope_at(&self, at: u32) -> Option<usize> {
        self.facts
            .scopes
            .iter()
            .enumerate()
            .filter(|(_, s)| s.start <= at && at <= s.end)
            .max_by_key(|(_, s)| s.start)
            .map(|(i, _)| i)
    }

    /// How a binding at `position` affects a use at `at`: decided by the innermost
    /// branch that holds the binding but not the use.
    fn mode(&self, position: u32, at: u32) -> Mode {
        let branch = self
            .facts
            .branches
            .iter()
            .filter(|b| b.start <= position && position <= b.end)
            .filter(|b| !(b.start <= at && at <= b.end))
            .max_by_key(|b| b.start);
        match branch {
            None => Mode::Replace,
            Some(branch) if at <= branch.statement_end => Mode::Ignore,
            Some(_) => Mode::Merge,
        }
    }

    /// The type of variable `name` used at `at`.
    fn var(&self, name: &str, at: u32, depth: u32) -> TypeExpr {
        if depth > MAX_DEPTH {
            return TypeExpr::Other;
        }
        if name == "this" {
            return self.enclosing(at).map_or(TypeExpr::Other, TypeExpr::Named);
        }
        let key = (name.to_string(), at);
        if let Some(ty) = self.memo.borrow().get(&key) {
            return ty.clone();
        }
        let ty = self.var_uncached(name, at, depth);
        self.memo.borrow_mut().insert(key, ty.clone());
        ty
    }

    fn var_uncached(&self, name: &str, at: u32, depth: u32) -> TypeExpr {
        let scope = self.scope_at(at);
        let same_scope = |position: u32| self.scope_at(position) == scope;

        let mut bindings: Vec<(u32, Mode, Binding<'_, 'ast, 'arena>)> = Vec::new();
        if let Some(i) = scope {
            let fact = &self.facts.scopes[i];
            for (p, param) in fact.params.parameters.iter().enumerate() {
                if variable_name(param.variable.name) == name {
                    bindings.push((fact.start, Mode::Replace, Binding::Param(fact, p)));
                }
            }
        }
        for assignment in &self.facts.assignments {
            if assignment.var == name && assignment.end <= at && same_scope(assignment.start) {
                let binding = match assignment.rhs {
                    Some(_) => Binding::Assign(assignment),
                    None => Binding::Unknown,
                };
                bindings.push((assignment.start, self.mode(assignment.start, at), binding));
            }
        }
        for foreach in &self.facts.foreaches {
            if foreach.body_start > at || !same_scope(foreach.start) {
                continue;
            }
            let mode = self.mode(foreach.start, at);
            if foreach.value.as_deref() == Some(name) {
                bindings.push((foreach.start, mode, Binding::ForeachValue(foreach)));
            } else if foreach.unknown.iter().any(|v| v == name) {
                bindings.push((foreach.start, mode, Binding::Unknown));
            }
        }
        for catch in &self.facts.catches {
            if catch.var == name && catch.start <= at && same_scope(catch.start) {
                bindings.push((
                    catch.start,
                    self.mode(catch.start, at),
                    Binding::Catch(catch),
                ));
            }
        }
        for narrowing in &self.facts.narrowings {
            if narrowing.var == name
                && narrowing.start <= at
                && at <= narrowing.end
                && same_scope(narrowing.start)
            {
                bindings.push((
                    narrowing.start,
                    Mode::Replace,
                    Binding::Narrow(narrowing.class),
                ));
            }
        }
        for (position, var, ty) in &self.var_docs {
            if var == name && *position < at && same_scope(*position) {
                bindings.push((
                    *position,
                    self.mode(*position, at),
                    Binding::Doc(ty.clone()),
                ));
            }
        }

        if bindings.iter().all(|(_, mode, _)| *mode == Mode::Ignore) {
            return self.captured(name, scope, depth);
        }
        bindings.sort_by_key(|(position, _, _)| *position);
        let last_replace = bindings
            .iter()
            .rposition(|(_, mode, _)| *mode == Mode::Replace);
        let mut state = State::Unset;
        for (i, (_, mode, binding)) in bindings.iter().enumerate() {
            match last_replace {
                Some(r) if i < r => {}
                Some(r) if i == r => state = self.binding_state(binding, depth),
                _ if *mode == Mode::Merge => {
                    state = state.merge(self.binding_state(binding, depth));
                }
                _ => {}
            }
        }
        let ty = state.into_type();
        match self.enclosing(at) {
            Some(class) => {
                let parent = self.enclosing_parent(at);
                bind(&ty, &class, &class, parent.as_deref())
            }
            None => ty,
        }
    }

    /// A variable with no binding in its own scope: arrow functions see the enclosing
    /// scope, closures see the variables of their `use` clause.
    fn captured(&self, name: &str, scope: Option<usize>, depth: u32) -> TypeExpr {
        let Some(scope) = scope.map(|i| &self.facts.scopes[i]) else {
            return TypeExpr::Other;
        };
        let visible = match &scope.kind {
            ScopeKind::Arrow => true,
            ScopeKind::Closure(uses) => uses.iter().any(|u| u == name),
            ScopeKind::Function => false,
        };
        if visible {
            // Just before the closure starts is the enclosing scope.
            self.var(name, scope.start.saturating_sub(1), depth + 1)
        } else {
            TypeExpr::Other
        }
    }

    fn binding_state(&self, binding: &Binding<'_, '_, '_>, depth: u32) -> State {
        match binding {
            Binding::Param(scope, i) => State::of(self.param_type(scope, *i)),
            Binding::Assign(assignment) => {
                if let Some(ty) = self.assignment_doc(assignment) {
                    return State::of(ty);
                }
                match assignment.rhs {
                    Some(Expression::Literal(Literal::Null(_))) => State::Null,
                    Some(rhs) => State::of(self.expr(rhs, assignment.start, depth + 1)),
                    None => State::Unknown,
                }
            }
            Binding::ForeachValue(foreach) => State::of(element_of(&self.expr(
                foreach.iterable,
                foreach.start,
                depth + 1,
            ))),
            Binding::Unknown => State::Unknown,
            Binding::Catch(catch) => State::of(self.hint_type(catch.hint)),
            Binding::Narrow(class) => match self.names.get(class) {
                Some(name) => State::Known(TypeExpr::Named(name.fqn.clone())),
                None => State::Unknown,
            },
            Binding::Doc(ty) => State::of(ty.clone()),
        }
    }

    /// A parameter's PHPDoc `@param` type when it names a class, else its declared type.
    fn param_type(&self, scope: &ScopeFact<'_, '_>, i: usize) -> TypeExpr {
        let Some(param) = scope.params.parameters.iter().nth(i) else {
            return TypeExpr::Other;
        };
        let name = variable_name(param.variable.name);
        let doc_type = get_docblock_before_position(self.program.trivia.as_slice(), scope.start)
            .and_then(|doc| {
                parse_docblock(
                    &String::from_utf8_lossy(doc.value),
                    &self.summary.file_names.at(scope.start),
                )
                .params
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, ty)| ty)
            });
        let declared = param.hint.as_ref().map(|hint| self.hint_type(hint));
        pick_type(doc_type, declared).unwrap_or(TypeExpr::Other)
    }

    /// The `@var` type of a docblock right before an assignment, when it names a
    /// class and either names this variable or no variable.
    fn assignment_doc(&self, assignment: &AssignFact<'_>) -> Option<TypeExpr> {
        let doc = get_docblock_before_position(self.program.trivia.as_slice(), assignment.start)?;
        parse_docblock(
            &String::from_utf8_lossy(doc.value),
            &self.summary.file_names.at(assignment.start),
        )
        .var
        .into_iter()
        .find(|(var, _)| var.as_deref().is_none_or(|v| v == assignment.var))
        .map(|(_, ty)| ty)
        .and_then(|ty| pick_type(Some(ty), None))
    }
}

/// Replaces `static`/`$this` with `static_class`, `self` with `self_class` and
/// `parent` with `parent`.
fn bind(ty: &TypeExpr, static_class: &str, self_class: &str, parent: Option<&str>) -> TypeExpr {
    match ty {
        TypeExpr::StaticType => TypeExpr::Named(static_class.to_string()),
        TypeExpr::SelfType => TypeExpr::Named(self_class.to_string()),
        TypeExpr::ParentType => parent.map_or(TypeExpr::Other, |p| TypeExpr::Named(p.to_string())),
        TypeExpr::ArrayOf(element) => {
            TypeExpr::ArrayOf(Box::new(bind(element, static_class, self_class, parent)))
        }
        TypeExpr::Union(members) => union(
            members
                .iter()
                .map(|m| bind(m, static_class, self_class, parent))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The class names a type can be: named classes alone or in a union.
pub(crate) fn class_names(ty: &TypeExpr) -> Vec<String> {
    match ty {
        TypeExpr::Named(name) => vec![name.clone()],
        TypeExpr::Union(members) => members.iter().flat_map(class_names).collect(),
        _ => Vec::new(),
    }
}

fn element_of(ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::ArrayOf(element) => (**element).clone(),
        TypeExpr::Union(members) => union(members.iter().map(element_of).collect()),
        _ => TypeExpr::Other,
    }
}

/// Flattens and deduplicates; `Other` members drop and a single member stands alone.
fn union(types: Vec<TypeExpr>) -> TypeExpr {
    let mut members: Vec<TypeExpr> = Vec::new();
    for ty in types {
        match ty {
            TypeExpr::Other => {}
            TypeExpr::Union(inner) => {
                for member in inner {
                    if !members.contains(&member) {
                        members.push(member);
                    }
                }
            }
            ty => {
                if !members.contains(&ty) {
                    members.push(ty);
                }
            }
        }
    }
    match members.len() {
        0 => TypeExpr::Other,
        1 => members.pop().expect("one member"),
        _ => TypeExpr::Union(members),
    }
}
