//! Framework modules: what a framework adds on top of PHP, such as service
//! containers configured in YAML. Core navigation reaches them only through
//! `Framework` and `registered`.

pub mod drupal;
mod project;
pub mod symfony;

use std::any::Any;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::diagnostics::Diagnostic;
use crate::filter::PHP_EXTENSION;
use crate::index::{Index, Location};
use crate::metadata::Metadata;
use crate::phpdoc::TypeExpr;
use crate::summary::{Declaration, Member, NameRef};

pub use project::Project;

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
    pub symfony: Activation,
}

/// What a framework sees of a call: what it targets and its literal arguments.
pub struct FrameworkCall<'a> {
    /// `Class::method` for every declaration the call resolves to, or the function.
    pub targets: &'a [CallTarget],
    /// Positional arguments that are string literals or `X::class`, by position.
    pub strings: &'a [Option<String>],
    /// The receiver's type, for method calls.
    pub receiver: Option<&'a TypeExpr>,
}

/// The declaration a definition request is on: its name, or one of its attributes.
pub enum FrameworkDeclaration<'a> {
    /// A function, with every declaration of its file.
    Function {
        path: &'a Path,
        declaration: &'a Declaration,
        file: &'a [Declaration],
    },
    Method {
        path: &'a Path,
        class: &'a Declaration,
        member: &'a Member,
    },
    /// One of the class's own attributes.
    Class {
        path: &'a Path,
        class: &'a Declaration,
        metadata: &'a Metadata,
    },
}

/// A method call found in the index, with its literal arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundCall {
    /// The method name at the call.
    pub location: Location,
    /// `Class::method` for every declaration the call resolves to.
    pub targets: Vec<CallTarget>,
    /// Positional arguments that are string literals or `X::class`, by position.
    pub strings: Vec<Option<String>>,
    /// The string literal elements of positional array arguments, by position.
    pub lists: Vec<Vec<String>>,
    /// The text shape of each positional argument, by position: a string's text
    /// with `*` for each part that is not literal, one per element of an array, or
    /// `*` for any other value.
    pub shapes: Vec<Vec<String>>,
    /// The byte range of each positional argument that is a string literal (inside
    /// its quotes) or `X::class`, by position.
    pub ranges: Vec<Option<(u32, u32)>>,
    /// The receiver's type; `Other` when unknown.
    pub receiver: TypeExpr,
    /// The method or function holding the call: `Class::method` with the declaring
    /// class's fully qualified name, or the function's fully qualified name.
    pub caller: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallTarget {
    Method { class: String, name: String },
    Function(String),
}

pub trait Framework: Send + Sync {
    /// The name shown in the log.
    fn name(&self) -> &'static str;
    /// The Composer package whose presence in a `composer.lock` marks a project.
    fn package(&self) -> &'static str;
    /// The part of the workspace the module reads and answers for.
    fn project(&self) -> &Project;
    fn set_project(&mut self, project: Project);
    /// Names of the directories holding code the framework installs rather than the
    /// project's own, nearest to project code first.
    fn dependency_directories(&self) -> &'static [&'static str];
    /// Extensions besides `php` of the PHP files the framework uses.
    fn php_extensions(&self) -> &'static [&'static str];
    /// Reads what the framework needs from the index and its own files, after the
    /// index is built. While `rebuild` and `update` run, `index.frameworks()` is
    /// empty, so a framework cannot query the others.
    fn rebuild(&mut self, index: &Index);
    /// Applies changed and removed paths (files or directories, any extension),
    /// after the index applied them.
    fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]);
    /// The type `call` returns when the framework knows better than its declaration.
    fn call_type(&self, index: &Index, call: &FrameworkCall<'_>) -> Option<TypeExpr>;
    /// Targets of the string literal at position `argument` of `call`, or of one of
    /// its elements when the argument is an array, each with the text expected at
    /// its range.
    fn string_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<(Location, String)>;
    /// Targets of a string literal that is the value of the array key `key`, each
    /// with the text expected at its range. `call` is the call whose positional
    /// argument at the given position holds the array, if any.
    fn keyed_string_targets(
        &self,
        index: &Index,
        key: &str,
        value: &str,
        call: Option<(&FrameworkCall<'_>, usize)>,
    ) -> Vec<(Location, String)>;
    /// Broken references the framework finds in the PHP file at `path`, whose
    /// current content is `source`.
    fn diagnostics(&self, index: &Index, path: &Path, source: &[u8]) -> Vec<Diagnostic>;
    /// Implementations the framework adds to the class-like `class`, each with the
    /// text expected at its range.
    fn class_implementations(&self, index: &Index, class: &str) -> Vec<(Location, String)>;
    /// The classes an unqualified annotation name no import resolves is, among the
    /// annotations of `class`, when the framework's discovery reads them; none when
    /// it does not.
    fn annotation_classes(&self, index: &Index, class: &str, name: &NameRef)
    -> Option<Vec<String>>;
    /// Targets at `offset` in a file the PHP engine does not read, whose current
    /// content is `source`, each with the text expected at its range.
    fn file_targets(
        &self,
        index: &Index,
        path: &Path,
        source: &[u8],
        offset: u32,
    ) -> Vec<(Location, String)>;
    /// Targets of a definition request on a declaration, each with the text expected
    /// at its range, when the framework knows what the declaration stands for.
    fn declaration_targets(
        &self,
        index: &Index,
        declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<(Location, String)>;
    /// The module itself, for code that needs its own API.
    fn as_any(&self) -> &dyn Any;
}

/// Every framework the engine knows, each with its setting.
pub fn registered(settings: &FrameworkSettings) -> Vec<(Activation, Box<dyn Framework>)> {
    vec![
        (settings.drupal, Box::new(drupal::Drupal::default())),
        (settings.symfony, Box::new(symfony::Symfony::default())),
    ]
}

/// Extensions of the files read as PHP in the project at `root`: `php`, then those
/// the active frameworks add.
pub fn php_extensions(root: &Path, settings: &FrameworkSettings) -> Vec<&'static str> {
    extensions(&active(root, settings))
}

/// `php`, then the extensions `frameworks` add.
pub(crate) fn extensions(frameworks: &[Box<dyn Framework>]) -> Vec<&'static str> {
    std::iter::once(PHP_EXTENSION)
        .chain(
            frameworks
                .iter()
                .flat_map(|framework| framework.php_extensions().iter().copied()),
        )
        .collect()
}

/// The frameworks to run for the workspace at `root`, each with its project: the
/// directory of its nearest `composer.lock`, or the root when forced on without one,
/// without the projects of the other active modules inside it.
pub(crate) fn active(root: &Path, settings: &FrameworkSettings) -> Vec<Box<dyn Framework>> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let found: Vec<(Box<dyn Framework>, PathBuf)> = registered(settings)
        .into_iter()
        .filter_map(|(activation, framework)| {
            let lock = project::nearest_lock(&root, framework.package());
            let dir = match activation {
                Activation::Off => return None,
                Activation::On => lock.unwrap_or_else(|| root.clone()),
                Activation::Auto => lock?,
            };
            Some((framework, dir))
        })
        .collect();
    let dirs: Vec<PathBuf> = found.iter().map(|(_, dir)| dir.clone()).collect();
    found
        .into_iter()
        .map(|(mut framework, dir)| {
            let excluded = dirs
                .iter()
                .filter(|other| **other != dir && other.starts_with(&dir))
                .cloned()
                .collect();
            framework.set_project(Project { dir, excluded });
            framework
        })
        .collect()
}
