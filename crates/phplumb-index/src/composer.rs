//! Which files under a Composer `vendor` directory Composer can load.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// The autoload paths of the packages listed in `<vendor>/composer/installed.json`.
/// Package tests, fixtures and leftovers that no autoloader reaches are left out,
/// because they declare symbols PHP never loads (test copies of core, global
/// functions named like PHP built-ins).
pub(crate) struct VendorFilter {
    composer_dir: PathBuf,
    allowed: HashSet<PathBuf>,
}

impl VendorFilter {
    /// None when the vendor directory has no readable `composer/installed.json`.
    pub(crate) fn load(vendor: &Path) -> Option<Self> {
        let composer_dir = vendor.join("composer");
        let text = std::fs::read(composer_dir.join("installed.json")).ok()?;
        let json: Value = serde_json::from_slice(&text).ok()?;
        let packages = match &json {
            Value::Object(map) => map.get("packages")?.as_array()?,
            Value::Array(list) => list,
            _ => return None,
        };

        let mut allowed = HashSet::new();
        for package in packages {
            let Some(install_path) = package.get("install-path").and_then(Value::as_str) else {
                continue;
            };
            let base = normalize(&composer_dir.join(install_path));
            let Some(autoload) = package.get("autoload") else {
                continue;
            };
            for key in ["psr-4", "psr-0"] {
                if let Some(Value::Object(map)) = autoload.get(key) {
                    for dirs in map.values() {
                        for dir in strings(dirs) {
                            allowed.insert(normalize(&base.join(dir)));
                        }
                    }
                }
            }
            for key in ["classmap", "files"] {
                if let Some(entries) = autoload.get(key) {
                    for entry in strings(entries) {
                        allowed.insert(normalize(&base.join(entry)));
                    }
                }
            }
        }
        Some(Self {
            composer_dir,
            allowed,
        })
    }

    /// Whether `path`, a file inside this vendor directory, is loadable. Composer's
    /// own runtime files directly in `vendor/composer` always are.
    pub(crate) fn allows(&self, path: &Path) -> bool {
        if path.parent() == Some(self.composer_dir.as_path()) {
            return true;
        }
        path.ancestors()
            .any(|ancestor| self.allowed.contains(ancestor))
    }
}

fn strings(value: &Value) -> Vec<&str> {
    match value {
        Value::String(s) => vec![s.as_str()],
        Value::Array(items) => items.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    }
}

/// Resolves `.` and `..` without touching the file system, so symlinked packages
/// keep the path they are reached through.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}
