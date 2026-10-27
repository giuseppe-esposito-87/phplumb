//! Drupal: services from `*.services.yml`.

use std::any::Any;
use std::path::{Path, PathBuf};

use crate::filter::PathFilter;
use crate::frameworks::{Framework, FrameworkCall};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;

#[derive(Default)]
pub struct Drupal {}

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

    fn rebuild(&mut self, _filter: &PathFilter) {}

    fn update(&mut self, _filter: &PathFilter, _changed: &[PathBuf], _removed: &[PathBuf]) {}

    fn call_type(&self, _index: &Index, _call: &FrameworkCall<'_>) -> Option<TypeExpr> {
        None
    }

    fn string_targets(
        &self,
        _index: &Index,
        _call: &FrameworkCall<'_>,
        _argument: usize,
        _value: &str,
    ) -> Vec<Location> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
