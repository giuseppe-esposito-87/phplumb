//! Framework modules: what a framework adds on top of PHP, such as service
//! containers configured in YAML. Core navigation reaches them only through
//! `Framework` and `registered`.

pub mod drupal;

use std::any::Any;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;

/// Whether a framework module runs: detected from the project, or forced.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Activation {
    #[default]
    Auto,
    On,
    Off,
}

/// One activation per framework module.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct FrameworkSettings {
    pub drupal: Activation,
}

/// What a framework sees of a call: what it targets and its literal arguments.
pub struct FrameworkCall<'a> {
    /// `Class::method` for every declaration the call resolves to, or the function.
    pub targets: &'a [CallTarget],
    /// Positional arguments that are string literals or `X::class`, by position.
    pub strings: &'a [Option<String>],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallTarget {
    Method { class: String, name: String },
    Function(String),
}

pub trait Framework: Send + Sync {
    /// The name shown in the log.
    fn name(&self) -> &'static str;
    /// Whether the framework applies to the project at `root`.
    fn detect(&self, root: &Path) -> bool;
    /// Reads what the framework needs from the index and its own files, after the
    /// index is built.
    fn rebuild(&mut self, index: &Index);
    /// Applies changed and removed paths (files or directories, any extension),
    /// after the index applied them.
    fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]);
    /// The type `call` returns when the framework knows better than its declaration.
    fn call_type(&self, index: &Index, call: &FrameworkCall<'_>) -> Option<TypeExpr>;
    /// Targets of the string literal at position `argument` of `call`.
    fn string_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<Location>;
    /// The module itself, for code that needs its own API.
    fn as_any(&self) -> &dyn Any;
}

/// Every framework the engine knows, each with its setting.
pub fn registered(settings: &FrameworkSettings) -> Vec<(Activation, Box<dyn Framework>)> {
    vec![(settings.drupal, Box::new(drupal::Drupal::default()))]
}

/// The frameworks to run for the project at `root`.
pub(crate) fn active(root: &Path, settings: &FrameworkSettings) -> Vec<Box<dyn Framework>> {
    registered(settings)
        .into_iter()
        .filter(|(activation, framework)| match activation {
            Activation::On => true,
            Activation::Off => false,
            Activation::Auto => framework.detect(root),
        })
        .map(|(_, framework)| framework)
        .collect()
}
