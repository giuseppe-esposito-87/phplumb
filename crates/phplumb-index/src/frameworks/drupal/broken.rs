//! Broken Drupal references: service ids, route names and plugin ids written in PHP
//! that are defined nowhere on disk. Any doubt stays silent.

use std::path::Path;

use super::routes::{route_methods, takes_route};
use super::{Drupal, is_a, plugins};
use crate::diagnostics::{Diagnostic, Severity};
use crate::frameworks::{CallTarget, FoundCall, FrameworkCall};
use crate::index::Index;
use crate::infer;
use crate::summary::{MemberKind, summarize};

/// Containers whose `get(id)` takes a service id. A plain PSR container may be any
/// other container.
const CONTAINERS: [&str; 2] = [
    "Symfony\\Component\\DependencyInjection\\ContainerInterface",
    "Drupal\\Component\\DependencyInjection\\ContainerInterface",
];

/// Routes Drupal defines without a routing file entry.
const SPECIAL_ROUTES: [&str; 5] = ["<front>", "<none>", "<nolink>", "<current>", "<button>"];

/// Prefixes of routes that entity route providers and views build at run time.
const GENERATED_ROUTES: [&str; 2] = ["entity.", "view."];

const DEFAULT_MANAGER: &str = "Drupal\\Core\\Plugin\\DefaultPluginManager";

pub(super) fn find(drupal: &Drupal, index: &Index, path: &Path, source: &[u8]) -> Vec<Diagnostic> {
    let summary = summarize(source);
    let mut names = vec![
        "service",
        "get",
        "has",
        "hasService",
        "hasDefinition",
        "createInstance",
        "getDefinition",
        "create",
        "addConstraint",
    ];
    names.extend(route_methods());
    let calls = infer::calls_named(index, path, &summary, source, &names);
    let guarded = |method: &str, id: &str| {
        calls.iter().any(|call| {
            call.strings.first().cloned().flatten().as_deref() == Some(id)
                && call.targets.iter().any(|t| matches!(t, CallTarget::Method { name, .. } if name.eq_ignore_ascii_case(method)))
        })
    };
    let mut found = Vec::new();
    for call in &calls {
        let framework = FrameworkCall {
            targets: &call.targets,
            strings: &call.strings,
            receiver: Some(&call.receiver),
        };
        if let Some((id, range)) = service_id(index, call)
            && !guarded("has", id)
            && !guarded("hasService", id)
            && !drupal.defines_service(index, id)
        {
            found.push(diagnostic(
                range,
                Severity::Error,
                format!("Undefined service '{id}'"),
            ));
        }
        for (position, name) in call.strings.iter().enumerate() {
            let (Some(name), Some(Some(range))) = (name, call.ranges.get(position)) else {
                continue;
            };
            if takes_route(index, &call.targets, position)
                && !SPECIAL_ROUTES.contains(&name.as_str())
                && !GENERATED_ROUTES
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
                && !drupal.defines_route(index, name)
            {
                found.push(diagnostic(
                    *range,
                    Severity::Error,
                    format!("Undefined route '{name}'"),
                ));
            }
            if let Some(manager) = drupal.plugins.call_manager(index, &framework, position)
                && let Some(message) = undefined_plugin(drupal, index, &manager, name, &guarded)
            {
                found.push(diagnostic(*range, Severity::Error, message));
            }
        }
    }
    for keyed in infer::keyed_strings(index, &summary, source) {
        let call = keyed.keyed.call.as_ref().map(|argument| {
            (
                FrameworkCall {
                    targets: &argument.targets,
                    strings: &argument.strings,
                    receiver: Some(&argument.receiver),
                },
                argument.position,
            )
        });
        let Some(manager) = plugins::keyed_manager(
            index,
            &keyed.keyed.key,
            call.as_ref().map(|(call, position)| (call, *position)),
        ) else {
            continue;
        };
        if let Some(message) =
            undefined_plugin(drupal, index, manager, &keyed.keyed.value, &guarded)
        {
            found.push(diagnostic(
                (keyed.start, keyed.end),
                Severity::Warning,
                message,
            ));
        }
    }
    found
}

/// The service id a call reads and its range: `\Drupal::service(id)` or a Drupal
/// container's `get(id)`, with a literal id.
fn service_id<'a>(index: &Index, call: &'a FoundCall) -> Option<(&'a str, (u32, u32))> {
    let id = call.strings.first()?.as_deref()?;
    let range = (*call.ranges.first()?)?;
    let reads = call.targets.iter().any(|target| match target {
        CallTarget::Method { class, name } => {
            (class.eq_ignore_ascii_case("Drupal") && name.eq_ignore_ascii_case("service"))
                || (name.eq_ignore_ascii_case("get") && is_a(index, class, &CONTAINERS))
        }
        CallTarget::Function(_) => false,
    });
    reads.then_some((id, range))
}

/// The message for a plugin id `manager` does not define, unless the file checks it
/// with `hasDefinition()` or the manager discovers plugins its own way.
fn undefined_plugin(
    drupal: &Drupal,
    index: &Index,
    manager: &str,
    id: &str,
    guarded: &dyn Fn(&str, &str) -> bool,
) -> Option<String> {
    if guarded("hasDefinition", id)
        || own_discovery(index, manager)
        || drupal.defines_plugin(index, manager, id)
    {
        return None;
    }
    let short = manager.rsplit('\\').next().unwrap_or(manager);
    Some(format!("Undefined plugin '{id}' of {short}"))
}

/// Whether the manager, or a parent below the default manager, declares its own
/// `getDiscovery()`.
fn own_discovery(index: &Index, manager: &str) -> bool {
    index
        .find_member(manager, MemberKind::Method, "getDiscovery")
        .first()
        .is_some_and(|(declaring, _)| {
            !declaring
                .declaration
                .fqn
                .eq_ignore_ascii_case(DEFAULT_MANAGER)
        })
}

fn diagnostic((start, end): (u32, u32), severity: Severity, message: String) -> Diagnostic {
    Diagnostic {
        start,
        end,
        severity,
        message,
    }
}
