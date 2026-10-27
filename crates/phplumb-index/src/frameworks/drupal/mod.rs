//! Drupal: services, entity types, hooks, plugins and routes.

mod entities;
mod extensions;
mod hooks;
mod plugins;
mod routes;
mod services;
mod yaml;

use std::any::Any;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::frameworks::{CallTarget, Framework, FrameworkCall, FrameworkDeclaration};
use crate::index::{Index, Location};
use crate::phpdoc::TypeExpr;

pub use entities::EntityType;
use entities::EntityTypes;
pub use hooks::{ExtensionKind, HookDoc, HookImplementation};
use hooks::{Hooks, Storage};
use plugins::Plugins;
pub use plugins::{Plugin, PluginType};
pub use routes::RouteDefinition;
use routes::Routes;
pub use services::ServiceDefinition;
use services::Services;

#[derive(Default)]
pub struct Drupal {
    services: Services,
    entities: EntityTypes,
    hooks: Hooks,
    plugins: Plugins,
    routes: Routes,
}

impl Drupal {
    /// Whether `id` is an entity type stored as `storage` asks.
    fn has_entity_type(&self, index: &Index, id: &str, storage: Storage) -> bool {
        let config = |entity: &EntityType| is_a(index, &entity.storage, &[CONFIG_STORAGE]);
        let found = self.entities.get(id);
        match storage {
            Storage::Any => !found.is_empty(),
            Storage::Config => found.iter().any(config),
            Storage::Content => found.iter().any(|entity| !config(entity)),
        }
    }

    /// Every definition of the route `name`, sorted by path.
    pub fn routes(&self, name: &str) -> &[RouteDefinition] {
        self.routes.get(name)
    }

    /// Every plugin type, sorted by manager.
    pub fn plugin_types(&self) -> &[PluginType] {
        self.plugins.types()
    }

    /// Every plugin of `manager`, sorted by path and offset.
    pub fn plugins_of(&self, manager: &str) -> &[Plugin] {
        self.plugins.of(manager)
    }

    /// The plugins of `manager` an id names: that id, or for `base:derivative` with
    /// none, `base`.
    pub fn plugins(&self, manager: &str, id: &str) -> Vec<&Plugin> {
        self.plugins.named(manager, id)
    }

    /// Every implementation of `hook`, sorted by path and offset.
    pub fn hook_implementations(&self, hook: &str) -> &[HookImplementation] {
        self.hooks.implementations(hook)
    }

    /// Every `hook_{name}` documentation function.
    pub fn hook_docs(&self, name: &str) -> &[HookDoc] {
        self.hooks.docs(name)
    }

    /// Every declaration of the entity type `id`, outside test modules.
    pub fn entity_types(&self, id: &str) -> &[EntityType] {
        self.entities.get(id)
    }

    /// Every definition of `id` on disk.
    pub fn services(&self, id: &str) -> &[ServiceDefinition] {
        self.services.definitions(id)
    }

    /// The classes a service id can give, sorted: through aliases, parents, an id
    /// that names an indexed class, and every service decorating it. Empty when
    /// unknown.
    pub fn service_classes(&self, index: &Index, id: &str) -> Vec<String> {
        self.services.classes(index, id)
    }
}

