//! Drupal hooks: extensions from `*.info.yml`, hook implementations found the way
//! Drupal's hook collector passes find them, and the `hook_*` documentation of
//! `*.api.php` files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use yaml_rust2::YamlLoader;

use super::is_a;
use super::services::{drupal_files, is_drupal_file};
use crate::filter::PathFilter;
use crate::frameworks::{CallTarget, FoundCall, FrameworkDeclaration};
use crate::index::{Index, Location};
use crate::metadata::{MetaValue, Metadata};
use crate::references;
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

/// A method invoking hooks: the position of its hook name, whether that name is an
/// alter type, and the position of the extension it is limited to.
struct Invoker {
    method: &'static str,
    hook: usize,
    alter: bool,
    extension: Option<usize>,
}

const MODULE_HANDLER: &str = "Drupal\\Core\\Extension\\ModuleHandlerInterface";
const THEME_MANAGER: &str = "Drupal\\Core\\Theme\\ThemeManagerInterface";

const fn invoker(
    method: &'static str,
    hook: usize,
    alter: bool,
    extension: Option<usize>,
) -> Invoker {
    Invoker {
        method,
        hook,
        alter,
        extension,
    }
}

/// The module handler's invoking methods, which reach modules and profiles.
const MODULE_INVOKERS: [Invoker; 7] = [
    invoker("invoke", 1, false, Some(0)),
    invoker("invokeAll", 0, false, None),
    invoker("invokeAllWith", 0, false, None),
    invoker("invokeDeprecated", 2, false, Some(1)),
    invoker("invokeAllDeprecated", 1, false, None),
    invoker("alter", 0, true, None),
    invoker("alterDeprecated", 1, true, None),
];

/// The theme manager's invoking methods, which reach themes.
const THEME_INVOKERS: [Invoker; 4] = [
    invoker("invoke", 1, false, Some(0)),
    invoker("invokeAllWith", 0, false, None),
    invoker("alter", 0, true, None),
    invoker("alterForTheme", 1, true, None),
];

/// The invoking method a call reaches, and whether it reaches themes.
fn invoker_of(index: &Index, targets: &[CallTarget]) -> Option<(&'static Invoker, bool)> {
    targets.iter().find_map(|target| {
        let CallTarget::Method { class, name } = target else {
            return None;
        };
        let find = |invokers: &'static [Invoker]| {
            invokers
                .iter()
                .find(|i| i.method.eq_ignore_ascii_case(name))
        };
        if let Some(found) =
            find(&MODULE_INVOKERS).filter(|_| is_a(index, class, &[MODULE_HANDLER]))
        {
            return Some((found, false));
        }
        find(&THEME_INVOKERS)
            .filter(|_| is_a(index, class, &[THEME_MANAGER]))
            .map(|found| (found, true))
    })
}

/// The hook a name at the invoker's hook position stands for.
fn invoked_hook(invoker: &Invoker, name: &str) -> String {
    if invoker.alter {
        format!("{name}_alter")
    } else {
        name.to_string()
    }
}

