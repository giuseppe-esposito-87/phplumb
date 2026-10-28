//! Symfony: the module that answers for a project built on the Symfony framework.

use std::any::Any;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::frameworks::{Framework, FrameworkCall, FrameworkDeclaration, Project};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;
use crate::summary::NameRef;

#[derive(Default)]
pub struct Symfony {
    project: Project,
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

    fn rebuild(&mut self, _index: &Index) {}

    fn update(&mut self, _index: &Index, _changed: &[PathBuf], _removed: &[PathBuf]) {}

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
