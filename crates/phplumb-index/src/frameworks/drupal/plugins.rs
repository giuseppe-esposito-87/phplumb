//! Drupal plugins: plugin types read from the constructors of plugin managers, and
//! the plugin classes of each type, found by attribute or annotation in the type's
//! subdirectory of every module namespace and of every `Drupal\Core\*` and
//! `Drupal\Component\*` namespace.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::extensions::Extensions;
use super::is_a;
use crate::frameworks::CallTarget;
use crate::index::Index;
use crate::infer;
use crate::metadata::{MetaValue, Metadata};
use crate::summary::{SymbolKind, summarize};

pub struct PluginType {
    /// The plugin manager class.
    pub manager: String,
    pub subdir: String,
    pub attribute: String,
    pub annotation: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    pub id: String,
    pub manager: String,
    pub class: String,
    pub path: PathBuf,
    /// The class name.
    pub start: u32,
    pub end: u32,
}

const DEFAULT_MANAGER: &str = "Drupal\\Core\\Plugin\\DefaultPluginManager";
const DEFAULT_ATTRIBUTE: &str = "Drupal\\Component\\Plugin\\Attribute\\Plugin";
const DEFAULT_ANNOTATION: &str = "Drupal\\Component\\Annotation\\Plugin";

/// Plugin types, and the plugins of each by lowercased manager.
#[derive(Default)]
pub(crate) struct Plugins {
    types: Vec<PluginType>,
    /// The files declaring plugin managers.
    manager_files: HashSet<PathBuf>,
    by_manager: HashMap<String, Vec<Plugin>>,
}

impl Plugins {
    pub(crate) fn read(index: &Index, extensions: &Extensions) -> Plugins {
        let mut plugins = Plugins::default();
        plugins.read_types(index);
        plugins.collect(index, extensions);
        plugins
    }

    /// Reads the plugin types again when a manager's file or a file declaring a
    /// manager changed, then collects the plugins again from the index.
    pub(crate) fn update(
        &mut self,
        index: &Index,
        extensions: &Extensions,
        changed: &[PathBuf],
        removed: &[PathBuf],
    ) {
        let touches_managers = removed
            .iter()
            .chain(changed)
            .any(|path| self.manager_files.iter().any(|file| file.starts_with(path)))
            || index.declarations().any(|(path, declaration)| {
                declaration.kind == SymbolKind::Class
                    && changed.iter().any(|changed| path.starts_with(changed))
                    && is_a(index, &declaration.fqn, &[DEFAULT_MANAGER])
            });
        if touches_managers {
            self.read_types(index);
        }
        self.collect(index, extensions);
    }

    pub(crate) fn types(&self) -> &[PluginType] {
        &self.types
    }

    pub(crate) fn of(&self, manager: &str) -> &[Plugin] {
        self.by_manager
            .get(&manager.to_ascii_lowercase())
            .map_or(&[], Vec::as_slice)
    }

    /// The plugins of `manager` named `id`; for an id `base:derivative` with none,
    /// those named `base`.
    pub(crate) fn named(&self, manager: &str, id: &str) -> Vec<&Plugin> {
        let exact: Vec<&Plugin> = self.of(manager).iter().filter(|p| p.id == id).collect();
        if !exact.is_empty() {
            return exact;
        }
        let Some((base, _)) = id.split_once(':') else {
            return Vec::new();
        };
        self.of(manager).iter().filter(|p| p.id == base).collect()
    }

