//! Fingerprints the indexing code, so an index cache written by one engine build is
//! never reused by another: a cached summary is only valid for the code that made it.

use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let lock = manifest.join("../../Cargo.lock");
    let mut inputs = vec![manifest.join("Cargo.toml"), lock];
    collect(&manifest.join("src"), &mut inputs);
    inputs.sort();

    // The directory itself, so a new source file also triggers a rerun.
    println!("cargo:rerun-if-changed={}", manifest.join("src").display());
    let mut hasher = DefaultHasher::new();
    for input in &inputs {
        println!("cargo:rerun-if-changed={}", input.display());
        hasher.write(input.to_string_lossy().as_bytes());
        hasher.write(&std::fs::read(input).unwrap_or_default());
    }
    println!(
        "cargo:rustc-env=PHPNAV_INDEX_BUILD={:016x}",
        hasher.finish()
    );
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}
