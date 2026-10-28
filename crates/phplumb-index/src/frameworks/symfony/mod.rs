//! Symfony: the module that answers for a project built on the Symfony framework.

mod routes;
mod templates;
mod twig;

use std::any::Any;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::frameworks::ancestry::is_a;
use crate::frameworks::{CallTarget, Framework, FrameworkCall, FrameworkDeclaration, Project};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;
use crate::summary::NameRef;

pub use routes::RouteTarget;
use routes::Routes;
use templates::Templates;

/// Methods whose argument 0 is a route name, by the class that declares them.
const ROUTE_METHODS: [(&str, &[&str]); 2] = [
    (
        "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController",
        &["redirectToRoute", "generateUrl"],
    ),
    (
        "Symfony\\Component\\Routing\\Generator\\UrlGeneratorInterface",
        &["generate"],
    ),
];

/// Whether `call` passes a route name as its argument 0.
fn route_call(index: &Index, call: &FrameworkCall<'_>) -> bool {
    call.targets.iter().any(|target| {
        let CallTarget::Method { class, name } = target else {
            return false;
        };
        ROUTE_METHODS.iter().any(|(owner, methods)| {
            methods.iter().any(|m| m.eq_ignore_ascii_case(name)) && is_a(index, class, &[owner])
        })
    })
}

#[derive(Default)]
pub struct Symfony {
    project: Project,
    routes: Routes,
    templates: Templates,
}

impl Symfony {
    /// Where the route `name` is defined: an attribute route's controller method, or
    /// a YAML route's key and controller method.
    pub fn route_targets(&self, name: &str) -> &[RouteTarget] {
        self.routes.targets(name)
    }

    /// The file the template `name` resolves to, as Twig's loader finds it.
    pub fn template_file(&self, name: &str) -> Option<PathBuf> {
        self.templates.file(name)
    }

    fn read(&mut self, index: &Index) {
        let filter = index.filter().scoped(&self.project);
        self.routes = Routes::read(index, &filter, &self.project);
        self.templates = Templates::read(&self.project);
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

    /// A route name given to a route method leads to the route's definitions.
    fn string_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<(Location, String)> {
        if argument != 0 || !route_call(index, call) {
            return Vec::new();
        }
        self.route_targets(value)
            .iter()
            .map(|target| (target.location.clone(), target.text.clone()))
            .collect()
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

    /// A route name in `path()` or `url()` of a Twig template leads to the route.
    fn file_targets(
        &self,
        _index: &Index,
        path: &Path,
        source: &[u8],
        offset: u32,
    ) -> Vec<(Location, String)> {
        if !crate::filter::is_twig(path) {
            return Vec::new();
        }
        let Some((name, _, _)) = twig::route_name_at(source, offset) else {
            return Vec::new();
        };
        self.route_targets(&name)
            .iter()
            .map(|target| (target.location.clone(), target.text.clone()))
            .collect()
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
