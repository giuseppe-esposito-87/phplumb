//! Runs the marker-annotated fixture workspaces under `tests/fixtures`.
//!
//! `/*@cursor:ID*/` places a cursor at the byte right after the marker.
//! `/*@target:ID*/` marks a target at the first byte after the marker that is neither
//! whitespace nor another marker; on `$name` or a quoted name it marks the name itself. A cursor passes only when the set of results equals
//! the set of targets with the same ID; a cursor with no target expects nothing.
//! Several cursors may share an ID.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use phpnav_index::discover::{DiscoverOptions, discover};
use phpnav_index::index::Index;

const OPEN: &[u8] = b"/*@";
const CLOSE: &[u8] = b"*/";

struct Marker {
    kind: String,
    id: String,
    end: usize,
}

fn markers(text: &[u8]) -> Vec<Marker> {
    let mut found = Vec::new();
    let mut pos = 0;
    while let Some(rel) = find(&text[pos..], OPEN) {
        let start = pos + rel + OPEN.len();
        let close = start + find(&text[start..], CLOSE).expect("unterminated marker");
        let body = std::str::from_utf8(&text[start..close]).unwrap();
        let (kind, id) = body.split_once(':').expect("marker without id");
        found.push(Marker {
            kind: kind.to_string(),
            id: id.to_string(),
            end: close + CLOSE.len(),
        });
        pos = close + CLOSE.len();
    }
    found
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// First byte at or after `pos` that is neither whitespace nor inside a marker.
fn skip_to_code(text: &[u8], mut pos: usize) -> usize {
    loop {
        while pos < text.len() && text[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if text[pos..].starts_with(OPEN) {
            pos += find(&text[pos..], CLOSE).unwrap() + CLOSE.len();
        } else if matches!(text.get(pos), Some(b'$' | b'\'' | b'"')) {
            return pos + 1;
        } else {
            return pos;
        }
    }
}

fn line_of(text: &[u8], offset: usize) -> usize {
    text[..offset].iter().filter(|b| **b == b'\n').count() + 1
}

fn run_fixture(name: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
        .canonicalize()
        .unwrap();
    let (mut index, _) = Index::build(&root, &DiscoverOptions::default()).unwrap();
    let rel = |path: &Path| path.strip_prefix(&root).unwrap().display().to_string();

    let mut cursors: Vec<(String, PathBuf, usize)> = Vec::new();
    let mut targets: HashMap<String, BTreeSet<(String, u32)>> = HashMap::new();
    for path in discover(&root, &DiscoverOptions::default()).unwrap() {
        let text = fs::read(&path).unwrap();
        for marker in markers(&text) {
            match marker.kind.as_str() {
                "cursor" => cursors.push((marker.id, path.clone(), marker.end)),
                "target" => {
                    let at = skip_to_code(&text, marker.end) as u32;
                    targets
                        .entry(marker.id)
                        .or_default()
                        .insert((rel(&path), at));
                }
                other => panic!("unknown marker kind {other}"),
            }
        }
    }
    assert!(!cursors.is_empty(), "fixture {name} has no cursors");

    let mut failures = Vec::new();
    for (id, path, offset) in &cursors {
        let source = fs::read(path).unwrap();
        let actual: BTreeSet<(String, u32)> = index
            .definition(path, &source, *offset as u32)
            .into_iter()
            .map(|loc| (rel(&loc.path), loc.start))
            .collect();
        let expected = targets.get(id).cloned().unwrap_or_default();
        if actual != expected {
            failures.push(format!(
                "{}:{} cursor {id}: expected {expected:?}, got {actual:?}",
                rel(path),
                line_of(&source, *offset)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn names() {
    run_fixture("names");
}

#[test]
fn functions() {
    run_fixture("functions");
}

#[test]
fn kinds() {
    run_fixture("kinds");
}

#[test]
fn ambiguous() {
    run_fixture("ambiguous");
}

#[test]
fn tolerant() {
    run_fixture("tolerant");
}

#[test]
fn static_members() {
    run_fixture("static_members");
}
