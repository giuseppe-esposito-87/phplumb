//! Broken Symfony references: route, template and parameter names written in PHP
//! that are defined nowhere. Any doubt stays silent.

use std::path::Path;

use super::{
    AUTOWIRE_ATTRIBUTE, PARAMETER_METHODS, ROUTE_METHODS, Symfony, TEMPLATE_ATTRIBUTE,
    TEMPLATE_METHODS, TemplateStatus, calls_one_of, config,
};
use crate::diagnostics::{Diagnostic, Severity};
use crate::frameworks::{FrameworkCall, mentions};
use crate::index::Index;
use crate::infer;
use crate::summary::summarize;

/// Exceptions a missing route, template or parameter throws: a file that mentions
/// one may catch it, so its names of that kind stay unchecked.
const ROUTE_NOT_FOUND: [&str; 1] =
    ["Symfony\\Component\\Routing\\Exception\\RouteNotFoundException"];
const TEMPLATE_NOT_FOUND: [&str; 1] = ["Twig\\Error\\LoaderError"];
const PARAMETER_NOT_FOUND: [&str; 1] =
    ["Symfony\\Component\\DependencyInjection\\Exception\\ParameterNotFoundException"];

pub(super) fn find(
    symfony: &Symfony,
    index: &Index,
    path: &Path,
    source: &[u8],
) -> Vec<Diagnostic> {
    // Names defined in excluded files are unknown.
    if index.has_exclusions() {
        return Vec::new();
    }
    let summary = summarize(source);
    let routes = !mentions(&summary, &ROUTE_NOT_FOUND);
    let templates = !mentions(&summary, &TEMPLATE_NOT_FOUND);
    let parameters = !mentions(&summary, &PARAMETER_NOT_FOUND);
    let names: Vec<&str> = ROUTE_METHODS
        .iter()
        .chain(&TEMPLATE_METHODS)
        .chain(&PARAMETER_METHODS)
        .flat_map(|(_, methods)| methods.iter().copied())
        .collect();
    let mut found = Vec::new();
    for call in infer::calls_named(index, path, &summary, source, &names) {
        let (Some(Some(name)), Some(Some(range))) = (call.strings.first(), call.ranges.first())
        else {
            continue;
        };
        let framework = FrameworkCall {
            targets: &call.targets,
            strings: &call.strings,
            receiver: Some(&call.receiver),
        };
        let calls = |methods: &[(&str, &[&str])]| calls_one_of(index, &framework, methods);
        let message = if routes && calls(&ROUTE_METHODS) && symfony.route_missing(name) {
            format!("Undefined route '{name}'")
        } else if templates
            && calls(&TEMPLATE_METHODS)
            && symfony.template_status(name) == TemplateStatus::Missing
        {
            format!("Undefined template '{name}'")
        } else if parameters && calls(&PARAMETER_METHODS) && symfony.parameter_missing(index, name)
        {
            format!("Undefined parameter '{name}'")
        } else {
            continue;
        };
        found.push(diagnostic(*range, message));
    }
    for (attribute, end) in infer::attribute_strings(&summary, source) {
        let first = attribute.key.is_none() && attribute.position == 0;
        let key = attribute.key.as_deref();
        if parameters && attribute.attribute.eq_ignore_ascii_case(AUTOWIRE_ATTRIBUTE) {
            if key == Some("param") && symfony.parameter_missing(index, &attribute.value) {
                found.push(diagnostic(
                    (attribute.start, end),
                    format!("Undefined parameter '{}'", attribute.value),
                ));
            }
            if key == Some("value") || first {
                let raw = String::from_utf8_lossy(&source[attribute.start as usize..end as usize]);
                for (name, start, stop) in config::parameters_in(&raw) {
                    if symfony.parameter_missing(index, name) {
                        found.push(diagnostic(
                            (
                                attribute.start + start as u32,
                                attribute.start + stop as u32,
                            ),
                            format!("Undefined parameter '{name}'"),
                        ));
                    }
                }
            }
        }
        let template = key == Some("template") || first;
        if templates
            && template
            && attribute.attribute.eq_ignore_ascii_case(TEMPLATE_ATTRIBUTE)
            && symfony.template_status(&attribute.value) == TemplateStatus::Missing
        {
            found.push(diagnostic(
                (attribute.start, end),
                format!("Undefined template '{}'", attribute.value),
            ));
        }
    }
    found
}

fn diagnostic((start, end): (u32, u32), message: String) -> Diagnostic {
    Diagnostic {
        start,
        end,
        severity: Severity::Error,
        message,
    }
}
