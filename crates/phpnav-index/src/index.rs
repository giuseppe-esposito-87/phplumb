//! Workspace-wide symbol table and the definition query.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

use rayon::prelude::*;

use crate::cache::{self, CachedFile};
use crate::discover::{self, DiscoverOptions, discover_in, discover_with};
use crate::filter::PathFilter;
use crate::hover::{self, Hover};
use crate::infer;
use crate::members;
use crate::phpdoc::TypeExpr;
use crate::summary::{
    ClassSel, Declaration, Member, MemberKind, NameKind, NameRef, SymbolKind, summarize,
};

/// A byte range in a file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub path: PathBuf,
    pub start: u32,
    pub end: u32,
}

/// `reused` files came from the cache; `reparsed` files were read and parsed.
#[derive(Clone, Debug, Default)]
pub struct IndexStats {
    pub files: usize,
    pub declarations: usize,
    pub files_with_parse_errors: usize,
    pub reused: usize,
    pub reparsed: usize,
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
    /// Index of the declaration in its file.
    decl: u32,
    start: u32,
    end: u32,
}

/// A class-like declaration found in the index.
#[derive(Clone, Copy, Debug)]
pub struct ClassRef<'a> {
    pub path: &'a Path,
    pub declaration: &'a Declaration,
}

/// Declarations of every PHP file under a root. Keys are lowercased fully qualified
/// names without a leading backslash, because PHP class and function names are
/// case-insensitive. Files are slots; a removed file leaves an empty slot. Each file
/// keeps its declarations, so their keys can be removed when it changes, and its size
/// and modification time, so a cache can tell whether it is still current.
pub struct Index {
    filter: PathFilter,
    files: Vec<Option<CachedFile>>,
    ids: HashMap<PathBuf, u32>,
    classes: HashMap<String, Vec<Entry>>,
    functions: HashMap<String, Vec<Entry>>,
    constants: HashMap<String, Vec<Entry>>,
}