/// The names of every hook-invoking method.
fn invoking_methods() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = MODULE_INVOKERS
        .iter()
        .chain(THEME_INVOKERS.iter())
        .map(|i| i.method)
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Hooks Drupal core invokes under computed names, as patterns, with the methods
/// whose invoking calls compute them. In a pattern, an uppercase run is a
/// placeholder.
const COMPUTED: &[(&[&str], &[&str])] = &[
    (
        &[
            "ENTITY_TYPE_create",
            "ENTITY_TYPE_presave",
            "ENTITY_TYPE_insert",
            "ENTITY_TYPE_update",
            "ENTITY_TYPE_predelete",
            "ENTITY_TYPE_delete",
            "ENTITY_TYPE_revision_delete",
            "ENTITY_TYPE_translation_create",
            "ENTITY_TYPE_translation_insert",
            "ENTITY_TYPE_translation_delete",
            "ENTITY_TYPE_field_values_init",
            "entity_create",
            "entity_presave",
            "entity_insert",
            "entity_update",
            "entity_predelete",
            "entity_delete",
            "entity_revision_delete",
            "entity_translation_create",
            "entity_translation_insert",
            "entity_translation_delete",
            "entity_field_values_init",
        ],
        &[
            "Drupal\\Core\\Entity\\EntityStorageBase::invokeHook",
            "Drupal\\Core\\Config\\Entity\\ConfigEntityStorage::invokeHook",
        ],
    ),
    (
        &["ENTITY_TYPE_load"],
        &["Drupal\\Core\\Entity\\EntityStorageBase::postLoad"],
    ),
    (
        &["ENTITY_TYPE_storage_load"],
        &["Drupal\\Core\\Entity\\ContentEntityStorageBase::invokeStorageLoadHook"],
    ),
    (
        &["ENTITY_TYPE_revision_create"],
        &["Drupal\\Core\\Entity\\ContentEntityStorageBase::createRevision"],
    ),
    (
        &["ENTITY_TYPE_duplicate"],
        &[
            "Drupal\\Core\\Entity\\EntityBase::createDuplicate",
            "Drupal\\Core\\Entity\\ContentEntityBase::createDuplicate",
        ],
    ),
    (
        &["ENTITY_TYPE_access"],
        &["Drupal\\Core\\Entity\\EntityAccessControlHandler::access"],
    ),
    (
        &["ENTITY_TYPE_create_access"],
        &["Drupal\\Core\\Entity\\EntityAccessControlHandler::createAccess"],
    ),
    (
        &["ENTITY_TYPE_view", "ENTITY_TYPE_view_alter"],
        &["Drupal\\Core\\Entity\\EntityViewBuilder::buildMultiple"],
    ),
    (
        &["ENTITY_TYPE_build_defaults_alter"],
        &["Drupal\\Core\\Entity\\EntityViewBuilder::viewMultiple"],
    ),
    (
        &["ENTITY_TYPE_view_mode_alter"],
        &["Drupal\\Core\\Entity\\EntityViewBuilder::getBuildDefaults"],
    ),
    (
        &["ENTITY_TYPE_form_mode_alter"],
        &["Drupal\\Core\\Entity\\Entity\\EntityFormDisplay::collectRenderDisplay"],
    ),
    (
        &["ENTITY_TYPE_prepare_form"],
        &["Drupal\\Core\\Entity\\EntityForm::prepareInvokeAll"],
    ),
    (
        &[
            "entity_query_ENTITY_TYPE_alter",
            "entity_query_tag__TAG_alter",
            "entity_query_tag__ENTITY_TYPE__TAG_alter",
        ],
        &["Drupal\\Core\\Entity\\Query\\QueryBase::alter"],
    ),
    (
        &[
            "form_alter",
            "form_FORM_ID_alter",
            "form_BASE_FORM_ID_alter",
        ],
        &["Drupal\\Core\\Form\\FormBuilder::prepareForm"],
    ),
    (
        &["query_alter", "query_TAG_alter"],
        &[
            "Drupal\\Core\\Database\\Query\\Select::preExecute",
            "Drupal\\Core\\Entity\\Plugin\\EntityReferenceSelection\\DefaultSelection::reAlterQuery",
        ],
    ),
    (
        &["field_widget_single_element_WIDGET_TYPE_form_alter"],
        &["Drupal\\Core\\Field\\WidgetBase::formSingleElement"],
    ),
    (
        &["field_widget_complete_WIDGET_TYPE_form_alter"],
        &["Drupal\\Core\\Field\\WidgetBase::form"],
    ),
    (
        &["block_view_BASE_BLOCK_ID_alter"],
        &["Drupal\\block\\BlockViewBuilder::buildPreRenderableBlock"],
    ),
    (
        &["block_build_BASE_BLOCK_ID_alter"],
        &["Drupal\\block\\BlockViewBuilder::viewMultiple"],
    ),
    (
        &[
            "theme_suggestions_HOOK",
            "theme_suggestions_alter",
            "theme_suggestions_HOOK_alter",
        ],
        &["Drupal\\Core\\Theme\\ThemeManager::buildThemeHookSuggestions"],
    ),
    (
        &["preprocess_HOOK"],
        &["Drupal\\Core\\Theme\\Registry::collectModulePreprocess"],
    ),
    (
        &[
            "plugin_filter_TYPE_alter",
            "plugin_filter_TYPE__CONSUMER_alter",
        ],
        &["Drupal\\Core\\Plugin\\FilteredPluginManagerTrait::getFilteredDefinitions"],
    ),
    (
        &[
            "language_fallback_candidates_alter",
            "language_fallback_candidates_OPERATION_alter",
        ],
        &["Drupal\\language\\ConfigurableLanguageManager::getFallbackCandidates"],
    ),
    (
        &["migrate_MIGRATION_ID_prepare_row"],
        &["Drupal\\migrate\\Plugin\\migrate\\source\\SourcePluginBase::prepareRow"],
    ),
    (
        &["jsonapi_ENTITY_TYPE_filter_access"],
        &["Drupal\\jsonapi\\Access\\TemporaryQueryGuard::getAccessResultsFromEntityFilterHook"],
    ),
];

