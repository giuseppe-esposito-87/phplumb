//! Drupal routes: every route of every `*.routing.yml` on disk outside `vendor` and
//! test modules.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::is_a;
use super::services::{Tests, drupal_files, in_tests, is_drupal_file};
use super::yaml::{self, Yaml};
use crate::discover::chosen_path;
use crate::filter::PathFilter;
use crate::frameworks::{CallTarget, FrameworkCall};
use crate::index::{Index, Location};

const ROUTING: &str = ".routing.yml";

/// Methods taking a route name, with the class declaring them and the position of
/// the name.
const ROUTE_CALLS: [(&str, &str, usize); 7] = [
    ("Drupal\\Core\\Url", "fromRoute", 0),
    ("Drupal\\Core\\Link", "createFromRoute", 1),
    ("Drupal\\Core\\Form\\FormStateInterface", "setRedirect", 0),
    ("Drupal\\Core\\Controller\\ControllerBase", "redirect", 0),
    ("Drupal\\Core\\Form\\FormBase", "redirect", 0),
    (
        "Drupal\\Core\\Routing\\UrlGeneratorInterface",
        "generateFromRoute",
        0,
    ),
    (
        "Drupal\\Core\\Access\\AccessManagerInterface",
        "checkNamedRoute",
        0,
    ),
];

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
    /// Routes outside test modules, which navigation uses.
    by_name: HashMap<String, Vec<RouteDefinition>>,
    /// Every route name defined, test modules included.
    defined: HashSet<String>,
    root: PathBuf,
}

impl Routes {
    pub(crate) fn read(filter: &PathFilter) -> Routes {
        let mut routes = Routes {
            root: filter.root().to_path_buf(),
            ..Routes::default()
        };
        for path in drupal_files(filter, filter.root(), ROUTING, Tests::Include) {
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
                drupal_files(filter, path, ROUTING, Tests::Include)
            } else {
                chosen_path(filter.root(), path)
                    .map(|(_, chosen)| chosen)
                    .filter(|chosen| {
                        is_drupal_file(filter, chosen, ROUTING, Tests::Include) && chosen.is_file()
                    })
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
        self.defined = self
            .by_file
            .values()
            .flatten()
            .map(|r| r.name.clone())
            .collect();
        let mut by_name: HashMap<String, Vec<RouteDefinition>> = HashMap::new();
        for definition in self
            .by_file
            .iter()
            .filter(|(file, _)| !in_tests(&self.root, file))
            .flat_map(|(_, definitions)| definitions)
        {
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

    /// Whether some routing file defines `name`, test modules included.
    pub(crate) fn defines(&self, name: &str) -> bool {
        self.defined.contains(name)
    }

    pub(crate) fn get(&self, name: &str) -> &[RouteDefinition] {
        self.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    /// The route keys a route name at position `argument` of `call` names, each with
    /// the route name as the expected text.
    pub(crate) fn call_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<(Location, String)> {
        if !takes_route(index, call.targets, argument) {
            return Vec::new();
        }
        self.targets(value)
    }

    /// Every definition of the route `name`, with the name as the expected text.
    pub(crate) fn targets(&self, name: &str) -> Vec<(Location, String)> {
        self.get(name)
            .iter()
            .map(|route| {
                let location = Location {
                    path: route.path.clone(),
                    start: route.start,
                    end: route.end,
                };
                (location, route.name.clone())
            })
            .collect()
    }
}

/// Whether position `argument` of a call to `targets` takes a route name.
pub(crate) fn takes_route(index: &Index, targets: &[CallTarget], argument: usize) -> bool {
    targets.iter().any(|target| {
        let CallTarget::Method { class, name } = target else {
            return false;
        };
        ROUTE_CALLS.iter().any(|(owner, method, position)| {
            *position == argument
                && name.eq_ignore_ascii_case(method)
                && is_a(index, class, &[owner])
        })
    })
}

/// The names of the methods taking a route name.
pub(crate) fn route_methods() -> Vec<&'static str> {
    ROUTE_CALLS.iter().map(|(_, method, _)| *method).collect()
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
