//! Symfony: the module that answers for a project built on the Symfony framework.

mod routes;

use std::any::Any;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::frameworks::{Framework, FrameworkCall, FrameworkDeclaration, Project};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;
use crate::summary::NameRef;

pub use routes::RouteTarget;
use routes::Routes;

#[derive(Default)]
pub struct Symfony {
    project: Project,
    routes: Routes,
}

impl Symfony {
    /// Where the route `name` is defined: an attribute route's controller method, or
    /// a YAML route's key and controller method.
    pub fn route_targets(&self, name: &str) -> &[RouteTarget] {
        self.routes.targets(name)
    }

    fn read(&mut self, index: &Index) {
        let filter = index.filter().scoped(&self.project);
        self.routes = Routes::read(index, &filter, &self.project);
    }
}

impl Framework for Symfony {
    fn name(&self) -> &'static str {
        "symfony"
    }

    fn package(&self) -> &'static str {
        "symfony/framework-bundle"
    }

    fn project(&self) -> &Project {
        &self.project
    }

    fn set_project(&mut self, project: Project) {
        self.project = project;
    }

    fn dependency_directories(&self) -> &'static [&'static str] {
        &[]
    }

    fn php_extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn rebuild(&mut self, index: &Index) {
        self.read(index);
    }

    /// Reads the routes again when a PHP or YAML file of the project, or a
    /// directory, changed.
    fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]) {
        let relevant = changed.iter().chain(removed).any(|path| {
            let path = index.filter().under_root(path);
            let kind = path.extension().and_then(|e| e.to_str());
            (self.project.contains(&path) || self.project.dir.starts_with(&path))
                && (path.is_dir() || matches!(kind, None | Some("php" | "yaml" | "yml")))
        });
        if relevant {
            self.read(index);
        }
    }

    fn call_type(&self, _index: &Index, _call: &FrameworkCall<'_>) -> Option<TypeExpr> {
        None
    }

    fn string_targets(
        &self,
        _index: &Index,
        _call: &FrameworkCall<'_>,
        _argument: usize,
        _value: &str,
    ) -> Vec<(Location, String)> {
        Vec::new()
    }

    fn keyed_string_targets(
        &self,
        _index: &Index,
        _key: &str,
        _value: &str,
        _call: Option<(&FrameworkCall<'_>, usize)>,
    ) -> Vec<(Location, String)> {
        Vec::new()
    }

    fn diagnostics(&self, _index: &Index, _path: &Path, _source: &[u8]) -> Vec<Diagnostic> {
        Vec::new()
    }

    fn class_implementations(&self, _index: &Index, _class: &str) -> Vec<(Location, String)> {
        Vec::new()
    }

    fn annotation_classes(
        &self,
        _index: &Index,
        _class: &str,
        _name: &NameRef,
    ) -> Option<Vec<String>> {
        None
    }

    fn file_targets(
        &self,
        _index: &Index,
        _path: &Path,
        _source: &[u8],
        _offset: u32,
    ) -> Vec<(Location, String)> {
        Vec::new()
    }

    fn declaration_targets(
        &self,
        _index: &Index,
        _declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<(Location, String)> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