/// One piece of a hook name pattern.
enum Piece<'a> {
    Text(&'a str),
    /// An uppercase run, such as `FORM_ID` or `ENTITY_TYPE`.
    Placeholder(&'a str),
}

/// A pattern split into text and placeholders: an uppercase run starts a
/// placeholder at the start of the pattern or after an underscore, and goes on
/// through underscores followed by more uppercase.
fn pieces(pattern: &str) -> Vec<Piece<'_>> {
    let bytes = pattern.as_bytes();
    let mut pieces = Vec::new();
    let (mut text_start, mut i) = (0, 0);
    while i < bytes.len() {
        let starts = bytes[i].is_ascii_uppercase() && (i == 0 || bytes[i - 1] == b'_');
        if !starts {
            i += 1;
            continue;
        }
        let mut end = i;
        loop {
            while end < bytes.len()
                && (bytes[end].is_ascii_uppercase() || bytes[end].is_ascii_digit())
            {
                end += 1;
            }
            if end + 1 < bytes.len() && bytes[end] == b'_' && bytes[end + 1].is_ascii_uppercase() {
                end += 1;
            } else {
                break;
            }
        }
        if text_start < i {
            pieces.push(Piece::Text(&pattern[text_start..i]));
        }
        pieces.push(Piece::Placeholder(&pattern[i..end]));
        text_start = end;
        i = end;
    }
    if text_start < bytes.len() {
        pieces.push(Piece::Text(&pattern[text_start..]));
    }
    pieces
}

