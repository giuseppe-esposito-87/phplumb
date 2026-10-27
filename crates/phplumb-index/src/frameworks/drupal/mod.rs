//! Drupal: services from `*.services.yml`.

mod services;

use std::any::Any;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::filter::PathFilter;
use crate::frameworks::{CallTarget, Framework, FrameworkCall};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;

pub use services::ServiceDefinition;
use services::Services;

#[derive(Default)]
pub struct Drupal {
    services: Services,
}

impl Drupal {
    /// Every definition of `id` on disk.
    pub fn services(&self, id: &str) -> &[ServiceDefinition] {
        self.services.definitions(id)
    }

    /// The classes a service id can give, sorted: through aliases, parents, an id
    /// that names an indexed class, and every service decorating it. Empty when
    /// unknown.
    pub fn service_classes(&self, index: &Index, id: &str) -> Vec<String> {
        self.services.classes(index, id)
    }
}

impl Framework for Drupal {
    fn name(&self) -> &'static str {
        "drupal"
    }

    /// A Drupal project's `composer.lock` lists `drupal/core`.
    fn detect(&self, root: &Path) -> bool {
        let Ok(text) = std::fs::read(root.join("composer.lock")) else {
            return false;
        };
        let Ok(lock) = serde_json::from_slice::<serde_json::Value>(&text) else {
            return false;
        };
        ["packages", "packages-dev"].iter().any(|section| {
            lock[section]
                .as_array()
                .is_some_and(|packages| packages.iter().any(|p| p["name"] == "drupal/core"))
        })
    }

    fn rebuild(&mut self, filter: &PathFilter) {
        self.services = Services::read(filter);
    }

    fn update(&mut self, filter: &PathFilter, changed: &[PathBuf], removed: &[PathBuf]) {
        self.services.update(filter, changed, removed);
    }

    /// `\Drupal::service(id)` and a container's `get(id)` give the service's classes.
    fn call_type(&self, index: &Index, call: &FrameworkCall<'_>) -> Option<TypeExpr> {
        let id = service_call(index, call)?;
        let classes = self.services.classes(index, id);
        match classes.len() {
            0 => None,
            1 => classes.into_iter().next().map(TypeExpr::Named),
            _ => Some(TypeExpr::Union(
                classes.into_iter().map(TypeExpr::Named).collect(),
            )),
        }
    }

    /// The id of a service call leads to every definition of the service.
    fn string_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<Location> {
        if argument != 0 || service_call(index, call) != Some(value) {
            return Vec::new();
        }
        self.services
            .definitions(value)
            .iter()
            .map(|definition| Location {
                path: definition.path.clone(),
                start: definition.start,
                end: definition.end,
            })
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Container interfaces whose `get(id)` returns a service.
const CONTAINERS: [&str; 2] = [
    "Symfony\\Component\\DependencyInjection\\ContainerInterface",
    "Psr\\Container\\ContainerInterface",
];

/// The service id `call` asks for, when it is `\Drupal::service(id)` or a
/// container's `get(id)` with a literal id.
fn service_call<'a>(index: &Index, call: &'a FrameworkCall<'_>) -> Option<&'a str> {
    let id = call.strings.first()?.as_deref()?;
    let asks_for_service = call.targets.iter().any(|target| match target {
        CallTarget::Method { class, name } => {
            (class.eq_ignore_ascii_case("Drupal") && name.eq_ignore_ascii_case("service"))
                || (name.eq_ignore_ascii_case("get") && is_container(index, class))
        }
        CallTarget::Function(_) => false,
    });
    asks_for_service.then_some(id)
}

/// Whether `class` is or implements one of the `CONTAINERS`.
fn is_container(index: &Index, class: &str) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending = vec![class.to_string()];
    while let Some(next) = pending.pop() {
        if CONTAINERS.iter().any(|c| c.eq_ignore_ascii_case(&next)) {
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
