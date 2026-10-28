//! Generic types: what a class written with arguments, such as
//! `Collection<int, Item>`, gives the templates of its ancestors, and the element
//! types of arrays and iterable classes.

use std::collections::{HashSet, VecDeque};

use crate::Index;
use crate::infer::union;
use crate::phpdoc::TypeExpr;

/// How many classes a walk through parents visits at most.
const MAX_CLASSES: usize = 16;

/// The arguments that `class<args>` gives `ancestor`, found through the generic
/// parents each class names in `@extends` and `@implements`, with each class's
/// templates replaced by the arguments it received. `None` when the walk does not
/// reach `ancestor`.
pub(crate) fn supertype_args(
    index: &Index,
    class: &str,
    args: &[TypeExpr],
    ancestor: &str,
) -> Option<Vec<TypeExpr>> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending: VecDeque<(String, Vec<TypeExpr>)> = VecDeque::new();
    pending.push_back((class.to_string(), args.to_vec()));
    while let Some((next, next_args)) = pending.pop_front() {
        if next.eq_ignore_ascii_case(ancestor) {
            return Some(next_args);
        }
        if !seen.insert(next.to_ascii_lowercase()) || seen.len() > MAX_CLASSES {
            continue;
        }
        for declaration in index.classes_named(&next) {
            let Some(shape) = &declaration.declaration.class else {
                continue;
            };
            for (parent, parent_args) in &shape.generic_supertypes {
                let bound = parent_args
                    .iter()
                    .map(|arg| substitute(arg, &shape.templates, &next_args))
                    .collect();
                pending.push_back((parent.clone(), bound));
            }
        }
    }
    None
}

/// `ty` with each template replaced by the argument at its position, or by
/// `Other` when there is none.
pub(crate) fn substitute(ty: &TypeExpr, templates: &[String], args: &[TypeExpr]) -> TypeExpr {
    match ty {
        TypeExpr::Named(name) => match templates.iter().position(|t| t == name) {
            Some(i) => args.get(i).cloned().unwrap_or(TypeExpr::Other),
            None => ty.clone(),
        },
        TypeExpr::ArrayOf(element) => match substitute(element, templates, args) {
            TypeExpr::Other => TypeExpr::Other,
            element => TypeExpr::ArrayOf(Box::new(element)),
        },
        TypeExpr::Generic { class, args: inner } => TypeExpr::Generic {
            class: class.clone(),
            args: inner
                .iter()
                .map(|arg| substitute(arg, templates, args))
                .collect(),
        },
        TypeExpr::Union(members) => union(
            members
                .iter()
                .map(|member| substitute(member, templates, args))
                .collect(),
        ),
        _ => ty.clone(),
    }
}

/// The type of the values `foreach` gives over a value of type `ty`. A class
/// written with one argument that is `Traversable` gives that argument, as
/// `\Iterator<Item>` does; any other class gives the value argument it passes to
/// `Traversable`.
pub(crate) fn element_type(index: &Index, ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::ArrayOf(element) => (**element).clone(),
        TypeExpr::Union(members) => union(
            members
                .iter()
                .map(|member| element_type(index, member))
                .collect(),
        ),
        TypeExpr::Generic { class, args }
            if args.len() == 1 && reaches(index, class, "Traversable") =>
        {
            args[0].clone()
        }
        _ => value_of(index, ty, "Traversable"),
    }
}

/// The type of `$value[$key]` for a value of type `ty`: an array's element, or the
/// value argument a class passes to `ArrayAccess`.
pub(crate) fn offset_type(index: &Index, ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::ArrayOf(element) => (**element).clone(),
        TypeExpr::Union(members) => union(
            members
                .iter()
                .map(|member| offset_type(index, member))
                .collect(),
        ),
        _ => value_of(index, ty, "ArrayAccess"),
    }
}

/// The second argument, the value, that the class of `ty` gives `root`.
fn value_of(index: &Index, ty: &TypeExpr, root: &str) -> TypeExpr {
    let (class, args) = match ty {
        TypeExpr::Named(class) | TypeExpr::Tagged { class, .. } => (class, &[][..]),
        TypeExpr::Generic { class, args } => (class, args.as_slice()),
        _ => return TypeExpr::Other,
    };
    supertype_args(index, class, args, root)
        .and_then(|args| args.get(1).cloned())
        .unwrap_or(TypeExpr::Other)
}

/// Whether `class` is `root` or has it among its parents and interfaces.
fn reaches(index: &Index, class: &str, root: &str) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending = vec![class.to_string()];
    while let Some(next) = pending.pop() {
        if next.eq_ignore_ascii_case(root) {
            return true;
        }
        if !seen.insert(next.to_ascii_lowercase()) || seen.len() > MAX_CLASSES {
            continue;
        }
        pending.extend(
            index
                .supertypes(&next)
                .into_iter()
                .map(|class| class.declaration.fqn.clone()),
        );
    }
    false
}