impl Framework for Drupal {
    fn name(&self) -> &'static str {
        "drupal"
    }

    /// A Drupal project's `composer.lock` lists `drupal/core`.
    fn detect(&self, root: &Path) -> bool {
        let Ok(text) = std::fs::read(root.join("composer.lock")) else {
            return false;
        };
        let Ok(lock) = serde_json::from_slice::<serde_json::Value>(&text) else {
            return false;
        };
        ["packages", "packages-dev"].iter().any(|section| {
            lock[section]
                .as_array()
                .is_some_and(|packages| packages.iter().any(|p| p["name"] == "drupal/core"))
        })
    }

    fn rebuild(&mut self, index: &Index) {
        self.services = Services::read(index.filter());
        self.entities = EntityTypes::read(index);
        self.hooks = Hooks::read(index);
        self.plugins = Plugins::read(index, self.hooks.extensions());
        self.routes = Routes::read(index.filter());
    }

    fn update(&mut self, index: &Index, changed: &[PathBuf], removed: &[PathBuf]) {
        self.services.update(index.filter(), changed, removed);
        self.entities = EntityTypes::read(index);
        self.hooks.update(index, changed, removed);
        self.plugins
            .update(index, self.hooks.extensions(), changed, removed);
        self.routes.update(index.filter(), changed, removed);
    }

    /// `\Drupal::service(id)` and a container's `get(id)` give the service's classes;
    /// `getStorage(id)` gives the entity type's storage, tagged with the entity type,
    /// and that storage's loading methods give the entity class.
    fn call_type(&self, index: &Index, call: &FrameworkCall<'_>) -> Option<TypeExpr> {
        if let Some(id) = service_call(index, call) {
            let classes = self.services.classes(index, id);
            return one_of(classes.into_iter().map(TypeExpr::Named).collect());
        }
        if let Some(id) = storage_call(index, call) {
            // A storage class the index lacks would hide the declared return type.
            return one_of(
                self.entities
                    .get(id)
                    .iter()
                    .filter(|entity| index.has_class(&entity.storage))
                    .map(|entity| TypeExpr::Tagged {
                        class: entity.storage.clone(),
                        tag: format!("{ENTITY_TAG}{id}"),
                    })
                    .collect(),
            );
        }
        let (ids, many) = entity_call(index, call)?;
        let entities: Vec<TypeExpr> = ids
            .iter()
            .flat_map(|id| self.entities.get(id))
            .map(|entity| TypeExpr::Named(entity.class.clone()))
            .collect();
        let entity = one_of(entities)?;
        Some(if many {
            TypeExpr::ArrayOf(Box::new(entity))
        } else {
            entity
        })
    }

    /// The id of a service call leads to every definition of the service; the id of
    /// `getStorage()` to the `id` of every declaration of the entity type; a hook
    /// name or alter type in an invocation to the hook's implementations and
    /// documentation.
    fn string_targets(
        &self,
        index: &Index,
        call: &FrameworkCall<'_>,
        argument: usize,
        value: &str,
    ) -> Vec<(Location, String)> {
        if argument == 0 && service_call(index, call) == Some(value) {
            return self
                .services
                .definitions(value)
                .iter()
                .map(|definition| {
                    let location = Location {
                        path: definition.path.clone(),
                        start: definition.start,
                        end: definition.end,
                    };
                    (location, value.to_string())
                })
                .collect();
        }
        if argument == 0 && storage_call(index, call) == Some(value) {
            return self
                .entities
                .get(value)
                .iter()
                .filter_map(|entity| entity.id_location())
                .map(|location| (location, value.to_string()))
                .collect();
        }
        let plugins = self.plugins.call_targets(index, call, argument, value);
        if !plugins.is_empty() {
            return plugins;
        }
        let entity_types = |id: &str, storage: Storage| self.has_entity_type(index, id, storage);
        self.hooks.invocation_targets(
            index,
            call.targets,
            call.strings,
            argument,
            value,
            &entity_types,
        )
    }

    /// A render element type or a widget or formatter type in display options leads
    /// to its plugin class.
    fn keyed_string_targets(
        &self,
        index: &Index,
        key: &str,
        value: &str,
        call: Option<(&FrameworkCall<'_>, usize)>,
    ) -> Vec<(Location, String)> {
        self.plugins.keyed_targets(index, key, value, call)
    }

    /// A plugin manager, or an interface only one plugin manager implements, has its
    /// plugins as implementations.
    fn class_implementations(&self, index: &Index, class: &str) -> Vec<(Location, String)> {
        self.plugins
            .manager_of(index, &[class.to_string()])
            .map(|manager| self.plugins.all_targets(&manager))
            .unwrap_or_default()
    }

    /// A hook implementation leads to the invocations of its hook and to the hook's
    /// documentation.
    fn declaration_targets(
        &self,
        index: &Index,
        declaration: &FrameworkDeclaration<'_>,
    ) -> Vec<(Location, String)> {
        let entity_types = |id: &str, storage: Storage| self.has_entity_type(index, id, storage);
        self.hooks
            .implementation_targets(index, declaration, &entity_types)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

const CONFIG_STORAGE: &str = "Drupal\\Core\\Config\\Entity\\ConfigEntityStorage";

/// The tag a storage carries: this prefix, then the entity type id.
const ENTITY_TAG: &str = "drupal.entity:";

/// Storage methods that return one entity, and those that return several.
const LOAD_ONE: [&str; 4] = ["load", "loadUnchanged", "loadRevision", "create"];
const LOAD_MANY: [&str; 2] = ["loadMultiple", "loadByProperties"];

/// One type, or their union; nothing when there is none.
fn one_of(mut types: Vec<TypeExpr>) -> Option<TypeExpr> {
    match types.len() {
        0 => None,
        1 => types.pop(),
        _ => Some(TypeExpr::Union(types)),
    }
}

/// The entity type id `call` asks for, when it is `getStorage(id)` on an entity type
/// manager with a literal id.
fn storage_call<'a>(index: &Index, call: &'a FrameworkCall<'_>) -> Option<&'a str> {
    let id = call.strings.first()?.as_deref()?;
    let asks_for_storage = call.targets.iter().any(|target| match target {
        CallTarget::Method { class, name } => {
            name.eq_ignore_ascii_case("getStorage")
                && is_a(
                    index,
                    class,
                    &["Drupal\\Core\\Entity\\EntityTypeManagerInterface"],
                )
        }
        CallTarget::Function(_) => false,
    });
    asks_for_storage.then_some(id)
}

/// The entity type ids of a loading call on a tagged storage, and whether it loads
/// several entities.
fn entity_call(index: &Index, call: &FrameworkCall<'_>) -> Option<(Vec<String>, bool)> {
    let mut ids = Vec::new();
    collect_tags(call.receiver?, &mut ids);
    if ids.is_empty() {
        return None;
    }
    let mut many = None;
    for target in call.targets {
        let CallTarget::Method { class, name } = target else {
            continue;
        };
        if !is_a(
            index,
            class,
            &["Drupal\\Core\\Entity\\EntityStorageInterface"],
        ) {
            continue;
        }
        if LOAD_ONE.iter().any(|m| m.eq_ignore_ascii_case(name)) {
            many = Some(false);
        } else if LOAD_MANY.iter().any(|m| m.eq_ignore_ascii_case(name)) {
            many = Some(true);
        }
    }
    Some((ids, many?))
}

/// The entity type ids tagged anywhere in `ty`.
fn collect_tags(ty: &TypeExpr, ids: &mut Vec<String>) {
    match ty {
        TypeExpr::Tagged { tag, .. } => {
            if let Some(id) = tag.strip_prefix(ENTITY_TAG)
                && !ids.iter().any(|known| known == id)
            {
                ids.push(id.to_string());
            }
        }
        TypeExpr::Union(members) => members.iter().for_each(|m| collect_tags(m, ids)),
        _ => {}
    }
}

/// Container interfaces whose `get(id)` returns a service.
const CONTAINERS: [&str; 2] = [
    "Symfony\\Component\\DependencyInjection\\ContainerInterface",
    "Psr\\Container\\ContainerInterface",
];

/// The service id `call` asks for, when it is `\Drupal::service(id)` or a
/// container's `get(id)` with a literal id.
fn service_call<'a>(index: &Index, call: &'a FrameworkCall<'_>) -> Option<&'a str> {
    let id = call.strings.first()?.as_deref()?;
    let asks_for_service = call.targets.iter().any(|target| match target {
        CallTarget::Method { class, name } => {
            (class.eq_ignore_ascii_case("Drupal") && name.eq_ignore_ascii_case("service"))
                || (name.eq_ignore_ascii_case("get") && is_a(index, class, &CONTAINERS))
        }
        CallTarget::Function(_) => false,
    });
    asks_for_service.then_some(id)
}

/// Whether `class` is, extends or implements one of `ancestors`.
pub(super) fn is_a(index: &Index, class: &str, ancestors: &[&str]) -> bool {
    let mut seen: HashSet<String> = HashSet::new();
    let mut pending = vec![class.to_string()];
    while let Some(next) = pending.pop() {
        if ancestors.iter().any(|c| c.eq_ignore_ascii_case(&next)) {
            return true;
        }
        if !seen.insert(next.to_ascii_lowercase()) {
            continue;
        }
        for supertype in index.supertypes(&next) {
            pending.push(supertype.declaration.fqn.clone());
        }
    }
    false
}