impl Index {
    pub fn build(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<(Index, IndexStats)> {
        Self::build_from(root, opts, HashMap::new())
    }

    /// Loads `cache` when it was written by this engine version for the same root and
    /// options, reuses entries whose file size and modification time are unchanged,
    /// and parses the rest. Any cache problem means a full build.
    pub fn build_with_cache(
        root: &Path,
        opts: &DiscoverOptions,
        cache: &Path,
    ) -> anyhow::Result<(Index, IndexStats)> {
        let filter = PathFilter::new(root, opts)?;
        let cached = cache::load(cache, &filter, cache::ENGINE);
        Self::build_from(root, opts, cached)
    }

    fn build_from(
        root: &Path,
        opts: &DiscoverOptions,
        mut cached: HashMap<PathBuf, CachedFile>,
    ) -> anyhow::Result<(Index, IndexStats)> {
        let started = Instant::now();
        let mut filter = PathFilter::new(root, opts)?;
        let paths = discover_with(&mut filter);

        let stamps: Vec<Option<(u64, u64)>> = paths.par_iter().map(|p| stamp(p)).collect();
        let mut files: Vec<Option<CachedFile>> = Vec::with_capacity(paths.len());
        let mut to_parse: Vec<(usize, PathBuf)> = Vec::new();
        for (i, (path, stamp)) in paths.into_iter().zip(stamps).enumerate() {
            match cached.remove(&path) {
                Some(hit) if stamp == Some((hit.size, hit.mtime_ns)) => files.push(Some(hit)),
                _ => {
                    files.push(None);
                    to_parse.push((i, path));
                }
            }
        }
        let mut stats = IndexStats {
            files: files.len(),
            reused: files.len() - to_parse.len(),
            reparsed: to_parse.len(),
            ..IndexStats::default()
        };
        let parse_paths: Vec<PathBuf> = to_parse.iter().map(|(_, p)| p.clone()).collect();
        for ((i, _), file) in to_parse.into_iter().zip(read_files(&parse_paths)) {
            files[i] = Some(file);
        }

        let mut index = Index {
            filter,
            files: Vec::new(),
            ids: HashMap::new(),
            classes: HashMap::new(),
            functions: HashMap::new(),
            constants: HashMap::new(),
        };
        for file in files.into_iter().flatten() {
            stats.files_with_parse_errors += usize::from(file.has_errors);
            stats.declarations += file.declarations.len();
            index.insert(file);
        }
        stats.elapsed_ms = started.elapsed().as_millis();
        Ok((index, stats))
    }

    /// Writes the index to `cache` for a later `build_with_cache`.
    pub fn save_cache(&self, cache: &Path) -> anyhow::Result<()> {
        let files = self.files.iter().flatten().cloned().collect();
        cache::save(cache, &self.filter, cache::ENGINE, files)
    }

    /// Applies file system changes. `changed` holds created or modified files or
    /// directories; a directory is walked like a discovery. `removed` holds deleted
    /// files or directories. Changed files that no longer exist or that the filter
    /// rejects are treated as removed.
    pub fn update(&mut self, changed: &[PathBuf], removed: &[PathBuf]) -> UpdateStats {
        let started = Instant::now();
        let mut stats = UpdateStats::default();

        let deleted: Vec<PathBuf> = removed.iter().map(|p| self.filter.under_root(p)).collect();
        let mut rejected: Vec<PathBuf> = Vec::new();
        let mut fresh: Vec<PathBuf> = Vec::new();
        for path in changed {
            let path = self.filter.under_root(path);
            let chosen = discover::chosen_path(self.filter.root(), &path).map(|(_, chosen)| chosen);
            match chosen {
                Some(chosen) if chosen.is_dir() => {
                    if self.filter.accepts_dir(&chosen) {
                        fresh.extend(discover_in(&mut self.filter, &chosen));
                    }
                }
                Some(chosen) if self.filter.accepts(&chosen) => fresh.push(chosen),
                _ => rejected.push(path),
            }
        }

        // A path given twice must be read and inserted once.
        fresh.sort();
        fresh.dedup();
        stats.removed += self.remove_paths(&rejected, &deleted);
        for path in &fresh {
            if let Some(&id) = self.ids.get(path) {
                self.remove(id);
            }
        }
        stats.reparsed = fresh.len();
        for file in read_files(&fresh) {
            self.insert(file);
        }
        stats.elapsed_ms = started.elapsed().as_millis();
        stats
    }

    /// Hover text for the word at `offset` in `path`, whose current content is
    /// `source`: the inferred type of a variable, or the signatures and docblock
    /// summaries of what the word refers to.
    pub fn hover(&mut self, path: &Path, source: &[u8], offset: u32) -> Option<Hover> {
        hover::hover(self, path, source, offset)
    }

    /// Every class-like declaration named `fqn`, matched case-insensitively.
    pub fn classes_named(&self, fqn: &str) -> Vec<ClassRef<'_>> {
        lookup(&self.classes, &fqn.to_ascii_lowercase())
            .iter()
            .filter_map(|entry| {
                let file = self.files[entry.file as usize].as_ref()?;
                Some(ClassRef {
                    path: &file.path,
                    declaration: file.declarations.get(entry.decl as usize)?,
                })
            })
            .collect()
    }

    /// Every declaration of `name` of `kind` visible on `class`, following the
    /// member lookup order through traits, parents and interfaces.
    pub fn find_member(
        &self,
        class: &str,
        kind: MemberKind,
        name: &str,
    ) -> Vec<(ClassRef<'_>, &Member)> {
        members::find_member(self, class, kind, name)
    }

