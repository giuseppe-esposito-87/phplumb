//! Drupal hooks: extensions from `*.info.yml`, hook implementations found the way
//! Drupal's hook collector passes find them, and the `hook_*` documentation of
//! `*.api.php` files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use yaml_rust2::YamlLoader;

use super::is_a;
use super::services::{drupal_files, is_drupal_file};
use crate::filter::PathFilter;
use crate::index::Index;
use crate::metadata::{MetaValue, Metadata};
use crate::summary::{Declaration, MemberKind, SymbolKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionKind {
    Module,
    Profile,
    Theme,
}

impl ExtensionKind {
    /// Whether this is a theme: modules and profiles form one group, themes another.
    pub(crate) fn is_theme(self) -> bool {
        self == ExtensionKind::Theme
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookImplementation {
    pub hook: String,
    /// The module or theme the implementation is for.
    pub extension: String,
    pub kind: ExtensionKind,
    pub path: PathBuf,
    /// The function or method name, and its range.
    pub name: String,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookDoc {
    /// The documented hook name, without `hook_`, placeholders included.
    pub name: String,
    pub path: PathBuf,
    pub start: u32,
    pub end: u32,
}

const INFO: &str = ".info.yml";
const HOOK: &str = "Drupal\\Core\\Hook\\Attribute\\Hook";
const SCAN_STOP: &str = "Drupal\\Core\\Hook\\Attribute\\ProceduralHookScanStop";
const LEGACY: [&str; 3] = [
    "Drupal\\Core\\Hook\\Attribute\\LegacyHook",
    "Drupal\\Core\\Hook\\Attribute\\LegacyModuleImplementsAlter",
    "Drupal\\Core\\Hook\\Attribute\\LegacyRequirementsHook",
];
/// Hooks that the hook system does not collect from procedural code.
const NOT_COLLECTED: [&str; 7] = [
    "install",
    "install_tasks",
    "install_tasks_alter",
    "schema",
    "uninstall",
    "update_last_removed",
    "update_dependencies",
];

/// Every module, profile and theme on disk, outside test modules.
#[derive(Default)]
pub(crate) struct Extensions {
    /// Extension directory to its name and kind.
    by_dir: HashMap<PathBuf, (String, ExtensionKind)>,
    kinds: HashMap<String, ExtensionKind>,
    /// Module and profile names, then theme names, longest first.
    modules: Vec<String>,
    themes: Vec<String>,
}

impl Extensions {
    fn read(filter: &PathFilter) -> Extensions {
        let mut extensions = Extensions::default();
        for path in drupal_files(filter, filter.root(), INFO) {
            let Some(kind) = info_kind(&path) else {
                continue;
            };
            let (Some(dir), Some(name)) = (
                path.parent(),
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(INFO)),
            ) else {
                continue;
            };
            extensions
                .by_dir
                .insert(dir.to_path_buf(), (name.to_string(), kind));
            extensions.kinds.insert(name.to_string(), kind);
        }
        for (name, kind) in &extensions.kinds {
            if kind.is_theme() {
                extensions.themes.push(name.clone());
            } else {
                extensions.modules.push(name.clone());
            }
        }
        for names in [&mut extensions.modules, &mut extensions.themes] {
            names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        }
        extensions
    }

    /// The extension a file belongs to: the one of its nearest ancestor directory
    /// holding an `.info.yml`, with that directory.
    fn owner<'a>(&'a self, path: &Path) -> Option<(&'a Path, &'a str, ExtensionKind)> {
        path.ancestors().skip(1).find_map(|dir| {
            self.by_dir
                .get_key_value(dir)
                .map(|(dir, (name, kind))| (dir.as_path(), name.as_str(), *kind))
        })
    }
}

/// The `type` of an `.info.yml` file, when it declares a module, profile or theme.
fn info_kind(path: &Path) -> Option<ExtensionKind> {
    let text = std::fs::read_to_string(path).ok()?;
    let docs = YamlLoader::load_from_str(&text).ok()?;
    match docs.first()?["type"].as_str()? {
        "module" => Some(ExtensionKind::Module),
        "profile" => Some(ExtensionKind::Profile),
        "theme" => Some(ExtensionKind::Theme),
        _ => None,
    }
}

/// Hook implementations by hook, and hook documentation by documented name.
#[derive(Default)]
pub(crate) struct Hooks {
    extensions: Extensions,
    implementations: HashMap<String, Vec<HookImplementation>>,
    docs: HashMap<String, Vec<HookDoc>>,
}

impl Hooks {
    pub(crate) fn read(index: &Index) -> Hooks {
        let mut hooks = Hooks {
            extensions: Extensions::read(index.filter()),
            ..Hooks::default()
        };
        hooks.collect(index);
        hooks
    }

    /// Re-reads the extensions when an `.info.yml` file or a directory changed, then
    /// collects the hooks again from the index.
    pub(crate) fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]) {
        let filter = index.filter();
        let extensions_changed = removed.iter().chain(changed).any(|path| {
            path.is_dir()
                || is_drupal_file(filter, path, INFO)
                || self
                    .extensions
                    .by_dir
                    .keys()
                    .any(|dir| dir.starts_with(path))
        });
        if extensions_changed {
            self.extensions = Extensions::read(filter);
        }
        self.collect(index);
    }

    fn collect(&mut self, index: &Index) {
        let mut implementations: HashMap<String, Vec<HookImplementation>> = HashMap::new();
        let mut docs: HashMap<String, Vec<HookDoc>> = HashMap::new();
        let root = index.filter().root().to_path_buf();
        let mut file: Vec<&Declaration> = Vec::new();
        let mut current: Option<&Path> = None;
        let mut flush = |path: Option<&Path>, file: &mut Vec<&Declaration>| {
            if let Some(path) = path {
                for found in self.file_hooks(index, path, file) {
                    implementations
                        .entry(found.hook.clone())
                        .or_default()
                        .push(found);
                }
            }
            file.clear();
        };
        for (path, declaration) in index.declarations() {
            if current != Some(path) {
                flush(current, &mut file);
                current = Some(path);
            }
            file.push(declaration);
            if let Some(doc) = hook_doc(&root, path, declaration) {
                docs.entry(doc.name.clone()).or_default().push(doc);
            }
        }
        flush(current, &mut file);
        for found in implementations.values_mut() {
            found.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
        }
        for found in docs.values_mut() {
            found.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
        }
        self.implementations = implementations;
        self.docs = docs;
    }

    pub(crate) fn implementations(&self, hook: &str) -> &[HookImplementation] {
        self.implementations.get(hook).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn docs(&self, name: &str) -> &[HookDoc] {
        self.docs.get(name).map_or(&[], Vec::as_slice)
    }

    /// The hooks the declarations of one file implement, procedural and attributed.
    pub(crate) fn file_hooks(
        &self,
        index: &Index,
        path: &Path,
        declarations: &[&Declaration],
    ) -> Vec<HookImplementation> {
        let Some((dir, owner, kind)) = self.extensions.owner(path) else {
            return Vec::new();
        };
        let Ok(relative) = path.strip_prefix(dir) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        if relative.starts_with("src/Hook") {
            for declaration in declarations {
                found.extend(self.attributed(index, path, owner, kind, declaration));
            }
        } else if procedural_file(relative, kind) {
            let mut functions: Vec<&&Declaration> = declarations
                .iter()
                .filter(|d| d.kind == SymbolKind::Function && !d.fqn.contains('\\'))
                .collect();
            functions.sort_by_key(|d| d.name_start);
            for function in functions {
                if has(&function.metadata, &[SCAN_STOP]) {
                    break;
                }
                if has(&function.metadata, &LEGACY) {
                    continue;
                }
                found.extend(self.procedural(path, owner, kind, function));
            }
        }
        found
    }

    /// The hook a global function implements, named `{extension}_{hook}`: the file's
    /// own extension first, then every extension of its group, longest first.
    fn procedural(
        &self,
        path: &Path,
        owner: &str,
        kind: ExtensionKind,
        function: &Declaration,
    ) -> Option<HookImplementation> {
        let name = function.fqn.as_str();
        let group = if kind.is_theme() {
            &self.extensions.themes
        } else {
            &self.extensions.modules
        };
        let (extension, hook) = std::iter::once(owner)
            .chain(group.iter().map(String::as_str))
            .find_map(|extension| {
                let hook = name.strip_prefix(extension)?.strip_prefix('_')?;
                (!hook.is_empty() && !starts_with_update_number(hook)).then_some((extension, hook))
            })?;
        if NOT_COLLECTED.contains(&hook) || ends_with_update_number(name) {
            return None;
        }
        Some(HookImplementation {
            hook: hook.to_string(),
            extension: extension.to_string(),
            kind: self
                .extensions
                .kinds
                .get(extension)
                .copied()
                .unwrap_or(kind),
            path: path.to_path_buf(),
            name: name.to_string(),
            start: function.name_start,
            end: function.name_end,
        })
    }

    /// The hooks a class in `src/Hook` implements with `Hook` attributes on the class
    /// or on its methods.
    fn attributed(
        &self,
        index: &Index,
        path: &Path,
        owner: &str,
        kind: ExtensionKind,
        class: &Declaration,
    ) -> Vec<HookImplementation> {
        let Some(shape) = &class.class else {
            return Vec::new();
        };
        let method = |name: &str| {
            shape
                .members
                .iter()
                .find(|m| m.kind == MemberKind::Method && m.name.eq_ignore_ascii_case(name))
        };
        let mut found = Vec::new();
        let mut add = |attribute: &Metadata, default_method: Option<&str>| {
            let Some(hook) = hook_name(attribute) else {
                return;
            };
            let named = text_argument(attribute, "method").filter(|m| !m.is_empty());
            let Some(member) = named.or(default_method).and_then(method) else {
                return;
            };
            let extension = text_argument(attribute, "module").unwrap_or(owner);
            found.push(HookImplementation {
                hook: hook.to_string(),
                extension: extension.to_string(),
                kind: self
                    .extensions
                    .kinds
                    .get(extension)
                    .copied()
                    .unwrap_or(kind),
                path: path.to_path_buf(),
                name: member.name.clone(),
                start: member.name_start,
                end: member.name_end,
            });
        };
        for attribute in shape.metadata.iter().filter(|m| is_hook(index, m)) {
            add(attribute, Some("__invoke"));
        }
        for member in shape
            .members
            .iter()
            .filter(|m| m.kind == MemberKind::Method)
        {
            for attribute in member.metadata.iter().filter(|m| is_hook(index, m)) {
                add(attribute, Some(&member.name));
            }
        }
        found
    }
}

/// Whether a file of an extension, at `relative` to its directory, can hold
/// procedural hooks.
fn procedural_file(relative: &Path, kind: ExtensionKind) -> bool {
    let skipped: &[&str] = if kind.is_theme() {
        &["tests", "js", "css", "templates"]
    } else {
        &["tests", "js", "css"]
    };
    let in_skipped = relative.parent().is_some_and(|dir| {
        dir.components()
            .any(|c| skipped.iter().any(|s| c.as_os_str() == *s))
    });
    let Some(name) = relative.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    let extensions: &[&str] = if kind.is_theme() {
        &[".inc", ".theme"]
    } else {
        &[".module", ".inc", ".install", ".profile"]
    };
    let eligible = extensions.iter().any(|e| name.ends_with(e))
        || (kind.is_theme() && name == "theme-settings.php");
    eligible && !in_skipped
}

/// `update_` followed by a digit.
fn starts_with_update_number(hook: &str) -> bool {
    hook.strip_prefix("update_")
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// `update_` followed by digits only, at the end.
fn ends_with_update_number(name: &str) -> bool {
    let digits = name.trim_end_matches(|c: char| c.is_ascii_digit());
    digits.len() < name.len() && digits.ends_with("update_")
}

fn has(metadata: &[Metadata], names: &[&str]) -> bool {
    metadata
        .iter()
        .any(|m| names.iter().any(|n| m.name.eq_ignore_ascii_case(n)))
}

/// A `Hook` attribute, or one of a class extending it.
pub(crate) fn is_hook(index: &Index, attribute: &Metadata) -> bool {
    attribute.name.eq_ignore_ascii_case(HOOK) || is_a(index, &attribute.name, &[HOOK])
}

/// The `hook` argument of a `Hook` attribute: named, or the first positional one.
pub(crate) fn hook_name(attribute: &Metadata) -> Option<&str> {
    attribute
        .argument("hook")
        .or_else(|| {
            attribute
                .arguments
                .first()
                .filter(|(key, _)| key.is_none())
                .map(|(_, value)| value)
        })
        .and_then(|value| match value {
            MetaValue::String(text) => Some(text.as_str()),
            _ => None,
        })
}

fn text_argument<'a>(attribute: &'a Metadata, name: &str) -> Option<&'a str> {
    match attribute.argument(name)? {
        MetaValue::String(text) => Some(text),
        _ => None,
    }
}

/// A `hook_*` function of an `*.api.php` file outside test modules.
fn hook_doc(root: &Path, path: &Path, declaration: &Declaration) -> Option<HookDoc> {
    if declaration.kind != SymbolKind::Function {
        return None;
    }
    let name = declaration.fqn.strip_prefix("hook_")?;
    let is_api = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".api.php"));
    let in_tests = path
        .strip_prefix(root)
        .is_ok_and(|relative| relative.components().any(|c| c.as_os_str() == "tests"));
    (is_api && !in_tests && !name.is_empty()).then(|| HookDoc {
        name: name.to_string(),
        path: path.to_path_buf(),
        start: declaration.name_start,
        end: declaration.name_end,
    })
}
