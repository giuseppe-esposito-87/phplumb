//! PHP's built-in classes, functions and constants, from the names JetBrains
//! `phpstorm-stubs` declares (see `data/README.md`).

use std::collections::{BTreeSet, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Context;

use crate::summary::{MemberKind, SymbolKind, summarize};

/// `#` comment lines, then one `c`, `f`, `k` or `m` line per class-like, function,
/// constant or method name; methods by their lowercased bare name.
const LIST: &str = include_str!("../data/builtins.txt");

struct Names {
    classes: HashSet<&'static str>,
    functions: HashSet<&'static str>,
    constants: HashSet<&'static str>,
    methods: HashSet<&'static str>,
}

fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = Names {
            classes: HashSet::new(),
            functions: HashSet::new(),
            constants: HashSet::new(),
            methods: HashSet::new(),
        };
        for line in LIST.lines().filter(|l| !l.starts_with('#')) {
            match line.split_once(' ') {
                Some(("c", name)) => names.classes.insert(name),
                Some(("f", name)) => names.functions.insert(name),
                Some(("k", name)) => names.constants.insert(name),
                Some(("m", name)) => names.methods.insert(name),
                _ => false,
            };
        }
        names
    })
}

/// Whether a built-in class, interface, trait or enum is named `fqn`.
pub fn is_class(fqn: &str) -> bool {
    names().classes.contains(class_key(fqn).as_str())
}

/// Whether a built-in function is named `fqn`.
pub fn is_function(fqn: &str) -> bool {
    names().functions.contains(class_key(fqn).as_str())
}

/// Whether a built-in constant is named `fqn`.
pub fn is_constant(fqn: &str) -> bool {
    names().constants.contains(constant_key(fqn).as_str())
}

/// Whether a built-in class-like declares a method named `name`.
pub fn is_method(name: &str) -> bool {
    names().methods.contains(name.to_ascii_lowercase().as_str())
}

/// Class and function names ignore ASCII case.
fn class_key(fqn: &str) -> String {
    fqn.trim_start_matches('\\').to_ascii_lowercase()
}

/// Constant names keep their case; only the namespace ignores it.
fn constant_key(fqn: &str) -> String {
    let fqn = fqn.trim_start_matches('\\');
    match fqn.rsplit_once('\\') {
        Some((namespace, name)) => format!("{}\\{name}", namespace.to_ascii_lowercase()),
        None => fqn.to_string(),
    }
}

/// The list for the stubs below `stubs`: one line per declaration in its PHP files,
/// sorted and deduplicated. The stubs' own `tests` directory is skipped.
pub fn generate(stubs: &Path) -> anyhow::Result<String> {
    let mut lines: BTreeSet<String> = BTreeSet::new();
    let mut pending = vec![stubs.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries =
            std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))?;
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                if !(dir == stubs && path.file_name().is_some_and(|n| n == "tests")) {
                    pending.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "php") {
                continue;
            }
            let source =
                std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            for declaration in summarize(&source).declarations {
                for member in declaration.class.iter().flat_map(|c| c.members.iter()) {
                    if member.kind == MemberKind::Method {
                        lines.insert(format!("m {}", member.name.to_ascii_lowercase()));
                    }
                }
                let line = match declaration.kind {
                    SymbolKind::Function => format!("f {}", class_key(&declaration.fqn)),
                    SymbolKind::Constant => format!("k {}", constant_key(&declaration.fqn)),
                    _ => format!("c {}", class_key(&declaration.fqn)),
                };
                lines.insert(line);
            }
        }
    }
    let mut list = String::new();
    for line in lines {
        list.push_str(&line);
        list.push('\n');
    }
    Ok(list)
}