    /// The return types of the functions a call to `name` reaches, with PHP's global
    /// fallback for unqualified, non-imported names.
    pub(crate) fn function_returns(&self, name: &NameRef) -> Vec<TypeExpr> {
        let key = name.fqn.to_ascii_lowercase();
        let mut entries = lookup(&self.functions, &key);
        if entries.is_empty()
            && !name.qualified
            && !name.imported
            && let Some((_, global)) = key.rsplit_once('\\')
        {
            entries = lookup(&self.functions, global);
        }
        entries
            .iter()
            .filter_map(|entry| {
                let file = self.files[entry.file as usize].as_ref()?;
                file.declarations.get(entry.decl as usize)?.returns.clone()
            })
            .collect()
    }

    /// Targets of members of `classes`, each with the name expected at its range.
    fn member_targets(
        &self,
        classes: &[String],
        kind: MemberKind,
        name: &str,
    ) -> Vec<(Location, String)> {
        let mut found: Vec<(Location, String)> = Vec::new();
        for class in classes {
            for (class_ref, member) in self.find_member(class, kind, name) {
                let location = Location {
                    path: class_ref.path.to_path_buf(),
                    start: member.name_start,
                    end: member.name_end,
                };
                if !found.iter().any(|(l, _)| *l == location) {
                    found.push((location, member.name.to_ascii_lowercase()));
                }
            }
        }
        found.sort_by(|(a, _), (b, _)| (&a.path, a.start).cmp(&(&b.path, b.start)));
        found
    }

    /// Targets of every declaration of the classes in `classes`.
    fn class_targets(&self, classes: &[String]) -> Vec<(Location, String)> {
        let mut found: Vec<(Location, String)> = classes
            .iter()
            .flat_map(|class| self.classes_named(class))
            .map(|class_ref| {
                let declaration = class_ref.declaration;
                let location = Location {
                    path: class_ref.path.to_path_buf(),
                    start: declaration.name_start,
                    end: declaration.name_end,
                };
                (location, short_name(&declaration.fqn))
            })
            .collect();
        found.sort_by(|(a, _), (b, _)| (&a.path, a.start).cmp(&(&b.path, b.start)));
        found.dedup_by(|(a, _), (b, _)| a == b);
        found
    }

    /// Definitions of the name at `offset` in `path`, whose current content is
    /// `source`. Results come only from resolved names and are sorted by path and
    /// offset; an unknown or unresolvable name returns nothing.
    ///
    /// A target file that no longer holds the declared name at the indexed range
    /// changed since it was indexed: it is indexed again and the query runs once more.
    pub fn definition(&mut self, path: &Path, source: &[u8], offset: u32) -> Vec<Location> {
        let found = self.resolve(path, source, offset);
        let mut stale: Vec<PathBuf> = found
            .iter()
            .filter(|(location, name)| !holds_name(location, name))
            .map(|(location, _)| location.path.clone())
            .collect();
        stale.sort();
        stale.dedup();
        if stale.is_empty() {
            return found.into_iter().map(|(location, _)| location).collect();
        }
        self.update(&stale, &[]);
        self.resolve(path, source, offset)
            .into_iter()
            .map(|(location, _)| location)
            .collect()
    }

