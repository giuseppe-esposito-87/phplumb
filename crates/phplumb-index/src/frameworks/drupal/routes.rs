//! Drupal routes: every route of every `*.routing.yml` on disk outside `vendor` and
//! test modules.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::services::{drupal_files, is_drupal_file};
use super::yaml::{self, Yaml};
use crate::discover::chosen_path;
use crate::filter::PathFilter;

const ROUTING: &str = ".routing.yml";

/// One definition of a route: where its key is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteDefinition {
    pub name: String,
    pub path: PathBuf,
    pub start: u32,
    pub end: u32,
}

#[derive(Default)]
pub(crate) struct Routes {
    by_file: HashMap<PathBuf, Vec<RouteDefinition>>,
    by_name: HashMap<String, Vec<RouteDefinition>>,
}

impl Routes {
    pub(crate) fn read(filter: &PathFilter) -> Routes {
        let mut routes = Routes::default();
        for path in drupal_files(filter, filter.root(), ROUTING) {
            routes.by_file.insert(path.clone(), parse_file(&path));
        }
        routes.reindex();
        routes
    }

    /// Re-reads changed routing files and directories; drops removed ones.
    pub(crate) fn update(&mut self, filter: &PathFilter, changed: &[PathBuf], removed: &[PathBuf]) {
        for path in removed.iter().chain(changed) {
            self.by_file.retain(|file, _| !file.starts_with(path));
        }
        for path in changed {
            let files = if path.is_dir() {
                drupal_files(filter, path, ROUTING)
            } else {
                chosen_path(filter.root(), path)
                    .map(|(_, chosen)| chosen)
                    .filter(|chosen| is_drupal_file(filter, chosen, ROUTING) && chosen.is_file())
                    .into_iter()
                    .collect()
            };
            for file in files {
                self.by_file.insert(file.clone(), parse_file(&file));
            }
        }
        self.reindex();
    }

    fn reindex(&mut self) {
        let mut by_name: HashMap<String, Vec<RouteDefinition>> = HashMap::new();
        for definition in self.by_file.values().flatten() {
            by_name
                .entry(definition.name.clone())
                .or_default()
                .push(definition.clone());
        }
        for definitions in by_name.values_mut() {
            definitions.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
        }
        self.by_name = by_name;
    }

    pub(crate) fn get(&self, name: &str) -> &[RouteDefinition] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }
}

/// The routes of one file; nothing when it does not parse.
fn parse_file(path: &Path) -> Vec<RouteDefinition> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Some(Yaml::Map(entries)) = yaml::parse(&text) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(key, _)| {
            let Yaml::Scalar { value, start, end } = key else {
                return None;
            };
            if value == "route_callbacks" {
                return None;
            }
            let (start, end) = yaml::value_range(&text, value, *start, *end);
            Some(RouteDefinition {
                name: value.clone(),
                path: path.to_path_buf(),
                start,
                end,
            })
        })
        .collect()
}
