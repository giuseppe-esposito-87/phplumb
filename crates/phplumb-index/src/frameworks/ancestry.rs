//! Whether a class descends from another, through parents and interfaces.

use std::collections::HashSet;

use crate::index::Index;

/// Whether `class` is, extends or implements one of `ancestors`.
pub(crate) fn is_a(index: &Index, class: &str, ancestors: &[&str]) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending = vec![class.to_string()];
    while let Some(next) = pending.pop() {
        if ancestors.iter().any(|c| c.eq_ignore_ascii_case(&next)) {
            return true;
        }
        if !seen.insert(next.to_ascii_lowercase()) {
            continue;
        }
        for supertype in index.supertypes(&next) {
            pending.push(supertype.declaration.fqn.clone());
        }
    }
    false
}