    /// Each target with the lowercased short name the index expects at its range.
    fn resolve(&self, path: &Path, source: &[u8], offset: u32) -> Vec<(Location, String)> {
        let summary = summarize(source);

        if let Some(declaration) = summary
            .declarations
            .iter()
            .find(|d| d.name_start <= offset && offset <= d.name_end)
        {
            let location = Location {
                path: path.to_path_buf(),
                start: declaration.name_start,
                end: declaration.name_end,
            };
            return vec![(location, short_name(&declaration.fqn))];
        }

        let covers = |start: u32, end: u32| start <= offset && offset <= end;
        let class_of = |sel: &ClassSel| -> Option<String> {
            match sel {
                ClassSel::Named(fqn) => Some(fqn.clone()),
                ClassSel::SelfKw | ClassSel::StaticKw => {
                    summary.enclosing_class(offset).map(|c| c.fqn.clone())
                }
                ClassSel::ParentKw => summary.enclosing_class(offset)?.parent.clone(),
            }
        };
        if let Some(keyword) = summary.keywords.iter().find(|k| covers(k.start, k.end)) {
            return class_of(&keyword.class)
                .map(|class| self.class_targets(&[class]))
                .unwrap_or_default();
        }
        if let Some(member) = summary.static_refs.iter().find(|r| covers(r.start, r.end)) {
            return class_of(&member.class)
                .map(|class| self.member_targets(&[class], member.kind, &member.name))
                .unwrap_or_default();
        }
        if let Some((classes, kind, name)) = infer::member_at(self, &summary, source, offset) {
            return self.member_targets(&classes, kind, &name);
        }

        let Some(name) = name_at(&summary.names, offset) else {
            return Vec::new();
        };
        let (key, entries): (String, &[Entry]) = match name.kind {
            NameKind::ClassLike => {
                let key = name.fqn.to_ascii_lowercase();
                let entries = lookup(&self.classes, &key);
                (key, entries)
            }
            NameKind::Function => {
                let key = name.fqn.to_ascii_lowercase();
                let found = lookup(&self.functions, &key);
                match key.rsplit_once('\\') {
                    // PHP falls back to the global function only for unqualified,
                    // non-imported names.
                    Some((_, global)) if found.is_empty() && !name.qualified && !name.imported => {
                        let global = global.to_string();
                        let entries = lookup(&self.functions, &global);
                        (global, entries)
                    }
                    _ => (key, found),
                }
            }
            NameKind::Constant => {
                let key = symbol_key(SymbolKind::Constant, &name.fqn);
                let found = lookup(&self.constants, &key);
                match name.fqn.rsplit_once('\\') {
                    // Unqualified constants fall back to the global one, like functions.
                    Some((_, global)) if found.is_empty() && !name.qualified && !name.imported => {
                        let global = global.to_string();
                        let entries = lookup(&self.constants, &global);
                        (global, entries)
                    }
                    _ => (key, found),
                }
            }
        };

        let expected = short_name(&key);
        let mut found: Vec<(Location, String)> = entries
            .iter()
            .map(|entry| {
                let file = self.files[entry.file as usize]
                    .as_ref()
                    .expect("symbol tables only reference live files");
                let location = Location {
                    path: file.path.clone(),
                    start: entry.start,
                    end: entry.end,
                };
                (location, expected.clone())
            })
            .collect();
        found.sort_by(|(a, _), (b, _)| (&a.path, a.start).cmp(&(&b.path, b.start)));
        found
    }

