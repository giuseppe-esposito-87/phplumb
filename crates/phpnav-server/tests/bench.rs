use std::path::Path;
use std::process::Command;

use phpnav_index::discover::{DiscoverOptions, discover};
use serde_json::Value;

/// 1-based `line:column` of the first occurrence of `needle` in `text`, with the
/// column in UTF-16 units.
fn position_of(text: &str, needle: &str) -> String {
    let offset = text.find(needle).expect("needle in fixture");
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let col = before[line_start..].encode_utf16().count() + 1;
    format!("{line}:{col}")
}

#[test]
fn bench_reports_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/names")
        .canonicalize()
        .unwrap();
    let consumer = std::fs::read_to_string(root.join("app/Consumer.php")).unwrap();
    let same = std::fs::read_to_string(root.join("app/Same.php")).unwrap();
    let at = format!("app/Consumer.php:{}", position_of(&consumer, "Same();"));
    let target = format!("app/Same.php:{}", position_of(&same, "Same {}"));

    let cases_dir = tempfile::TempDir::new().unwrap();
    let cases = cases_dir.path().join("cases.toml");
    std::fs::write(
        &cases,
        format!(
            "[[case]]\nname = \"same namespace\"\nkind = \"definition\"\nat = \"{at}\"\nexpect = [\"{target}\"]\n\n\
             [[case]]\nname = \"wrong on purpose\"\nkind = \"definition\"\nat = \"{at}\"\nexpect = [\"lib/Thing.php:1:1\"]\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .arg("--cases")
        .arg(&cases)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let php_files = discover(&root, &DiscoverOptions::default()).unwrap().len();
    assert_eq!(report["files"], php_files);
    assert_eq!(report["cases_passed"], 1);
    assert_eq!(report["cases_failed"], 1);
    assert_eq!(report["failures"][0]["name"], "wrong on purpose");
    assert_eq!(report["failures"][0]["actual"][0], target);
}

#[test]
fn bench_reports_warm_start_and_updates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/names")
        .canonicalize()
        .unwrap();
    let cache_dir = tempfile::TempDir::new().unwrap();
    let cache = cache_dir.path().join("index-v1.bin");

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .arg("--cache")
        .arg(&cache)
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    for field in ["warm_start_ms", "update_one_ms", "update_1000_ms"] {
        assert!(report[field].is_number(), "{field} missing in {report}");
    }
    assert!(cache.exists());
}

#[test]
fn bench_reports_member_coverage() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/inference")
        .canonicalize()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .arg("--coverage")
        .arg("app")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let accesses = report["coverage"]["member_accesses"].as_u64().unwrap();
    let resolved = report["coverage"]["resolved"].as_u64().unwrap();
    assert!(accesses >= 30, "{report}");
    assert!(resolved > 0 && resolved < accesses, "{report}");
}

#[test]
fn bench_runs_implementation_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/implementations")
        .canonicalize()
        .unwrap();
    let shapes = std::fs::read_to_string(root.join("lib/Shapes.php")).unwrap();
    let at = format!("lib/Shapes.php:{}", position_of(&shapes, "Lonely"));
    let cases_dir = tempfile::TempDir::new().unwrap();
    let cases = cases_dir.path().join("cases.toml");
    std::fs::write(
        &cases,
        format!(
            "[[case]]\nname = \"no subclasses\"\nkind = \"implementation\"\nat = \"{at}\"\nexpect = []\n\n\
             [[case]]\nname = \"wrong on purpose\"\nkind = \"implementation\"\nat = \"{at}\"\nexpect = [\"lib/Shapes.php:1:1\"]\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .arg("--cases")
        .arg(&cases)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cases_passed"], 1, "{report}");
    assert!(report["implementation_ms"].is_number(), "{report}");
}

#[test]
fn bench_runs_reference_cases() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/references")
        .canonicalize()
        .unwrap();
    let global = std::fs::read_to_string(root.join("global.php")).unwrap();
    let uses = std::fs::read_to_string(root.join("app/Use.php")).unwrap();
    let at = format!("global.php:{}", position_of(&global, "ghelper()"));
    let call = format!("app/Use.php:{}", position_of(&uses, "ghelper();"));
    let cases_dir = tempfile::TempDir::new().unwrap();
    let cases = cases_dir.path().join("cases.toml");
    std::fs::write(
        &cases,
        format!(
            "[[case]]\nname = \"global function\"\nkind = \"references\"\nat = \"{at}\"\nexpect = [\"{call}\"]\n\n\
             [[case]]\nname = \"wrong on purpose\"\nkind = \"references\"\nat = \"{at}\"\nexpect = []\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .arg("--cases")
        .arg(&cases)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cases_passed"], 1, "{report}");
    assert!(report["references_ms"].is_number(), "{report}");
}

#[test]
fn bench_reports_symbol_search_time() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../phpnav-index/tests/fixtures/names")
        .canonicalize()
        .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_php-navigator"))
        .arg("bench")
        .arg("--root")
        .arg(&root)
        .output()
        .unwrap();

    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["symbols_ms"].is_number(), "{report}");
}
