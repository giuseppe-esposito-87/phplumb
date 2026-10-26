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
