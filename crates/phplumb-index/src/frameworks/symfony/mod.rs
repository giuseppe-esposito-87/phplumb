//! Symfony: the module that answers for a project built on the Symfony framework.

mod config;
mod doctrine;
mod messenger;
mod routes;
mod templates;
mod twig;

use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::diagnostics::Diagnostic;
use crate::frameworks::ancestry::is_a;
use crate::frameworks::yaml::{self, Yaml};
use crate::frameworks::{
    AttributeString, CallTarget, Framework, FrameworkCall, FrameworkDeclaration, Project,
};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;
use crate::summary::NameRef;

use doctrine::Mappings;
use messenger::Handlers;
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

/// The method whose argument 0 is a parameter name.
const PARAMETER_METHODS: [(&str, &[&str]); 1] = [(
    "Symfony\\Bundle\\FrameworkBundle\\Controller\\AbstractController",
    &["getParameter"],
)];

/// The bus method whose argument 0 is a message.
const DISPATCH: [(&str, &[&str]); 1] = [(
    "Symfony\\Component\\Messenger\\MessageBusInterface",
    &["dispatch"],
)];

/// The attribute that injects a parameter, service or value into an argument.
const AUTOWIRE_ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Autowire";

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

/// The declarations of the class `fqn`, at their names.
fn class_targets(index: &Index, fqn: &str) -> Vec<(Location, String)> {
    let fqn = fqn.trim_start_matches('\\');
    let short = fqn.rsplit('\\').next().unwrap_or(fqn).to_string();
    index
        .classes_named(fqn)
        .iter()
        .map(|class| {
            (
                Location {
                    path: class.path.to_path_buf(),
                    start: class.declaration.name_start,
                    end: class.declaration.name_end,
                },
                short.clone(),
            )
        })
        .collect()
}

/// The declaration of property `name` on `class` or its parents, at its name.
fn property_targets(index: &Index, class: &str, name: &str) -> Vec<(Location, String)> {
    index
        .find_member(class, crate::summary::MemberKind::Property, name)
        .into_iter()
        .map(|(found, member)| {
            (
                Location {
                    path: found.path.to_path_buf(),
                    start: member.name_start,
                    end: member.name_end,
                },
                member.name.clone(),
            )
        })
        .collect()
}

