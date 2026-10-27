//! Drupal entity types, declared by attributes or annotations on entity classes.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::index::Index;
use crate::metadata::Metadata;

/// One declaration of an entity type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntityType {
    pub id: String,
    pub class: String,
    pub storage: String,
    pub path: PathBuf,
    /// The whole attribute or annotation in the file.
    pub span: (u32, u32),
}

/// Content and config entity types, with their default storage classes.
const KINDS: [(&str, Option<&str>); 3] = [
    (
        "ContentEntityType",
        Some("Drupal\\Core\\Entity\\Sql\\SqlContentEntityStorage"),
    ),
    (
        "ConfigEntityType",
        Some("Drupal\\Core\\Config\\Entity\\ConfigEntityStorage"),
    ),
    ("EntityType", None),
];

#[derive(Default)]
pub(crate) struct EntityTypes {
    by_id: HashMap<String, Vec<EntityType>>,
}

impl EntityTypes {
    /// Every entity type the index's classes declare, outside test modules.
    pub(crate) fn read(index: &Index) -> EntityTypes {
        let root = index.filter().root();
        let mut by_id: HashMap<String, Vec<EntityType>> = HashMap::new();
        for (path, declaration) in index.declarations() {
            let Some(shape) = &declaration.class else {
                continue;
            };
            if shape.metadata.is_empty() {
                continue;
            }
            let in_tests = path
                .strip_prefix(root)
                .is_ok_and(|relative| relative.components().any(|c| c.as_os_str() == "tests"));
            if in_tests {
                continue;
            }
            for metadata in &shape.metadata {
                let Some(entity) = entity_type(metadata, &declaration.fqn, path) else {
                    continue;
                };
                let entries = by_id.entry(entity.id.clone()).or_default();
                if !entries
                    .iter()
                    .any(|e| e.class == entity.class && e.storage == entity.storage)
                {
                    entries.push(entity);
                }
            }
        }
        for entries in by_id.values_mut() {
            entries.sort_by(|a, b| (&a.class, &a.path).cmp(&(&b.class, &b.path)));
        }
        EntityTypes { by_id }
    }

    pub(crate) fn get(&self, id: &str) -> &[EntityType] {
        self.by_id.get(id).map(Vec::as_slice).unwrap_or_default()
    }
}

/// The entity type `metadata` declares on `class`. Attributes must name Drupal's
/// attribute classes; annotations match by their short name, as Drupal reads them.
fn entity_type(metadata: &Metadata, class: &str, path: &std::path::Path) -> Option<EntityType> {
    let default_storage = KINDS.iter().find_map(|(kind, storage)| {
        let matches = if metadata.annotation {
            metadata.name.rsplit('\\').next() == Some(*kind)
        } else {
            metadata.name == format!("Drupal\\Core\\Entity\\Attribute\\{kind}")
        };
        matches.then_some(*storage)
    })?;
    let id = metadata.argument("id")?.text()?.to_string();
    let storage = metadata
        .argument("handlers")
        .and_then(|handlers| handlers.entry("storage"))
        .and_then(|storage| storage.text())
        .map(|storage| storage.trim_start_matches('\\').to_string())
        .or_else(|| default_storage.map(str::to_string))?;
    Some(EntityType {
        id,
        class: class.to_string(),
        storage,
        path: path.to_path_buf(),
        span: metadata.span,
    })
}