    /// Removes the indexed files in `files`, and those equal to or inside any of
    /// `deleted`, which may name directories. Returns how many were removed.
    ///
    /// Deleted paths that are not indexed files are checked in one pass over the
    /// index, looking each file's ancestors up in a set, so a batch costs the same
    /// however many unrelated paths it holds.
    fn remove_paths(&mut self, files: &[PathBuf], deleted: &[PathBuf]) -> usize {
        let mut ids: Vec<u32> = files
            .iter()
            .filter_map(|p| self.ids.get(p))
            .copied()
            .collect();
        let mut directories: HashSet<&Path> = HashSet::new();
        for path in deleted {
            match self.ids.get(path) {
                Some(&id) => ids.push(id),
                None => {
                    directories.insert(path);
                }
            }
        }
        if !directories.is_empty() {
            ids.extend(
                self.ids
                    .iter()
                    .filter(|(path, _)| {
                        path.ancestors()
                            .skip(1)
                            .any(|ancestor| directories.contains(ancestor))
                    })
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

    fn insert(&mut self, file: CachedFile) {
        let id = self.files.len() as u32;
        for (decl, declaration) in file.declarations.iter().enumerate() {
            self.table_mut(declaration.kind)
                .entry(symbol_key(declaration.kind, &declaration.fqn))
                .or_default()
                .push(Entry {
                    file: id,
                    decl: decl as u32,
                    start: declaration.name_start,
                    end: declaration.name_end,
                });
        }
        self.ids.insert(file.path.clone(), id);
        self.files.push(Some(file));
    }

    fn remove(&mut self, id: u32) {
        let Some(file) = self.files[id as usize].take() else {
            return;
        };
        self.ids.remove(&file.path);
        for declaration in &file.declarations {
            let key = symbol_key(declaration.kind, &declaration.fqn);
            let table = self.table_mut(declaration.kind);
            if let Some(entries) = table.get_mut(&key) {
                entries.retain(|e| e.file != id);
                if entries.is_empty() {
                    table.remove(&key);
                }
            }
        }
    }

    fn table_mut(&mut self, kind: SymbolKind) -> &mut HashMap<String, Vec<Entry>> {
        match kind {
            SymbolKind::Function => &mut self.functions,
            SymbolKind::Constant => &mut self.constants,
            _ => &mut self.classes,
        }
    }
}

fn lookup<'a>(table: &'a HashMap<String, Vec<Entry>>, key: &str) -> &'a [Entry] {
    table.get(key).map(Vec::as_slice).unwrap_or_default()
}

/// The table key of a symbol: lowercased for classes and functions, which PHP
/// matches case-insensitively; for constants only the namespace part is lowercased.
fn symbol_key(kind: SymbolKind, fqn: &str) -> String {
    match (kind, fqn.rsplit_once('\\')) {
        (SymbolKind::Constant, Some((namespace, name))) => {
            format!("{}\\{name}", namespace.to_ascii_lowercase())
        }
        (SymbolKind::Constant, None) => fqn.to_string(),
        _ => fqn.to_ascii_lowercase(),
    }
}

fn short_name(fqn: &str) -> String {
    fqn.rsplit('\\').next().unwrap_or(fqn).to_ascii_lowercase()
}

/// Whether the file at `location` still holds `name` (lowercased) at its range.
fn holds_name(location: &Location, name: &str) -> bool {
    std::fs::read(&location.path).is_ok_and(|text| {
        text.get(location.start as usize..location.end as usize)
            .is_some_and(|found| found.eq_ignore_ascii_case(name.as_bytes()))
    })
}

/// The name covering `offset`. A name that strictly contains the offset wins over
/// one that ends exactly at it.
fn name_at(names: &[NameRef], offset: u32) -> Option<&NameRef> {
    names
        .iter()
        .find(|n| n.start <= offset && offset < n.end)
        .or_else(|| names.iter().find(|n| n.end == offset))
}

/// A declaration without the text only hover shows: signatures and docblocks are
/// read again from the file when needed, which keeps the index and its cache small.
fn lean(mut declaration: Declaration) -> Declaration {
    declaration.signature = String::new();
    declaration.doc = None;
    if let Some(class) = &mut declaration.class {
        for member in &mut class.members {
            member.signature = String::new();
            member.doc = None;
        }
    }
    declaration
}

/// Size and modification time in nanoseconds.
fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?;
    Some((meta.len(), mtime.as_nanos() as u64))
}

/// Reads and summarizes files in parallel. The stamp is taken before reading, so a
/// file written meanwhile looks changed to the next cache check. Unreadable files
/// yield no declarations rather than failing.
fn read_files(paths: &[PathBuf]) -> Vec<CachedFile> {
    paths
        .par_iter()
        .map(|path| {
            let (size, mtime_ns) = stamp(path).unwrap_or_default();
            let (declarations, has_errors) = match std::fs::read(path) {
                Ok(source) => {
                    let summary = summarize(&source);
                    let declarations = summary.declarations.into_iter().map(lean).collect();
                    (declarations, summary.has_parse_errors)
                }
                Err(_) => (Vec::<Declaration>::new(), false),
            };
            CachedFile {
                path: path.clone(),
                size,
                mtime_ns,
                has_errors,
                declarations,
            }
        })
        .collect()
}
