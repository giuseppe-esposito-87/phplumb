//! Type inference for the expression under the cursor.
//!
//! One walk of the cursor file collects the facts inference needs, as references
//! into the syntax tree: member accesses, function scopes, assignments, `foreach`
//! and `catch` bindings, `instanceof` checks and inline `@var` docblocks. Types are
//! then inferred on demand, before the tree's arena is dropped.

use std::collections::HashMap;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_span::HasSpan;
use mago_syntax::comments::docblock::get_docblock_before_position;
use mago_syntax::cst::{
    Access, ArrowFunction, Assignment, BinaryOperator, Call, ClassLikeMemberSelector, Closure,
    Expression, Foreach, ForeachTarget, Function, FunctionLikeParameterList, Hint, If, IfBody,
    Method, MethodCall, NullSafeMethodCall, NullSafePropertyAccess, Program, PropertyAccess,
    TriviaKind, TryCatchClause, Variable,
};
use mago_syntax::parser::parse_file_content;
use mago_syntax::walker::Walker;

use crate::index::Index;
use crate::phpdoc::{TypeExpr, parse_docblock, parse_type};
use crate::summary::{FileSummary, MemberKind, NameKind, NameRef, pick_type};

const MAX_DEPTH: u32 = 16;

/// The classes, kind and name of the instance member access under `offset`, when
/// the cursor is on the member name of `$x->name` or `$x?->name()`. The classes are
/// empty when the receiver's type is unknown.
pub(crate) fn member_at(
    index: &Index,
    summary: &FileSummary,
    source: &[u8],
    offset: u32,
) -> Option<(Vec<String>, MemberKind, String)> {
    let arena = LocalArena::new();
    let program = parse_file_content(&arena, FileId::zero(), source);
    let mut facts = Facts::default();
    Collect.walk_program(program, &mut facts);
    let access = facts
        .accesses
        .iter()
        .find(|a| a.start <= offset && offset <= a.end)?;
    let ctx = Ctx::new(index, summary, program, source, &facts);
    let receiver = ctx.expr(access.object, access.start, 0);
    Some((class_names(&receiver), access.kind, access.name.clone()))
}

struct AccessFact<'arena> {
    start: u32,
    end: u32,
    kind: MemberKind,
    name: String,
    object: &'arena Expression<'arena>,
}

struct ScopeFact<'ast, 'arena> {
    start: u32,
    end: u32,
    params: &'ast FunctionLikeParameterList<'arena>,
}

struct AssignFact<'arena> {
    start: u32,
    end: u32,
    var: String,
    rhs: &'arena Expression<'arena>,
}

struct ForeachFact<'arena> {
    start: u32,
    body_start: u32,
    end: u32,
    var: String,
    iterable: &'arena Expression<'arena>,
}