    /// Every subclass of the default plugin manager whose own constructor calls the
    /// default constructor with a literal subdirectory.
    fn read_types(&mut self, index: &Index) {
        let mut types = Vec::new();
        let mut files = HashSet::new();
        for manager in index.all_subtypes(DEFAULT_MANAGER) {
            if manager.declaration.kind != SymbolKind::Class {
                continue;
            }
            let fqn = &manager.declaration.fqn;
            let Ok(source) = std::fs::read(manager.path) else {
                continue;
            };
            let summary = summarize(&source);
            let caller = format!("{fqn}::__construct");
            let calls =
                infer::calls_named(index, manager.path, &summary, &source, &["__construct"]);
            let Some(call) = calls.iter().find(|call| {
                call.caller
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&caller))
                    && call.targets.iter().any(|target| {
                        matches!(target, CallTarget::Method { class, .. } if class.eq_ignore_ascii_case(DEFAULT_MANAGER))
                    })
            }) else {
                continue;
            };
            let argument = |position: usize| call.strings.get(position).cloned().flatten();
            let Some(subdir) = argument(0) else {
                continue;
            };
            files.insert(manager.path.to_path_buf());
            types.push(PluginType {
                manager: fqn.clone(),
                subdir,
                attribute: argument(4).unwrap_or_else(|| DEFAULT_ATTRIBUTE.to_string()),
                annotation: argument(5)
                    .map(|a| a.trim_start_matches('\\').to_string())
                    .unwrap_or_else(|| DEFAULT_ANNOTATION.to_string()),
            });
        }
        types.sort_by(|a, b| a.manager.cmp(&b.manager));
        types.dedup_by(|a, b| a.manager.eq_ignore_ascii_case(&b.manager));
        self.types = types;
        self.manager_files = files;
    }

    /// The plugins of every type: classes in the type's subdirectory of a module,
    /// core or component namespace, outside test modules, with the type's attribute
    /// or, failing that, its annotation.
    fn collect(&mut self, index: &Index, extensions: &Extensions) {
        let mut by_subdir: HashMap<String, Vec<&PluginType>> = HashMap::new();
        for plugin_type in &self.types {
            let namespace = plugin_type.subdir.trim_matches('/').replace('/', "\\");
            by_subdir
                .entry(namespace.to_ascii_lowercase())
                .or_default()
                .push(plugin_type);
        }
        let roots: HashSet<String> = extensions
            .modules
            .iter()
            .map(|m| m.to_ascii_lowercase())
            .chain(["core".to_string(), "component".to_string()])
            .collect();
        let root = index.filter().root().to_path_buf();
        let mut is_a_cache: HashMap<(String, String), bool> = HashMap::new();
        let mut found: HashMap<String, Vec<Plugin>> = HashMap::new();
        for (path, declaration) in index.declarations() {
            if declaration.kind != SymbolKind::Class || in_tests(&root, path) {
                continue;
            }
            let Some(shape) = &declaration.class else {
                continue;
            };
            let lower = declaration.fqn.to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix("drupal\\") else {
                continue;
            };
            let Some((extension, rest)) = rest.split_once('\\') else {
                continue;
            };
            if !roots.contains(extension) {
                continue;
            }
            // Core and component plugins live in a namespace one level below,
            // `Drupal\Core\{component}`, as the kernel registers them.
            let rest = if extension == "core" || extension == "component" {
                match rest.split_once('\\') {
                    Some((_, below)) => below,
                    None => continue,
                }
            } else {
                rest
            };
            let mut prefixes = Vec::new();
            let mut end = 0;
            while let Some(next) = rest[end..].find('\\') {
                end += next;
                prefixes.push(&rest[..end]);
                end += 1;
            }
            for prefix in prefixes {
                for plugin_type in by_subdir.get(prefix).into_iter().flatten() {
                    let mut matches = |metadata: &Metadata, target: &str| {
                        if metadata.name.eq_ignore_ascii_case(target) {
                            return true;
                        }
                        *is_a_cache
                            .entry((
                                metadata.name.to_ascii_lowercase(),
                                target.to_ascii_lowercase(),
                            ))
                            .or_insert_with(|| is_a(index, &metadata.name, &[target]))
                    };
                    let attribute = shape
                        .metadata
                        .iter()
                        .find(|m| !m.annotation && matches(m, &plugin_type.attribute));
                    let declared = attribute.or_else(|| {
                        shape
                            .metadata
                            .iter()
                            .find(|m| m.annotation && matches(m, &plugin_type.annotation))
                    });
                    let Some(id) = declared.and_then(plugin_id) else {
                        continue;
                    };
                    found
                        .entry(plugin_type.manager.to_ascii_lowercase())
                        .or_default()
                        .push(Plugin {
                            id: id.to_string(),
                            manager: plugin_type.manager.clone(),
                            class: declaration.fqn.clone(),
                            path: path.to_path_buf(),
                            start: declaration.name_start,
                            end: declaration.name_end,
                        });
                }
            }
        }
        for plugins in found.values_mut() {
            plugins.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
        }
        self.by_manager = found;
    }
}

/// The `id` argument of a plugin attribute or annotation: named, or the first
/// positional one.
fn plugin_id(metadata: &Metadata) -> Option<&str> {
    metadata
        .argument("id")
        .or_else(|| {
            metadata
                .arguments
                .first()
                .filter(|(key, _)| key.is_none())
                .map(|(_, value)| value)
        })
        .and_then(|value| match value {
            MetaValue::String(id) => Some(id.as_str()),
            _ => None,
        })
}

fn in_tests(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root)
        .is_ok_and(|relative| relative.components().any(|c| c.as_os_str() == "tests"))
}
