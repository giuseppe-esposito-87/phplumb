//! Twig template names: the directories `config/packages/twig.yaml` gives each
//! namespace, in the order TwigBundle adds them to Twig's filesystem loader.

use std::path::{Path, PathBuf};

use crate::frameworks::Project;
use crate::frameworks::yaml::{self, Yaml};

/// The placeholder for the project directory in configured paths.
const PROJECT_DIR: &str = "%kernel.project_dir%";

#[derive(Default)]
pub(crate) struct Templates {
    /// Each directory with its namespace, `None` for the main namespace, in order;
    /// a directory the module cannot read is `None` and ends its namespace.
    paths: Vec<(Option<String>, Option<PathBuf>)>,
}

impl Templates {
    /// The `twig.paths` entries of `config/packages/twig.yaml`, then
    /// `default_path` (`templates` by default) when it exists. A `when@<env>` block
    /// adds only namespaced paths, after the others, since its main-namespace paths
    /// and `default_path` would shadow the other environments' templates.
    pub(crate) fn read(project: &Project) -> Templates {
        let dir = &project.dir;
        let mut templates = Templates::default();
        let mut default_path = format!("{PROJECT_DIR}/templates");
        for name in ["twig.yaml", "twig.yml"] {
            let Ok(text) = std::fs::read_to_string(dir.join("config/packages").join(name)) else {
                continue;
            };
            let Some(root) = yaml::parse(&text) else {
                continue;
            };
            if let Some(twig) = root.get("twig") {
                templates.read_paths(dir, twig.get("paths"), true);
                if let Some(path) = twig.get("default_path").and_then(Yaml::text) {
                    default_path = path.to_string();
                }
            }
            if let Yaml::Map(entries) = &root {
                let environments = entries
                    .iter()
                    .filter(|(key, _)| key.text().is_some_and(|k| k.starts_with("when@")))
                    .filter_map(|(_, value)| value.get("twig"));
                for twig in environments {
                    templates.read_paths(dir, twig.get("paths"), false);
                }
            }
        }
        match resolve(dir, &default_path) {
            Some(path) if path.is_dir() => templates.paths.push((None, Some(path))),
            Some(_) => {}
            None => templates.paths.push((None, None)),
        }
        templates
    }

    /// `paths` as a map from directory to namespace (empty or null for the main
    /// one), or as a list of main-namespace directories; `main` says whether
    /// main-namespace entries count.
    fn read_paths(&mut self, dir: &Path, paths: Option<&Yaml>, main: bool) {
        let entries: Vec<(Option<&str>, Option<String>)> = match paths {
            Some(Yaml::Map(entries)) => entries
                .iter()
                .map(|(path, namespace)| {
                    let namespace = namespace
                        .text()
                        .filter(|n| !matches!(n.to_ascii_lowercase().as_str(), "" | "~" | "null"))
                        .map(str::to_string);
                    (path.text(), namespace)
                })
                .collect(),
            Some(Yaml::Sequence(entries)) => {
                entries.iter().map(|path| (path.text(), None)).collect()
            }
            _ => Vec::new(),
        };
        for (path, namespace) in entries {
            if namespace.is_none() && !main {
                continue;
            }
            self.paths
                .push((namespace, path.and_then(|p| resolve(dir, p))));
        }
    }

    /// The first existing file `name` names: `@Ns/rest` in `Ns`'s directories,
    /// anything else in the main namespace's.
    pub(crate) fn file(&self, name: &str) -> Option<PathBuf> {
        let (namespace, rest) = match name.strip_prefix('@') {
            Some(named) => {
                let (namespace, rest) = named.split_once('/')?;
                (Some(namespace), rest)
            }
            None => (None, name),
        };
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() || rest.split('/').any(|part| part == "..") {
            return None;
        }
        for (_, dir) in self
            .paths
            .iter()
            .filter(|(ns, _)| ns.as_deref() == namespace)
        {
            // Twig would look in an unreadable directory first: its answer is unknown.
            let path = dir.as_ref()?.join(rest);
            if path.is_file() {
                return Some(path);
            }
        }
        None
    }
}

/// A configured path with the project directory filled in; relative paths are
/// relative to the project. A path with another parameter cannot be read.
fn resolve(dir: &Path, path: &str) -> Option<PathBuf> {
    if path.replace(PROJECT_DIR, "").contains('%') {
        return None;
    }
    let path = path.replace(PROJECT_DIR, &dir.to_string_lossy());
    let path = PathBuf::from(path);
    Some(if path.is_absolute() {
        path
    } else {
        dir.join(path)
    })
}
