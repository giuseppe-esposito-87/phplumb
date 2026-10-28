//! Drupal entity types, declared by attributes or annotations on entity classes.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::filter::PathFilter;
use crate::index::{Index, Location};
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

impl EntityType {
    /// Where the id is written in the declaration: the quoted value of the `id` key,
    /// or else the first quoted string that holds exactly the id.
    pub fn id_location(&self) -> Option<Location> {
        let text = std::fs::read(&self.path).ok()?;
        let (start, end) = (self.span.0 as usize, self.span.1 as usize);
        let declaration = text.get(start..end)?;
        let id = self.id.as_bytes();
        let mut first = None;
        for &quote in b"'\"" {
            let needle: Vec<u8> = [&[quote][..], id, &[quote][..]].concat();
            let mut from = 0;
            while let Some(at) = declaration[from..]
                .windows(needle.len())
                .position(|w| w == needle)
                .map(|i| from + i)
            {
                from = at + 1;
                first = Some(first.map_or(at, |f: usize| f.min(at)));
                if follows_id_key(&declaration[..at]) {
                    return Some(self.location_at(start + at + 1));
                }
            }
        }
        first.map(|at| self.location_at(start + at + 1))
    }

    fn location_at(&self, start: usize) -> Location {
        Location {
            path: self.path.clone(),
            start: start as u32,
            end: (start + self.id.len()) as u32,
        }
    }
}

/// Whether `before` ends with the `id` key: `id:` in attributes, `id =` in
/// annotations.
fn follows_id_key(before: &[u8]) -> bool {
    let before = before.trim_ascii_end();
    let Some(before) = before
        .strip_suffix(b":")
        .or_else(|| before.strip_suffix(b"="))
    else {
        return false;
    };
    let before = before.trim_ascii_end();
    before.ends_with(b"id")
        && before
            .len()
            .checked_sub(3)
            .and_then(|i| before.get(i))
            .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_'))
}

#[derive(Default)]
pub(crate) struct EntityTypes {
    by_id: HashMap<String, Vec<EntityType>>,
}

impl EntityTypes {
    /// Every entity type the index's classes declare, outside test modules.
    pub(crate) fn read(index: &Index, filter: &PathFilter) -> EntityTypes {
        let root = filter.root();
        let mut by_id: HashMap<String, Vec<EntityType>> = HashMap::new();
        for (path, declaration) in index.declarations() {
            if !filter.in_scope(path) {
                continue;
            }
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
