//! Navigation inside Drupal YAML files: the classes, methods, services and routes
//! that services, routing and links files reference.

use std::path::Path;

use super::Drupal;
use super::yaml::{self, Step, Yaml};
use crate::index::{Index, Location};
use crate::summary::MemberKind;

const LINKS: [&str; 4] = [
    ".links.menu.yml",
    ".links.task.yml",
    ".links.action.yml",
    ".links.contextual.yml",
];

/// The targets of the YAML scalar at `offset` in `source`, the content of `path`.
pub(super) fn targets(
    drupal: &Drupal,
    index: &Index,
    path: &Path,
    source: &[u8],
    offset: u32,
) -> Vec<(Location, String)> {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return Vec::new();
    };
    let Ok(text) = std::str::from_utf8(source) else {
        return Vec::new();
    };
    let Some(root) = yaml::parse(text) else {
        return Vec::new();
    };
    let Some((steps, scalar)) = yaml::scalar_at(&root, offset) else {
        return Vec::new();
    };
    let Some(value) = scalar.text() else {
        return Vec::new();
    };
    if name.ends_with(".services.yml") {
        services(drupal, index, &root, &steps, value)
    } else if name.ends_with(".routing.yml") {
        routing(drupal, index, &steps, value)
    } else if LINKS.iter().any(|suffix| name.ends_with(suffix)) {
        links(drupal, index, &steps, value)
    } else {
        Vec::new()
    }
}

fn services(
    drupal: &Drupal,
    index: &Index,
    root: &Yaml,
    steps: &[Step<'_>],
    value: &str,
) -> Vec<(Location, String)> {
    if let Some(id) = value.strip_prefix('@') {
        return service_targets(drupal, id.trim_start_matches('?'));
    }
    match steps {
        [Step::Key("services"), Step::KeyItself]
        | [Step::Key("services"), Step::Key(_), Step::Key("class")]
        | [
            Step::Key("services"),
            Step::Key(_),
            Step::Key("factory"),
            Step::Index(0),
        ] => class_targets(index, value),
        [
            Step::Key("services"),
            Step::Key(_),
            Step::Key("parent" | "decorates" | "alias"),
        ] => service_targets(drupal, value),
        [Step::Key("services"), Step::Key(_), Step::Key("factory")] => {
            callable_targets(index, value)
        }
        [
            Step::Key("services"),
            Step::Key(id),
            Step::Key("factory"),
            Step::Index(1),
        ] => {
            let Some(Yaml::Sequence(items)) = root
                .get("services")
                .and_then(|services| services.get(id))
                .and_then(|service| service.get("factory"))
            else {
                return Vec::new();
            };
            let classes = match items.first().and_then(Yaml::text) {
                Some(first) => match first.strip_prefix('@') {
                    Some(service) => own_classes(drupal, index, service),
                    None => vec![first.trim_start_matches('\\').to_string()],
                },
                None => Vec::new(),
            };
            method_targets(index, &classes, value)
        }
        [
            Step::Key("services"),
            Step::Key(id),
            Step::Key("calls"),
            Step::Index(_),
            Step::Index(0),
        ] => method_targets(index, &own_classes(drupal, index, id), value),
        _ => Vec::new(),
    }
}

fn routing(
    drupal: &Drupal,
    index: &Index,
    steps: &[Step<'_>],
    value: &str,
) -> Vec<(Location, String)> {
    match steps {
        [
            Step::Key(_),
            Step::Key("defaults"),
            Step::Key("_controller" | "_title_callback"),
        ]
        | [
            Step::Key(_),
            Step::Key("requirements"),
            Step::Key("_custom_access"),
        ] => match value.split_once("::") {
            Some(_) => callable_targets(index, value),
            None => match value.split_once(':') {
                Some((service, method)) => {
                    method_targets(index, &own_classes(drupal, index, service), method)
                }
                // An invokable controller class.
                None => class_targets(index, value),
            },
        },
        [Step::Key("route_callbacks"), Step::Index(_)] => callable_targets(index, value),
        [Step::Key(_), Step::Key("defaults"), Step::Key("_form")] => class_targets(index, value),
        _ => Vec::new(),
    }
}

fn links(
    drupal: &Drupal,
    index: &Index,
    steps: &[Step<'_>],
    value: &str,
) -> Vec<(Location, String)> {
    match steps {
        [Step::Key(_), Step::Key("route_name" | "base_route")]
        | [Step::Key(_), Step::Key("appears_on"), Step::Index(_)] => drupal.routes.targets(value),
        [Step::Key(_), Step::Key("class" | "deriver")] => class_targets(index, value),
        _ => Vec::new(),
    }
}

/// The classes a service's own definitions declare, or its id when it names a
/// class.
fn own_classes(drupal: &Drupal, index: &Index, id: &str) -> Vec<String> {
    let declared: Vec<String> = drupal
        .services(id)
        .iter()
        .filter_map(|definition| definition.class.clone())
        .collect();
    if declared.is_empty() && index.has_class(id) {
        return vec![id.to_string()];
    }
    declared
}

/// Every declaration of the class `name`, with its short name as the expected text.
fn class_targets(index: &Index, name: &str) -> Vec<(Location, String)> {
    index
        .classes_named(name.trim_start_matches('\\'))
        .into_iter()
        .map(|class| {
            let declaration = class.declaration;
            let short = declaration
                .fqn
                .rsplit('\\')
                .next()
                .unwrap_or(&declaration.fqn);
            let location = Location {
                path: class.path.to_path_buf(),
                start: declaration.name_start,
                end: declaration.name_end,
            };
            (location, short.to_string())
        })
        .collect()
}

/// The method of `Class::method`.
fn callable_targets(index: &Index, callable: &str) -> Vec<(Location, String)> {
    match callable.split_once("::") {
        Some((class, method)) => {
            method_targets(index, &[class.trim_start_matches('\\').to_string()], method)
        }
        None => Vec::new(),
    }
}

/// The nearest declaration of `method` on each class, with its name as the
/// expected text.
fn method_targets(index: &Index, classes: &[String], method: &str) -> Vec<(Location, String)> {
    let mut found: Vec<(Location, String)> = Vec::new();
    for class in classes {
        let Some((declaring, member)) = index
            .find_member(class, MemberKind::Method, method)
            .into_iter()
            .next()
        else {
            continue;
        };
        let location = Location {
            path: declaring.path.to_path_buf(),
            start: member.name_start,
            end: member.name_end,
        };
        if !found.iter().any(|(l, _)| *l == location) {
            found.push((location, member.name.clone()));
        }
    }
    found
}

/// Every definition of the service `id`, with the id as the expected text.
fn service_targets(drupal: &Drupal, id: &str) -> Vec<(Location, String)> {
    drupal
        .services(id)
        .iter()
        .map(|definition| {
            let location = Location {
                path: definition.path.clone(),
                start: definition.start,
                end: definition.end,
            };
            (location, id.to_string())
        })
        .collect()
}
