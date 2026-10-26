//! Workspace-wide symbol table and the definition query.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::discover::{DiscoverOptions, discover};
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

#[derive(Clone, Copy, Debug)]
struct Entry {
    file: u32,
    start: u32,
    end: u32,
}

/// Declarations of every PHP file under a root. Keys are lowercased fully qualified
/// names without a leading backslash, because PHP class and function names are
/// case-insensitive.
pub struct Index {
    paths: Vec<PathBuf>,
    classes: HashMap<String, Vec<Entry>>,
    functions: HashMap<String, Vec<Entry>>,
}

impl Index {
    pub fn build(root: &Path, opts: &DiscoverOptions) -> anyhow::Result<(Index, IndexStats)> {
        let started = Instant::now();
        let paths = discover(root, opts)?;

        // Unreadable files yield no declarations rather than failing the build.
        let summaries: Vec<(Vec<Declaration>, bool)> = paths
            .par_iter()
            .map(|path| match std::fs::read(path) {
                Ok(source) => {
                    let summary = summarize(&source);
                    (summary.declarations, summary.has_parse_errors)
                }
                Err(_) => (Vec::new(), false),
            })
            .collect();

        let mut index = Index {
            paths,
            classes: HashMap::new(),
            functions: HashMap::new(),
        };
        let mut stats = IndexStats {
            files: index.paths.len(),
            ..IndexStats::default()
        };
        for (file, (declarations, has_errors)) in summaries.into_iter().enumerate() {
            stats.files_with_parse_errors += usize::from(has_errors);
            stats.declarations += declarations.len();
            for declaration in declarations {
                let table = match declaration.kind {
                    SymbolKind::Function => &mut index.functions,
                    _ => &mut index.classes,
                };
                table
                    .entry(declaration.fqn.to_ascii_lowercase())
                    .or_default()
                    .push(Entry {
                        file: file as u32,
                        start: declaration.name_start,
                        end: declaration.name_end,
                    });
            }
        }
        stats.elapsed_ms = started.elapsed().as_millis();
        Ok((index, stats))
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
                path: self.paths[entry.file as usize].clone(),
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
