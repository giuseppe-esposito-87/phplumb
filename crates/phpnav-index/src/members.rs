//! Member lookup through the class hierarchy.

use std::collections::{HashSet, VecDeque};

use crate::index::{ClassRef, Index};
use crate::summary::{Member, MemberKind};

/// Every declaration of `name` of `kind` visible on `class`, one per declaration of
/// `class` in the index, deduplicated. The lookup order per declaration: its own
/// members, its traits (recursively), its parent chain (each with its traits), then
/// all interfaces reached on the way (recursively). `Constant` also finds enum cases.
pub(crate) fn find_member<'a>(
    index: &'a Index,
    class: &str,
    kind: MemberKind,
    name: &str,
) -> Vec<(ClassRef<'a>, &'a Member)> {
    let mut found: Vec<(ClassRef<'a>, &'a Member)> = Vec::new();
    for root in index.classes_named(class) {
        if let Some(hit) = search(index, root, kind, name) {
            let duplicate = found
                .iter()
                .any(|(c, m)| c.path == hit.0.path && m.name_start == hit.1.name_start);
            if !duplicate {
                found.push(hit);
            }
        }
    }
    found
}

fn search<'a>(
    index: &'a Index,
    root: ClassRef<'a>,
    kind: MemberKind,
    name: &str,
) -> Option<(ClassRef<'a>, &'a Member)> {
    let mut visited: HashSet<String> = HashSet::new();
    let mut interfaces: VecDeque<String> = VecDeque::new();

    let mut current = Some(root);
    while let Some(class) = current {
        if !visited.insert(class.declaration.fqn.to_ascii_lowercase()) {
            break;
        }
        if let Some(member) = own(&class, kind, name) {
            return Some((class, member));
        }
        if let Some(hit) = in_traits(index, &class, kind, name, &mut visited) {
            return Some(hit);
        }
        let shape = class.declaration.class.as_ref()?;
        interfaces.extend(shape.interfaces.iter().cloned());
        current = shape
            .parent
            .as_deref()
            .and_then(|parent| index.classes_named(parent).into_iter().next());
    }

    while let Some(interface) = interfaces.pop_front() {
        let Some(declaration) = index.classes_named(&interface).into_iter().next() else {
            continue;
        };
        if !visited.insert(declaration.declaration.fqn.to_ascii_lowercase()) {
            continue;
        }
        if let Some(member) = own(&declaration, kind, name) {
            return Some((declaration, member));
        }
        if let Some(shape) = &declaration.declaration.class {
            interfaces.extend(shape.interfaces.iter().cloned());
        }
    }
    None
}

fn in_traits<'a>(
    index: &'a Index,
    class: &ClassRef<'a>,
    kind: MemberKind,
    name: &str,
    visited: &mut HashSet<String>,
) -> Option<(ClassRef<'a>, &'a Member)> {
    let shape = class.declaration.class.as_ref()?;
    for name_of_trait in &shape.traits {
        let Some(declaration) = index.classes_named(name_of_trait).into_iter().next() else {
            continue;
        };
        if !visited.insert(declaration.declaration.fqn.to_ascii_lowercase()) {
            continue;
        }
        if let Some(member) = own(&declaration, kind, name) {
            return Some((declaration, member));
        }
        if let Some(hit) = in_traits(index, &declaration, kind, name, visited) {
            return Some(hit);
        }
    }
    None
}

/// A member declared directly on `class`.
fn own<'a>(class: &ClassRef<'a>, kind: MemberKind, name: &str) -> Option<&'a Member> {
    class
        .declaration
        .class
        .as_ref()?
        .members
        .iter()
        .find(|member| matches(member, kind, name))
}

/// Whether `member` is the `kind` member `name`. Methods match case-insensitively;
/// properties, constants and enum cases match exactly.
pub(crate) fn matches(member: &Member, kind: MemberKind, name: &str) -> bool {
    match (kind, member.kind) {
        (MemberKind::Method, MemberKind::Method) => member.name.eq_ignore_ascii_case(name),
        (MemberKind::Property, MemberKind::Property) => member.name == name,
        (MemberKind::Constant, MemberKind::Constant | MemberKind::EnumCase)
        | (MemberKind::EnumCase, MemberKind::EnumCase) => member.name == name,
        _ => false,
    }
}
