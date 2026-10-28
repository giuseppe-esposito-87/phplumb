//! Symfony's YAML configuration: the parameters `config/services*.yaml` and
//! `config/packages/*.yaml` define, and parameter names written as `%name%`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::filter::PathFilter;
use crate::frameworks::Project;
use crate::frameworks::symfony::RouteTarget;
use crate::frameworks::yaml::{self, Yaml};
use crate::index::Location;

/// The configuration files of the project: `config/services.yaml`,
/// `config/services_<env>.yaml`, then `config/packages/*.yaml`, in path order.
pub(crate) fn config_files(project: &Project, filter: &PathFilter) -> Vec<PathBuf> {
    let config = project.dir.join("config");
    let mut files = yaml_files(&config, |name| {
        name == "services.yaml" || (name.starts_with("services_") && name.ends_with(".yaml"))
    });
    files.extend(yaml_files(&config.join("packages"), |name| {
        name.ends_with(".yaml")
    }));
    files.retain(|path| filter.in_scope(path));
    files
}

fn yaml_files(dir: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .filter(|path| path.file_name().and_then(|n| n.to_str()).is_some_and(&keep))
        .collect();
    files.sort();
    files
}

/// The parameters the configuration files define under `parameters:`, also
/// inside `when@<env>` blocks, each at its key; `env(NAME)` keys are environment
/// defaults, not parameters.
pub(crate) fn read_parameters(files: &[PathBuf]) -> HashMap<String, Vec<RouteTarget>> {
    let mut found: HashMap<String, Vec<RouteTarget>> = HashMap::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(file) else {
            continue;
        };
        let Some(root) = yaml::parse(&text) else {
            continue;
        };
        for block in top_blocks(&root) {
            let Some(Yaml::Map(entries)) = block.get("parameters") else {
                continue;
            };
            for (key, _) in entries {
                let Yaml::Scalar { value, start, end } = key else {
                    continue;
                };
                if value.starts_with("env(") {
                    continue;
                }
                let (start, end) = yaml::value_range(&text, value, *start, *end);
                found.entry(value.clone()).or_default().push(RouteTarget {
                    location: Location {
                        path: file.clone(),
                        start,
                        end,
                    },
                    text: value.clone(),
                });
            }
        }
    }
    found
}

/// The document itself, then each `when@<env>` block in it.
pub(crate) fn top_blocks(root: &Yaml) -> Vec<&Yaml> {
    let mut blocks = vec![root];
    if let Yaml::Map(entries) = root {
        blocks.extend(
            entries
                .iter()
                .filter(|(key, _)| key.text().is_some_and(|k| k.starts_with("when@")))
                .map(|(_, value)| value),
        );
    }
    blocks
}

/// The parameter name of the `%name%` that holds `offset` in `text`; none for the
/// `%%` escape, `%env(...)%` and the kernel's own `kernel.*` parameters.
pub(crate) fn parameter_at(text: &str, offset: usize) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            i += 1;
            continue;
        }
        if bytes.get(i + 1) == Some(&b'%') {
            i += 2;
            continue;
        }
        let close = i + 1 + text[i + 1..].find('%')?;
        if i <= offset && offset <= close {
            let name = &text[i + 1..close];
            let readable = !name.is_empty()
                && !name.contains(char::is_whitespace)
                && !name.starts_with("env(")
                && !name.starts_with("kernel.");
            return readable.then(|| name.to_string());
        }
        i = close + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parameter_at;

    fn at(text: &str, needle: &str) -> Option<String> {
        parameter_at(text, text.find(needle).unwrap() + 1)
    }

    #[test]
    fn reads_parameter_names() {
        assert_eq!(at("'%a.b%'", "a.b"), Some("a.b".into()));
        assert_eq!(at("x/%a%/y", "a%"), Some("a".into()));
        assert_eq!(at("x/%a%/y", "/y"), None);
        assert_eq!(at("'100%%'", "%%"), None);
        assert_eq!(at("'%env(A)%'", "env"), None);
        assert_eq!(at("'%kernel.secret%'", "kernel"), None);
        assert_eq!(at("'%%a%'", "a%"), None);
    }
}
