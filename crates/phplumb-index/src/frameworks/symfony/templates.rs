//! Twig template names: the directories `config/packages/twig.yaml` gives each
//! namespace, in the order TwigBundle adds them to Twig's filesystem loader.

use std::path::{Path, PathBuf};

use crate::frameworks::Project;
use crate::frameworks::yaml::{self, Yaml};

/// The placeholder for the project directory in configured paths.
const PROJECT_DIR: &str = "%kernel.project_dir%";

#[derive(Default)]
pub(crate) struct Templates {
    /// Each directory with its namespace, `None` for the main namespace, in order.
    paths: Vec<(Option<String>, PathBuf)>,
}

impl Templates {
    /// The `twig.paths` entries of `config/packages/twig.yaml`, also inside
    /// `when@<env>` blocks, then `default_path` (`templates` by default) when it
    /// exists.
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
            let mut blocks = vec![root.get("twig")];
            if let Yaml::Map(entries) = &root {
                blocks.extend(
                    entries
                        .iter()
                        .filter(|(key, _)| key.text().is_some_and(|k| k.starts_with("when@")))
                        .map(|(_, value)| value.get("twig")),
                );
            }
            for twig in blocks.into_iter().flatten() {
                templates.read_paths(dir, twig.get("paths"));
                if let Some(path) = twig.get("default_path").and_then(Yaml::text) {
                    default_path = path.to_string();
                }
            }
        }
        if let Some(path) = resolve(dir, &default_path)
            && path.is_dir()
        {
            templates.paths.push((None, path));
        }
        templates
    }

    /// `paths` as a map from directory to namespace (empty or null for the main
    /// one), or as a list of main-namespace directories.
    fn read_paths(&mut self, dir: &Path, paths: Option<&Yaml>) {
        match paths {
            Some(Yaml::Map(entries)) => {
                for (path, namespace) in entries {
                    let Some(path) = path.text().and_then(|p| resolve(dir, p)) else {
                        continue;
                    };
                    let namespace = namespace
                        .text()
                        .filter(|n| !matches!(*n, "" | "~" | "null"))
                        .map(str::to_string);
                    self.paths.push((namespace, path));
                }
            }
            Some(Yaml::Sequence(entries)) => {
                for path in entries {
                    if let Some(path) = path.text().and_then(|p| resolve(dir, p)) {
                        self.paths.push((None, path));
                    }
                }
            }
            _ => {}
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
        self.paths
            .iter()
            .filter(|(ns, _)| ns.as_deref() == namespace)
            .map(|(_, dir)| dir.join(rest))
            .find(|path| path.is_file())
    }
}

/// A configured path with the project directory filled in; relative paths are
/// relative to the project. A path with another parameter cannot be read.
fn resolve(dir: &Path, path: &str) -> Option<PathBuf> {
    let path = path.replace(PROJECT_DIR, &dir.to_string_lossy());
    if path.contains('%') {
        return None;
    }
    let path = PathBuf::from(path);
    Some(if path.is_absolute() {
        path
    } else {
        dir.join(path)
    })
}
