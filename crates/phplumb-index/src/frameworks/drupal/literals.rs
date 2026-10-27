//! What PHP code may define as Drupal ids at run time. Every string literal and
//! class name of a service provider or compiler pass, and the first argument of
//! every `set()`, `register()`, `setAlias()` or `setDefinition()` call, may be a
//! service; every string literal of a file building routes may be a route, and the
//! strings it builds give route name shapes; every string literal of a file adding
//! plugin definitions may be a plugin. Such an id is never reported as undefined.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::index::Index;
use crate::infer;
use rayon::prelude::*;

use crate::summary::{NameKind, RefKey, summarize};

/// Classes whose mention makes a file one that may define services.
const SERVICE_DEFINERS: [&str; 4] = [
    "Symfony\\Component\\DependencyInjection\\ContainerBuilder",
    "Drupal\\Core\\DependencyInjection\\ServiceProviderInterface",
    "Drupal\\Core\\DependencyInjection\\ServiceModifierInterface",
    "Symfony\\Component\\DependencyInjection\\Compiler\\CompilerPassInterface",
];

/// Classes whose mention makes a file one that may define routes.
const ROUTE_DEFINERS: [&str; 2] = [
    "Symfony\\Component\\Routing\\Route",
    "Symfony\\Component\\Routing\\RouteCollection",
];

/// Methods whose first argument may name a service they define.
const SERVICE_SETTERS: [&str; 4] = ["set", "register", "setAlias", "setDefinition"];

/// What one file contributes.
#[derive(Default)]
struct FileLiterals {
    services: Vec<String>,
    routes: Vec<String>,
    route_shapes: Vec<String>,
    plugins: Vec<String>,
}

/// The ids PHP code may define, kept per file so an update reads only the files that
/// changed.
#[derive(Default)]
pub(crate) struct PhpLiterals {
    by_file: HashMap<PathBuf, FileLiterals>,
    pub(crate) services: HashSet<String>,
    pub(crate) routes: HashSet<String>,
    /// Shapes of route names built from parts, `*` for each part that is not
    /// literal; each starts with literal text holding a `.`.
    pub(crate) route_shapes: Vec<String>,
    pub(crate) plugins: HashSet<String>,
}

impl PhpLiterals {
    pub(crate) fn read(index: &Index) -> PhpLiterals {
        let mut literals = PhpLiterals::default();
        literals.refresh(index);
        literals
    }

    /// Reads the changed files again and drops the removed ones.
    pub(crate) fn update(&mut self, changed: &[PathBuf], removed: &[PathBuf], index: &Index) {
        for path in removed.iter().chain(changed) {
            self.by_file.retain(|file, _| !file.starts_with(path));
        }
        self.refresh(index);
    }

    /// Whether a route shape matches `name`.
    pub(crate) fn matches_route(&self, name: &str) -> bool {
        self.route_shapes.iter().any(|shape| glob(shape, name))
    }

    /// Reads the candidate files not read yet, then gathers every file's share.
    fn refresh(&mut self, index: &Index) {
        let candidates = |keys: Vec<RefKey>| -> HashSet<PathBuf> {
            keys.iter()
                .flat_map(|key| index.candidate_files(key))
                .map(Path::to_path_buf)
                .collect()
        };
        let classes = |names: &[&str]| -> Vec<RefKey> {
            names
                .iter()
                .map(|name| RefKey::Class(name.to_string()))
                .collect()
        };
        let members = |names: &[&str]| -> Vec<RefKey> {
            names
                .iter()
                .map(|name| RefKey::Member(name.to_string()))
                .collect()
        };
        let service_files = candidates(classes(&SERVICE_DEFINERS));
        let setter_files = candidates(members(&SERVICE_SETTERS));
        let route_files = candidates(classes(&ROUTE_DEFINERS));
        let plugin_files = candidates(members(&["setDefinition"]));
        let mut all: Vec<&PathBuf> = service_files
            .iter()
            .chain(&setter_files)
            .chain(&route_files)
            .chain(&plugin_files)
            .filter(|path| !self.by_file.contains_key(*path))
            .collect();
        all.sort();
        all.dedup();
        let read: Vec<(PathBuf, FileLiterals)> = all
            .par_iter()
            .filter_map(|path| {
                let source = std::fs::read(path).ok()?;
                let mut found = FileLiterals::default();
                let literals = infer::string_literals(&source);
                if service_files.contains(*path) {
                    found.services.extend(literals.iter().cloned());
                    found.services.extend(
                        summarize(&source)
                            .names
                            .into_iter()
                            .filter(|n| n.kind == NameKind::ClassLike)
                            .map(|n| n.fqn),
                    );
                }
                if setter_files.contains(*path) {
                    found
                        .services
                        .extend(infer::first_arguments(&source, &SERVICE_SETTERS));
                }
                if route_files.contains(*path) {
                    found.routes.extend(literals.iter().cloned());
                    found.route_shapes = infer::string_shapes(&source)
                        .into_iter()
                        .filter(|shape| {
                            shape
                                .split_once('*')
                                .is_some_and(|(head, _)| head.contains('.'))
                        })
                        .collect();
                }
                if plugin_files.contains(*path) {
                    found.plugins.extend(literals);
                }
                Some(((*path).clone(), found))
            })
            .collect();
        self.by_file.extend(read);
        let files = self.by_file.values();
        self.services = files
            .clone()
            .flat_map(|f| f.services.iter().cloned())
            .collect();
        self.routes = files
            .clone()
            .flat_map(|f| f.routes.iter().cloned())
            .collect();
        self.plugins = files
            .clone()
            .flat_map(|f| f.plugins.iter().cloned())
            .collect();
        let mut shapes: Vec<String> = files.flat_map(|f| f.route_shapes.iter().cloned()).collect();
        shapes.sort();
        shapes.dedup();
        self.route_shapes = shapes;
    }
}

/// Whether `text` matches `shape`, where each `*` stands for a non-empty run.
fn glob(shape: &str, text: &str) -> bool {
    let Some((head, tail)) = shape.split_once('*') else {
        return shape == text;
    };
    let Some(after) = text.strip_prefix(head) else {
        return false;
    };
    (1..=after.len())
        .filter(|i| after.is_char_boundary(*i))
        .any(|i| glob(tail, &after[i..]))
}