/// Targets as `(location, expected text)` pairs.
fn targets_of(targets: &[RouteTarget]) -> Vec<(Location, String)> {
    targets
        .iter()
        .map(|target| (target.location.clone(), target.text.clone()))
        .collect()
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
    handlers: Handlers,
    /// Parameter definitions, by name.
    parameters: HashMap<String, Vec<RouteTarget>>,
    /// Service definitions, by id.
    services: HashMap<String, Vec<RouteTarget>>,
    mappings: Mappings,
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

    /// The handler methods of the message class `message`, at their names.
    pub fn handlers_of(&self, index: &Index, message: &str) -> Vec<Location> {
        self.handlers
            .of(index, message)
            .into_iter()
            .map(|(location, _)| location)
            .collect()
    }

    /// The message classes the handler method named at `start` in `path` handles.
    pub fn handled_by(&self, path: &Path, start: u32) -> Vec<String> {
        self.handlers.handled_by(path, start)
    }

    /// The `dispatch()` calls of the project whose message is, extends or
    /// implements one of `handled`, at their method name.
    fn dispatches(&self, index: &Index, handled: &[String]) -> Vec<(Location, String)> {
        let handled: Vec<&str> = handled.iter().map(String::as_str).collect();
        index
            .calls_named(&["dispatch"])
            .into_iter()
            .filter(|call| self.project.contains(&call.location.path))
            .filter(|call| {
                let found = FrameworkCall {
                    targets: &call.targets,
                    strings: &call.strings,
                    receiver: Some(&call.receiver),
                };
                calls_one_of(index, &found, &DISPATCH)
                    && call.arguments.first().is_some_and(|message| {
                        crate::infer::class_names(message)
                            .iter()
                            .any(|class| is_a(index, class, &handled))
                    })
            })
            .map(|call| (call.location, "dispatch".to_string()))
            .collect()
    }

    /// Where the parameter `name` is defined under `parameters:`.
    pub fn parameter_targets(&self, name: &str) -> &[RouteTarget] {
        self.parameters.get(name).map_or(&[], Vec::as_slice)
    }

    /// The definition of the service `id`, or the class when the id names one that no
    /// key defines.
    pub fn service_targets(&self, index: &Index, id: &str) -> Vec<(Location, String)> {
        match self.services.get(id) {
            Some(targets) => targets_of(targets),
            None if config::looks_like_class(id) => class_targets(index, id),
            None => Vec::new(),
        }
    }

    /// In a YAML configuration file: a `%parameter%`, an `@id` service reference, or a
    /// class name under `offset`.
    fn yaml_targets(
        &self,
        index: &Index,
        path: &Path,
        source: &[u8],
        offset: u32,
    ) -> Vec<(Location, String)> {
        // Elsewhere, such as in translations, `%name%` is a placeholder.
        let config = self.project.dir.join("config");
        if !index.filter().under_root(path).starts_with(&config) {
            return Vec::new();
        }
        let Ok(text) = std::str::from_utf8(source) else {
            return Vec::new();
        };
        let Some(root) = yaml::parse(text) else {
            return Vec::new();
        };
        let Some((steps, Yaml::Scalar { value, start, end })) = yaml::scalar_at(&root, offset)
        else {
            return Vec::new();
        };
        let in_services = steps.contains(&yaml::Step::Key("services"));
        let names_a_service = matches!(
            steps.last(),
            Some(yaml::Step::Key("alias" | "decorates" | "parent"))
        );
        if in_services && names_a_service {
            return self.service_targets(index, value);
        }
        let raw = &text[*start as usize..*end as usize];
        if let Some(name) = config::parameter_at(raw, (offset - start) as usize) {
            return targets_of(self.parameter_targets(&name));
        }
        if let Some(reference) = value.strip_prefix('@') {
            // `@?id` is optional, `@=` an expression and `@@` an escaped `@`.
            if reference.starts_with('=') || reference.starts_with('@') {
                return Vec::new();
            }
            return self.service_targets(index, reference.trim_start_matches('?'));
        }
        if config::looks_like_class(value) {
            return class_targets(index, value);
        }
        Vec::new()
    }

    /// The repository class the Doctrine mappings declare for `entity`.
    pub fn repository_of(&self, entity: &str) -> Option<&str> {
        self.mappings.repository_of(entity)
    }

    /// The `%name%` under the cursor in the value of an attribute.
    fn value_parameter_targets(
        &self,
        found: &AttributeString,
        offset: u32,
    ) -> Vec<(Location, String)> {
        config::parameter_at(&found.value, offset.saturating_sub(found.start) as usize)
            .map(|name| targets_of(self.parameter_targets(&name)))
            .unwrap_or_default()
    }

    fn read(&mut self, index: &Index) {
        let filter = index.filter().scoped(&self.project);
        let files = config::config_files(&self.project, &filter);
        self.parameters = config::read_parameters(&files);
        self.services = config::read_services(&files);
        self.mappings = Mappings::read(index, &filter, &self.project.dir);
        self.routes = Routes::read(index, &filter, &self.project);
        self.templates = Templates::read(&self.project);
        self.handlers = Handlers::read(index, &filter);
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

    /// Reads the routes, template paths, message handlers, configuration and Doctrine
    /// mappings again when a PHP, YAML or XML file of the project, or a directory,
    /// changed.
    fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]) {
        let relevant = changed.iter().chain(removed).any(|path| {
            let path = index.filter().under_root(path);
            let kind = path.extension().and_then(|e| e.to_str());
            (self.project.contains(&path) || self.project.dir.starts_with(&path))
                && (path.is_dir() || matches!(kind, None | Some("php" | "yaml" | "yml" | "xml")))
        });
        if relevant {
            self.read(index);
        }
    }

    fn call_type(&self, _index: &Index, _call: &FrameworkCall<'_>) -> Option<TypeExpr> {
        None
    }

    /// A template name given to a rendering method opens the template; a route name
    /// given to a route method leads to the route's definitions.
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
        if calls_one_of(index, call, &PARAMETER_METHODS) {
            return targets_of(self.parameter_targets(value));
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

    /// A parameter or service in `#[Autowire]` leads to its definitions; a template
    /// in `#[Template]` opens its file.
    fn attribute_targets(
        &self,
        index: &Index,
        found: &AttributeString,
        offset: u32,
    ) -> Vec<(Location, String)> {
        let first = found.key.is_none() && found.position == 0;
        if found.attribute.eq_ignore_ascii_case(AUTOWIRE_ATTRIBUTE) {
            return match found.key.as_deref() {
                Some("param") => targets_of(self.parameter_targets(&found.value)),
                Some("service") => self.service_targets(index, &found.value),
                Some("value") => self.value_parameter_targets(found, offset),
                None if first => self.value_parameter_targets(found, offset),
                _ => Vec::new(),
            };
        }
        let template = found.key.as_deref() == Some("template") || first;
        if found.attribute.eq_ignore_ascii_case(TEMPLATE_ATTRIBUTE) && template {
            return self
                .template_file(&found.value)
                .into_iter()
                .map(file_start)
                .collect();
        }
        Vec::new()
    }

    /// A `dispatch()` on a message bus lists the handlers of its message.
    fn call_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        arguments: &[TypeExpr],
    ) -> Vec<(Location, String)> {
        if !calls_one_of(index, call, &DISPATCH) {
            return Vec::new();
        }
        let Some(message) = arguments.first() else {
            return Vec::new();
        };
        let mut found: Vec<(Location, String)> = Vec::new();
        for class in crate::infer::class_names(message) {
            for (location, name) in self.handlers.of(index, &class) {
                if !found.iter().any(|(known, _)| *known == location) {
                    found.push((location, name));
                }
            }
        }
        found
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

    /// In YAML configuration, a parameter, a service reference or a class name leads
    /// to its definition; in a Doctrine mapping, a class or property name opens it;
    /// in a Twig template, a template name opens its file, and a route name in
    /// `path()` or `url()` leads to the route.
    fn file_targets(
        &self,
        index: &Index,
        path: &Path,
        source: &[u8],
        offset: u32,
    ) -> Vec<(Location, String)> {
        if crate::filter::is_yaml(path) {
            return self.yaml_targets(index, path, source, offset);
        }
        if crate::filter::is_doctrine_xml(path) {
            let Ok(text) = std::str::from_utf8(source) else {
                return Vec::new();
            };
            return match doctrine::named_at(text, offset) {
                Some(doctrine::Named::Class(class)) => class_targets(index, &class),
                Some(doctrine::Named::Property(class, property)) => {
                    property_targets(index, &class, &property)
                }
                None => Vec::new(),
            };
        }
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

    /// A message handler, by its method or its `#[AsMessageHandler]`, lists the
    /// dispatches of its messages.
    fn declaration_targets(
        &self,
        index: &Index,
        declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<(Location, String)> {
        let handled = match declaration {
            FrameworkDeclaration::Method { path, member, .. } => {
                self.handled_by(path, member.name_start)
            }
            FrameworkDeclaration::Class {
                path,
                class,
                metadata,
            } => messenger::handler_method(class, metadata)
                .map(|method| self.handled_by(path, method.name_start))
                .unwrap_or_default(),
            FrameworkDeclaration::Function { .. } => Vec::new(),
        };
        if handled.is_empty() {
            return Vec::new();
        }
        self.dispatches(index, &handled)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