/// Whether `hook` matches `pattern`: `ENTITY_TYPE` stands for an entity type id,
/// `N` for digits, any other placeholder for a non-empty run of lowercase letters,
/// digits and underscores.
pub(crate) fn matches_pattern(
    pattern: &str,
    hook: &str,
    is_entity_type: &dyn Fn(&str) -> bool,
) -> bool {
    fn rest(pieces: &[Piece<'_>], hook: &str, is_entity_type: &dyn Fn(&str) -> bool) -> bool {
        let Some((first, others)) = pieces.split_first() else {
            return hook.is_empty();
        };
        match first {
            Piece::Text(text) => hook
                .strip_prefix(text)
                .is_some_and(|after| rest(others, after, is_entity_type)),
            Piece::Placeholder(name) => (1..=hook.len()).any(|length| {
                let (value, after) = hook.split_at(length);
                let fits = match *name {
                    "ENTITY_TYPE" => is_entity_type(value),
                    "N" => value.bytes().all(|b| b.is_ascii_digit()),
                    _ => value
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
                };
                fits && rest(others, after, is_entity_type)
            }),
        }
    }
    rest(&pieces(pattern), hook, is_entity_type)
}

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
    /// Documented names with placeholders, sorted.
    documented_patterns: Vec<String>,
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
        let mut documented_patterns: Vec<String> = docs
            .keys()
            .filter(|name| name.bytes().any(|b| b.is_ascii_uppercase()))
            .cloned()
            .collect();
        documented_patterns.sort();
        self.implementations = implementations;
        self.docs = docs;
        self.documented_patterns = documented_patterns;
    }

    /// From a hook name or alter type at position `argument` of an invoking call: the
    /// implementations the call reaches, then the hook's documentation.
    pub(crate) fn invocation_targets(
        &self,
        index: &Index,
        targets: &[CallTarget],
        strings: &[Option<String>],
        argument: usize,
        value: &str,
        is_entity_type: &dyn Fn(&str) -> bool,
    ) -> Vec<(Location, String)> {
        let Some((invoker, themes)) = invoker_of(index, targets) else {
            return Vec::new();
        };
        if argument != invoker.hook {
            return Vec::new();
        }
        let hook = invoked_hook(invoker, value);
        let limited = invoker
            .extension
            .and_then(|position| strings.get(position)?.as_deref());
        let mut found: Vec<(Location, String)> = self
            .implementations(&hook)
            .iter()
            .filter(|i| i.kind.is_theme() == themes && limited.is_none_or(|e| e == i.extension))
            .map(|i| {
                let location = Location {
                    path: i.path.clone(),
                    start: i.start,
                    end: i.end,
                };
                (location, i.name.clone())
            })
            .collect();
        found.extend(self.doc_targets(&hook, is_entity_type));
        found
    }

    /// From a hook implementation: the invocations of each hook it implements, then
    /// their documentation.
    pub(crate) fn implementation_targets(
        &self,
        index: &Index,
        declaration: &FrameworkDeclaration<'_>,
        is_entity_type: &dyn Fn(&str) -> bool,
    ) -> Vec<(Location, String)> {
        let implemented = self.declared_hooks(index, declaration);
        if implemented.is_empty() {
            return Vec::new();
        }
        let calls = index.calls_named(&invoking_methods());
        let mut sites: Vec<&FoundCall> = calls
            .iter()
            .filter(|call| {
                implemented
                    .iter()
                    .any(|i| invokes(index, call, i) || computes(index, call, i, is_entity_type))
            })
            .collect();
        sites.sort_by(|a, b| {
            (
                references::rank(&a.location.path),
                &a.location.path,
                a.location.start,
            )
                .cmp(&(
                    references::rank(&b.location.path),
                    &b.location.path,
                    b.location.start,
                ))
        });
        let mut found: Vec<(Location, String)> = sites
            .into_iter()
            .map(|call| {
                let name = match call.targets.first() {
                    Some(CallTarget::Method { name, .. }) => name.clone(),
                    _ => String::new(),
                };
                (call.location.clone(), name)
            })
            .collect();
        for implementation in &implemented {
            for doc in self.doc_targets(&implementation.hook, is_entity_type) {
                if !found.contains(&doc) {
                    found.push(doc);
                }
            }
        }
        found
    }

    /// The hooks a declaration implements, by the rules of the registry applied to
    /// the declaration as the editor has it.
    fn declared_hooks(
        &self,
        index: &Index,
        declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<HookImplementation> {
        match declaration {
            FrameworkDeclaration::Function {
                path,
                declaration,
                file,
            } => {
                let file: Vec<&Declaration> = file.iter().collect();
                self.file_hooks(index, path, &file)
                    .into_iter()
                    .filter(|i| i.start == declaration.name_start)
                    .collect()
            }
            FrameworkDeclaration::Method {
                path,
                class,
                member,
            } => self
                .file_hooks(index, path, &[class])
                .into_iter()
                .filter(|i| i.start == member.name_start)
                .collect(),
            FrameworkDeclaration::Class {
                path,
                class,
                metadata,
            } => {
                if !is_hook(index, metadata) {
                    return Vec::new();
                }
                let Some(hook) = hook_name(metadata) else {
                    return Vec::new();
                };
                self.file_hooks(index, path, &[class])
                    .into_iter()
                    .filter(|i| i.hook == hook)
                    .collect()
            }
        }
    }

    /// The documentation of `hook`: its own, or that of every documented pattern it
    /// matches.
    fn doc_targets(
        &self,
        hook: &str,
        is_entity_type: &dyn Fn(&str) -> bool,
    ) -> Vec<(Location, String)> {
        let exact = self.docs(hook);
        let docs: Vec<&HookDoc> = if exact.is_empty() {
            self.documented_patterns
                .iter()
                .filter(|pattern| matches_pattern(pattern, hook, is_entity_type))
                .flat_map(|pattern| self.docs(pattern))
                .collect()
        } else {
            exact.iter().collect()
        };
        docs.into_iter()
            .map(|doc| {
                let location = Location {
                    path: doc.path.clone(),
                    start: doc.start,
                    end: doc.end,
                };
                (location, format!("hook_{}", doc.name))
            })
            .collect()
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

/// Whether `call` invokes `implementation`'s hook by its literal name, for the
/// implementation's group and extension.
fn invokes(index: &Index, call: &FoundCall, implementation: &HookImplementation) -> bool {
    let Some((invoker, themes)) = invoker_of(index, &call.targets) else {
        return false;
    };
    if themes != implementation.kind.is_theme() {
        return false;
    }
    let limited = invoker
        .extension
        .and_then(|position| call.strings.get(position)?.as_deref());
    if limited.is_some_and(|e| e != implementation.extension) {
        return false;
    }
    let named = call.strings.get(invoker.hook).cloned().flatten();
    let listed = if invoker.alter {
        call.lists.get(invoker.hook).cloned().unwrap_or_default()
    } else {
        Vec::new()
    };
    named
        .into_iter()
        .chain(listed)
        .any(|name| invoked_hook(invoker, &name) == implementation.hook)
}

/// Whether `call` is an invoking call of the implementation's group inside a method
/// the table lists for a pattern the implementation's hook matches.
fn computes(
    index: &Index,
    call: &FoundCall,
    implementation: &HookImplementation,
    is_entity_type: &dyn Fn(&str) -> bool,
) -> bool {
    let Some(caller) = call.caller.as_deref() else {
        return false;
    };
    let in_group = invoker_of(index, &call.targets)
        .is_some_and(|(_, themes)| themes == implementation.kind.is_theme());
    in_group
        && COMPUTED.iter().any(|(patterns, sites)| {
            sites.iter().any(|site| site.eq_ignore_ascii_case(caller))
                && patterns
                    .iter()
                    .any(|pattern| matches_pattern(pattern, &implementation.hook, is_entity_type))
        })
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
