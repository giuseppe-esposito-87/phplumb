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
use crate::metadata::MetaValue;
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

/// Methods whose argument 0 is a template name, by the class that declares them.
const TEMPLATE_METHODS: [(&str, &[&str]); 2] = [
    (
        "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController",
        &[
            "render",
            "renderView",
            "renderBlock",
            "renderBlockView",
            "stream",
        ],
    ),
    ("Twig\\Environment", &["render", "display", "load"]),
];

/// The attribute that names a controller method's template.
const TEMPLATE_ATTRIBUTE: &str = "Symfony\\Bridge\\Twig\\Attribute\\Template";

/// Whether `call` targets one of `methods`, by declaring class.
fn calls_one_of(index: &Index, call: &FrameworkCall<'_>, methods: &[(&str, &[&str])]) -> bool {
    call.targets.iter().any(|target| {
        let CallTarget::Method { class, name } = target else {
            return false;
        };
        methods.iter().any(|(owner, names)| {
            names.iter().any(|m| m.eq_ignore_ascii_case(name)) && is_a(index, class, &[owner])
        })
    })
}

/// A file target at its start.
fn file_start(path: PathBuf) -> (Location, String) {
    (
        Location {
            path,
            start: 0,
            end: 0,
        },
        String::new(),
    )
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
        if argument != 0 {
            return Vec::new();
        }
        if calls_one_of(index, call, &TEMPLATE_METHODS) {
            return self
                .template_file(value)
                .into_iter()
                .map(file_start)
                .collect();
        }
        if !calls_one_of(index, call, &ROUTE_METHODS) {
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

    /// In a Twig template, a template name opens its file, and a route name in
    /// `path()` or `url()` leads to the route.
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
        if let Some((name, _, _)) = twig::template_name_at(source, offset) {
            return self
                .template_file(&name)
                .into_iter()
                .map(file_start)
                .collect();
        }
        let Some((name, _, _)) = twig::route_name_at(source, offset) else {
            return Vec::new();
        };
        self.route_targets(&name)
            .iter()
            .map(|target| (target.location.clone(), target.text.clone()))
            .collect()
    }

    /// The template of a `#[Template]` attribute under the cursor opens its file.
    fn declaration_targets(
        &self,
        _index: &Index,
        declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<(Location, String)> {
        let FrameworkDeclaration::Method { member, offset, .. } = declaration else {
            return Vec::new();
        };
        member
            .metadata
            .iter()
            .filter(|m| {
                m.name.eq_ignore_ascii_case(TEMPLATE_ATTRIBUTE)
                    && m.span.0 <= *offset
                    && *offset <= m.span.1
            })
            .filter_map(|m| {
                match m.argument("template").or_else(|| {
                    m.arguments
                        .first()
                        .filter(|(key, _)| key.is_none())
                        .map(|(_, v)| v)
                }) {
                    Some(MetaValue::String(name)) => self.template_file(name),
                    _ => None,
                }
            })
            .map(file_start)
            .collect()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
