//! Workspace-wide symbol table and the definition query.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::discover::{self, DiscoverOptions, discover_with};
use crate::filter::PathFilter;
use crate::summary::{Declaration, NameKind, NameRef, SymbolKind, summarize};

/// A byte range in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, Default)]
pub struct IndexStats {
    pub files: usize,
    pub declarations: usize,
    pub files_with_parse_errors: usize,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateStats {
    pub reparsed: usize,
    pub removed: usize,
    pub elapsed_ms: u128,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    file: u32,
    start: u32,
    end: u32,
}

/// One indexed file. Its declarations are kept so their keys can be removed from the
/// symbol tables when the file changes.
struct FileEntry {
    path: PathBuf,
    declarations: Vec<Declaration>,
}

/// Declarations of every PHP file under a root. Keys are lowercased fully qualified
/// names without a leading backslash, because PHP class and function names are
/// case-insensitive. Files are slots; a removed file leaves an empty slot.
pub struct Index {
    filter: PathFilter,
    files: Vec<Option<FileEntry>>,
    ids: HashMap<PathBuf, u32>,
    classes: HashMap<String, Vec<Entry>>,
    functions: HashMap<String, Vec<Entry>>,
}

impl Index {
    pub fn build(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<(Index, IndexStats)> {
        let started = Instant::now();
        let mut filter = PathFilter::new(root, opts)?;
        let paths = discover_with(&mut filter);
        let summaries = summarize_files(&paths);

        let mut index = Index {
            filter,
            files: Vec::new(),
            ids: HashMap::new(),
            classes: HashMap::new(),
            functions: HashMap::new(),
        };
        let mut stats = IndexStats {
            files: paths.len(),
            ..IndexStats::default()
        };
        for (path, (declarations, has_errors)) in paths.into_iter().zip(summaries) {
            stats.files_with_parse_errors += usize::from(has_errors);
            stats.declarations += declarations.len();
            index.insert(path, declarations);
        }
        stats.elapsed_ms = started.elapsed().as_millis();
        Ok((index, stats))
    }

    /// Applies file system changes. `changed` holds created or modified paths;
    /// `removed` holds deleted files or directories. Changed paths that no longer
    /// exist or that the filter rejects are treated as removed.
    pub fn update(&mut self, changed: &[PathBuf], removed: &[PathBuf]) -> UpdateStats {
        let started = Instant::now();
        let mut stats = UpdateStats::default();

        let mut gone: Vec<PathBuf> = removed.iter().map(|p| self.filter.under_root(p)).collect();
        let mut fresh: Vec<PathBuf> = Vec::new();
        for path in changed {
            let path = self.filter.under_root(path);
            let chosen = discover::chosen_path(self.filter.root(), &path).map(|(_, chosen)| chosen);
            match chosen {
                Some(chosen) if self.filter.accepts(&chosen) => fresh.push(chosen),
                _ => gone.push(path),
            }
        }

        stats.removed += self.remove_paths(&gone);
        for path in &fresh {
            if let Some(&id) = self.ids.get(path) {
                self.remove(id);
            }
        }
        let summaries = summarize_files(&fresh);
        stats.reparsed = fresh.len();
        for (path, (declarations, _)) in fresh.into_iter().zip(summaries) {
            self.insert(path, declarations);
        }
        stats.elapsed_ms = started.elapsed().as_millis();
        stats
    }

    /// Removes indexed files equal to, or inside, any of `paths`. Returns how many.
    fn remove_paths(&mut self, paths: &[PathBuf]) -> usize {
        let mut ids: Vec<u32> = Vec::new();
        let mut directories: Vec<&PathBuf> = Vec::new();
        for path in paths {
            match self.ids.get(path) {
                Some(&id) => ids.push(id),
                None => directories.push(path),
            }
        }
        if !directories.is_empty() {
            ids.extend(
                self.ids
                    .iter()
                    .filter(|(path, _)| directories.iter().any(|dir| path.starts_with(dir)))
                    .map(|(_, &id)| id),
            );
        }
        ids.sort_unstable();
        ids.dedup();
        for &id in &ids {
            self.remove(id);
        }
        ids.len()
    }

    fn insert(&mut self, path: PathBuf, declarations: Vec<Declaration>) {
        let file = self.files.len() as u32;
        for declaration in &declarations {
            self.table_mut(declaration.kind)
                .entry(declaration.fqn.to_ascii_lowercase())
                .or_default()
                .push(Entry {
                    file,
                    start: declaration.name_start,
                    end: declaration.name_end,
                });
        }
        self.ids.insert(path.clone(), file);
        self.files.push(Some(FileEntry { path, declarations }));
    }

    fn remove(&mut self, file: u32) {
        let Some(entry) = self.files[file as usize].take() else {
            return;
        };
        self.ids.remove(&entry.path);
        for declaration in &entry.declarations {
            let key = declaration.fqn.to_ascii_lowercase();
            let table = self.table_mut(declaration.kind);
            if let Some(entries) = table.get_mut(&key) {
                entries.retain(|e| e.file != file);
                if entries.is_empty() {
                    table.remove(&key);
                }
            }
        }
    }

    fn table_mut(&mut self, kind: SymbolKind) -> &mut HashMap<String, Vec<Entry>> {
        match kind {
            SymbolKind::Function => &mut self.functions,
            _ => &mut self.classes,
        }
    }

    /// Definitions of the name at `offset` in `path`, whose current content is
    /// `source`. Results come only from resolved names and are sorted by path and
    /// offset; an unknown or unresolvable name returns nothing.
    pub fn definition(&self, path: &Path, source: &[u8], offset: u32) -> Vec<Location> {
        let summary = summarize(source);

        if let Some(declaration) = summary
            .declarations
            .iter()
            .find(|d| d.name_start <= offset && offset <= d.name_end)
        {
            return vec![Location {
                path: path.to_path_buf(),
                start: declaration.name_start,
                end: declaration.name_end,
            }];
        }

        let Some(name) = name_at(&summary.names, offset) else {
            return Vec::new();
        };
        let entries: &[Entry] = match name.kind {
            NameKind::ClassLike => self.lookup(&self.classes, &name.fqn),
            NameKind::Function => {
                let found = self.lookup(&self.functions, &name.fqn);
                match name.fqn.rsplit_once('\\') {
                    // PHP falls back to the global function only for unqualified,
                    // non-imported names.
                    Some((_, short)) if found.is_empty() && !name.qualified && !name.imported => {
                        self.lookup(&self.functions, short)
                    }
                    _ => found,
                }
            }
            NameKind::Constant => &[],
        };

        let mut locations: Vec<Location> = entries
            .iter()
            .map(|entry| Location {
                path: self.files[entry.file as usize]
                    .as_ref()
                    .expect("symbol tables only reference live files")
                    .path
                    .clone(),
                start: entry.start,
                end: entry.end,
            })
            .collect();
        locations.sort_by(|a, b| (&a.path, a.start).cmp(&(&b.path, b.start)));
        locations
    }

    fn lookup<'a>(&self, table: &'a HashMap<String, Vec<Entry>>, fqn: &str) -> &'a [Entry] {
        table
            .get(&fqn.to_ascii_lowercase())
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

/// The name covering `offset`. A name that strictly contains the offset wins over
/// one that ends exactly at it.
fn name_at(names: &[NameRef], offset: u32) -> Option<&NameRef> {
    names
        .iter()
        .find(|n| n.start <= offset && offset < n.end)
        .or_else(|| names.iter().find(|n| n.end == offset))
}

/// Declarations and parse-error flag of each file, in parallel. Unreadable files
/// yield no declarations rather than failing.
fn summarize_files(paths: &[PathBuf]) -> Vec<(Vec<Declaration>, bool)> {
    paths
        .par_iter()
        .map(|path| match std::fs::read(path) {
            Ok(source) => {
                let summary = summarize(&source);
                (summary.declarations, summary.has_parse_errors)
            }
            Err(_) => (Vec::new(), false),
        })
        .collect()
}
