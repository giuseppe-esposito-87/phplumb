//! Twig template names: the directories `config/packages/twig.yaml` gives each
//! namespace, in the order TwigBundle adds them to Twig's filesystem loader.

use std::path::{Path, PathBuf};

use crate::frameworks::Project;
use crate::frameworks::yaml::{self, Yaml};

/// The placeholder for the project directory in configured paths.
const PROJECT_DIR: &str = "%kernel.project_dir%";

/// TwigBridge's form themes, which FrameworkBundle adds to the main namespace last.
const FORM_THEMES: &str = "vendor/symfony/twig-bridge/Resources/views/Form";

/// Whether a template name resolves to a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TemplateStatus {
    Found(PathBuf),
    /// Every directory of its namespace was read, and none holds it.
    Missing,
    /// Its namespace is not configured, or a directory of it cannot be read.
    Unknown,
}

#[derive(Default)]
pub(crate) struct Templates {
    /// Each directory with its namespace, `None` for the main namespace, in order;
    /// a directory the module cannot read is `None` and ends its namespace.
    paths: Vec<(Option<String>, Option<PathBuf>)>,
}

impl Templates {
    /// The `twig.paths` entries of `config/packages/twig.yaml`, then
    /// `default_path` (`templates` by default) when it exists, then TwigBridge's form
    /// themes. A `when@<env>` block
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
        // Without the installed bridge, the main namespace is not known in full.
        let form_themes = dir.join(FORM_THEMES);
        let form_themes = form_themes.is_dir().then_some(form_themes);
        templates.paths.push((None, form_themes));
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
        match self.status(name) {
            TemplateStatus::Found(path) => Some(path),
            TemplateStatus::Missing | TemplateStatus::Unknown => None,
        }
    }

    /// Whether `name` resolves to a file, as Twig's loader looks for it.
    pub(crate) fn status(&self, name: &str) -> TemplateStatus {
        let (namespace, rest) = match name.strip_prefix('@') {
            Some(named) => match named.split_once('/') {
                Some((namespace, rest)) => (Some(namespace), rest),
                None => return TemplateStatus::Unknown,
            },
            None => (None, name),
        };
        let rest = rest.trim_start_matches('/');
        if rest.is_empty() || rest.split('/').any(|part| part == "..") {
            return TemplateStatus::Unknown;
        }
        let mut configured = false;
        for (_, dir) in self
            .paths
            .iter()
            .filter(|(ns, _)| ns.as_deref() == namespace)
        {
            configured = true;
            // Twig would look in an unreadable directory first: its answer is unknown.
            let Some(dir) = dir else {
                return TemplateStatus::Unknown;
            };
            let path = dir.join(rest);
            if path.is_file() {
                return TemplateStatus::Found(path);
            }
        }
        if configured {
            TemplateStatus::Missing
        } else {
            TemplateStatus::Unknown
        }
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
