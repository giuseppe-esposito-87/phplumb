//! String literals of the PHP files that can define Drupal ids at run time: service
//! providers and compiler passes, route subscribers and route callbacks, and code
//! adding plugin definitions. Every literal there counts as a defined id, so an id
//! such code may define is never reported as undefined.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::index::Index;
use crate::infer;
use crate::summary::RefKey;

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

#[derive(Default)]
pub(crate) struct PhpLiterals {
    pub(crate) services: HashSet<String>,
    pub(crate) routes: HashSet<String>,
    pub(crate) plugins: HashSet<String>,
}

impl PhpLiterals {
    pub(crate) fn read(index: &Index) -> PhpLiterals {
        let classes = |names: &[&str]| -> Vec<RefKey> {
            names
                .iter()
                .map(|name| RefKey::Class(name.to_string()))
                .collect()
        };
        PhpLiterals {
            services: literals(index, &classes(&SERVICE_DEFINERS)),
            routes: literals(index, &classes(&ROUTE_DEFINERS)),
            plugins: literals(index, &[RefKey::Member("setDefinition".to_string())]),
        }
    }
}

/// Every string literal of the files mentioning one of `keys`.
fn literals(index: &Index, keys: &[RefKey]) -> HashSet<String> {
    let mut files: Vec<PathBuf> = keys
        .iter()
        .flat_map(|key| index.candidate_files(key))
        .map(|path| path.to_path_buf())
        .collect();
    files.sort();
    files.dedup();
    files
        .iter()
        .filter_map(|path| std::fs::read(path).ok())
        .flat_map(|source| infer::string_literals(&source))
        .collect()
}