struct CatchFact<'ast, 'arena> {
    start: u32,
    end: u32,
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

#[derive(Default)]
struct Facts<'ast, 'arena> {
    accesses: Vec<AccessFact<'arena>>,
    scopes: Vec<ScopeFact<'ast, 'arena>>,
    assignments: Vec<AssignFact<'arena>>,
    foreaches: Vec<ForeachFact<'arena>>,
    catches: Vec<CatchFact<'ast, 'arena>>,
    narrowings: Vec<NarrowFact>,
    /// Every direct variable: name without `$`, start, end.
    variables: Vec<(String, u32, u32)>,
}

impl<'arena> Facts<'_, 'arena> {
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
        let span = method.span();
        f.scopes.push(ScopeFact {
            start: span.start.offset,
            end: span.end.offset,
            params: &method.parameter_list,
        });
    }

    fn walk_in_function(&self, function: &'ast Function<'arena>, f: &mut Facts<'ast, 'arena>) {
        let span = function.span();
        f.scopes.push(ScopeFact {
            start: span.start.offset,
            end: span.end.offset,
            params: &function.parameter_list,
        });
    }

    fn walk_in_closure(&self, closure: &'ast Closure<'arena>, f: &mut Facts<'ast, 'arena>) {
        let span = closure.span();
        f.scopes.push(ScopeFact {
            start: span.start.offset,
            end: span.end.offset,
            params: &closure.parameter_list,
        });
    }

    fn walk_in_arrow_function(
        &self,
        arrow: &'ast ArrowFunction<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        let span = arrow.span();
        f.scopes.push(ScopeFact {
            start: span.start.offset,
            end: span.end.offset,
            params: &arrow.parameter_list,
        });
    }

    fn walk_in_assignment(
        &self,
        assignment: &'ast Assignment<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        if !assignment.operator.is_assign() {
            return;
        }
        if let Expression::Variable(Variable::Direct(variable)) = assignment.lhs {
            let span = assignment.span();
            f.assignments.push(AssignFact {
                start: span.start.offset,
                end: span.end.offset,
                var: variable_name(variable.name),
                rhs: assignment.rhs,
            });
        }
    }

    fn walk_in_foreach(&self, foreach: &'ast Foreach<'arena>, f: &mut Facts<'ast, 'arena>) {
        let value = match &foreach.target {
            ForeachTarget::Value(target) => target.value,
            ForeachTarget::KeyValue(target) => target.value,
        };
        if let Expression::Variable(Variable::Direct(variable)) = value {
            let span = foreach.span();
            f.foreaches.push(ForeachFact {
                start: span.start.offset,
                body_start: foreach.right_parenthesis.end.offset,
                end: span.end.offset,
                var: variable_name(variable.name),
                iterable: foreach.expression,
            });
        }
    }

    fn walk_in_try_catch_clause(
        &self,
        clause: &'ast TryCatchClause<'arena>,
        f: &mut Facts<'ast, 'arena>,
    ) {
        if let Some(variable) = &clause.variable {
            let span = clause.block.span();
            f.catches.push(CatchFact {
                start: span.start.offset,
                end: span.end.offset,
                var: variable_name(variable.name),
                hint: &clause.hint,
            });
        }
    }

    fn walk_in_if(&self, r#if: &'ast If<'arena>, f: &mut Facts<'ast, 'arena>) {
        let (start, end) = match &r#if.body {
            IfBody::Statement(body) => {
                let span = body.statement.span();
                (span.start.offset, span.end.offset)
            }
            IfBody::ColonDelimited(body) => {
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
                start,
                end,
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

/// A source of a variable's type, by position: the latest before the use wins.
enum Binding<'f, 'ast, 'arena> {
    Param(&'f ScopeFact<'ast, 'arena>, usize),
    Assign(&'f AssignFact<'arena>),
    Foreach(&'f ForeachFact<'arena>),
    Catch(&'f CatchFact<'ast, 'arena>),
    Doc(TypeExpr),
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

    fn enclosing(&self, at: u32) -> Option<String> {
        self.summary.enclosing_class(at).map(|c| c.fqn.clone())
    }

    fn enclosing_parent(&self, at: u32) -> Option<String> {
        self.summary.enclosing_class(at)?.parent.clone()
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
            Expression::Call(Call::Method(call)) => self.member_on(
                self.expr(call.object, at, depth),
                &call.method,
                MemberKind::Method,
            ),
            Expression::Call(Call::NullSafeMethod(call)) => self.member_on(
                self.expr(call.object, at, depth),
                &call.method,
                MemberKind::Method,
            ),
            Expression::Call(Call::StaticMethod(call)) => match self.class_of(call.class, at) {
                Some(class) => {
                    self.member_on(TypeExpr::Named(class), &call.method, MemberKind::Method)
                }
                None => TypeExpr::Other,
            },
            Expression::Call(Call::Function(call)) => self.function_returns(call.function),
            Expression::Access(Access::Property(access)) => self.member_on(
                self.expr(access.object, at, depth),
                &access.property,
                MemberKind::Property,
            ),
            Expression::Access(Access::NullSafeProperty(access)) => self.member_on(
                self.expr(access.object, at, depth),
                &access.property,
                MemberKind::Property,
            ),
            Expression::Access(Access::StaticProperty(access)) => {
                match (self.class_of(access.class, at), &access.property) {
                    (Some(class), Variable::Direct(variable)) => self.member_type(
                        &TypeExpr::Named(class),
                        MemberKind::Property,
                        &variable_name(variable.name),
                    ),
                    _ => TypeExpr::Other,
                }
            }
            _ => TypeExpr::Other,
        }
    }

    fn member_on(
        &self,
        receiver: TypeExpr,
        selector: &ClassLikeMemberSelector<'_>,
        kind: MemberKind,
    ) -> TypeExpr {
        match selector {
            ClassLikeMemberSelector::Identifier(name) => {
                self.member_type(&receiver, kind, &String::from_utf8_lossy(name.value))
            }
            _ => TypeExpr::Other,
        }
    }

    /// The type of member `name` on every class of `receiver`. `static` and `$this`
    /// become the receiver class, `self` the declaring class, `parent` its parent.
    fn member_type(&self, receiver: &TypeExpr, kind: MemberKind, name: &str) -> TypeExpr {
        let mut types = Vec::new();
        for class in class_names(receiver) {
            for (declaring, member) in self.index.find_member(&class, kind, name) {
                if let Some(ty) = &member.ty {
                    let shape = declaring.declaration.class.as_ref();
                    types.push(bind(
                        ty,
                        &class,
                        &declaring.declaration.fqn,
                        shape.and_then(|s| s.parent.as_deref()),
                    ));
                }
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

    /// The type of variable `name` used at `at`.
    fn var(&self, name: &str, at: u32, depth: u32) -> TypeExpr {
        if depth > MAX_DEPTH {
            return TypeExpr::Other;
        }
        if name == "this" {
            return self.enclosing(at).map_or(TypeExpr::Other, TypeExpr::Named);
        }
        let narrowed = self
            .facts
            .narrowings
            .iter()
            .filter(|n| n.var == name && n.start <= at && at <= n.end)
            .max_by_key(|n| n.start);
        if let Some(narrowed) = narrowed
            && let Some(class) = self.names.get(&narrowed.class)
        {
            return TypeExpr::Named(class.fqn.clone());
        }

        let scope = self
            .facts
            .scopes
            .iter()
            .filter(|s| s.start <= at && at <= s.end)
            .max_by_key(|s| s.start);
        let (scope_start, scope_end) = scope.map_or((0, u32::MAX), |s| (s.start, s.end));
        let in_scope = |position: u32| scope_start <= position && position <= scope_end;

        let mut best: Option<(u32, Binding<'_, 'ast, 'arena>)> = None;
        let mut consider = |position: u32, binding: Binding<'a, 'ast, 'arena>| {
            if best.as_ref().is_none_or(|(p, _)| position >= *p) {
                best = Some((position, binding));
            }
        };
        if let Some(scope) = scope {
            for (i, param) in scope.params.parameters.iter().enumerate() {
                if variable_name(param.variable.name) == name {
                    consider(scope.start, Binding::Param(scope, i));
                }
            }
        }
        for assignment in &self.facts.assignments {
            if assignment.var == name && assignment.end <= at && in_scope(assignment.start) {
                consider(assignment.start, Binding::Assign(assignment));
            }
        }
        for foreach in &self.facts.foreaches {
            if foreach.var == name && foreach.body_start <= at && at <= foreach.end {
                consider(foreach.start, Binding::Foreach(foreach));
            }
        }
        for catch in &self.facts.catches {
            if catch.var == name && catch.start <= at && at <= catch.end {
                consider(catch.start, Binding::Catch(catch));
            }
        }
        for (position, var, ty) in &self.var_docs {
            if var == name && *position < at && in_scope(*position) {
                consider(*position, Binding::Doc(ty.clone()));
            }
        }

        let Some((_, binding)) = best else {
            return TypeExpr::Other;
        };
        let ty = match binding {
            Binding::Param(scope, i) => self.param_type(scope, i),
            Binding::Assign(assignment) => match self.assignment_doc(assignment) {
                Some(ty) => ty,
                None => self.expr(assignment.rhs, assignment.start, depth + 1),
            },
            Binding::Foreach(foreach) => {
                element_of(&self.expr(foreach.iterable, foreach.start, depth + 1))
            }
            Binding::Catch(catch) => self.hint_type(catch.hint),
            Binding::Doc(ty) => ty,
        };
        let class = self.enclosing(at);
        let parent = self.enclosing_parent(at);
        match &class {
            Some(class) => bind(&ty, class, class, parent.as_deref()),
            None => ty,
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

/// Replaces `static`/`$this` with the receiver, `self` with the declaring class and
/// `parent` with its parent.
fn bind(ty: &TypeExpr, receiver: &str, declaring: &str, parent: Option<&str>) -> TypeExpr {
    match ty {
        TypeExpr::StaticType => TypeExpr::Named(receiver.to_string()),
        TypeExpr::SelfType => TypeExpr::Named(declaring.to_string()),
        TypeExpr::ParentType => parent.map_or(TypeExpr::Other, |p| TypeExpr::Named(p.to_string())),
        TypeExpr::ArrayOf(element) => {
            TypeExpr::ArrayOf(Box::new(bind(element, receiver, declaring, parent)))
        }
        TypeExpr::Union(members) => union(
            members
                .iter()
                .map(|m| bind(m, receiver, declaring, parent))
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
