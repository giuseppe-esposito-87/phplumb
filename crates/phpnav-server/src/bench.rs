//! `php-navigator bench`: indexes a project, measures it and checks a case file.
//!
//! Case positions are `relative/path:line:column`, 1-based, with columns in UTF-16
//! units, pointing at the start of a name. An empty `expect` means "nothing".

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use phpnav_index::discover::{DiscoverOptions, discover};
use phpnav_index::line_index::LineIndex;
use phpnav_index::{Index, member_access_offsets};
use serde::{Deserialize, Serialize};

pub struct BenchArgs {
    pub root: PathBuf,
    pub cases: Option<PathBuf>,
    pub cache: Option<PathBuf>,
    /// A directory relative to the root whose member accesses are measured.
    pub coverage: Option<PathBuf>,
    pub exclude: Vec<String>,
}

impl BenchArgs {
    pub fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut root = None;
        let mut cases = None;
        let mut cache = None;
        let mut coverage = None;
        let mut exclude = Vec::new();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().with_context(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "--root" => root = Some(PathBuf::from(value()?)),
                "--cases" => cases = Some(PathBuf::from(value()?)),
                "--cache" => cache = Some(PathBuf::from(value()?)),
                "--coverage" => coverage = Some(PathBuf::from(value()?)),
                "--exclude" => exclude.push(value()?.clone()),
                other => bail!("unknown bench option {other}"),
            }
        }
        Ok(BenchArgs {
            root: root.context("bench needs --root <DIR>")?,
            cases,
            cache,
            coverage,
            exclude,
        })
    }
}

#[derive(Deserialize)]
struct CaseFile {
    #[serde(default, rename = "case")]
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    kind: String,
    at: String,
    #[serde(default)]
    expect: Vec<String>,
}

#[derive(Serialize)]
pub struct Report {
    files: usize,
    declarations: usize,
    files_with_parse_errors: usize,
    cold_index_ms: u128,
    /// With `--cache`: a build from the cache written after the cold build, then
    /// updates of the first file and of the first 1,000 files.
    #[serde(skip_serializing_if = "Option::is_none")]
    warm_start_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_one_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    update_1000_ms: Option<u128>,
    #[serde(skip_serializing_if = "Option::is_none")]
    coverage: Option<Coverage>,
    peak_rss_mb: u64,
    cases_passed: usize,
    cases_failed: usize,
    failures: Vec<Failure>,
}

/// How many member accesses under a directory reach at least one definition.
#[derive(Serialize)]
struct Coverage {
    member_accesses: usize,
    resolved: usize,
    percent: f64,
}

#[derive(Serialize)]
struct Failure {
    name: String,
    expected: Vec<String>,
    actual: Vec<String>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.cases_failed == 0
    }
}

pub fn run(args: &BenchArgs) -> anyhow::Result<Report> {
    let root = args.root.canonicalize()?;
    let opts = DiscoverOptions {
        exclude: args.exclude.clone(),
        ..DiscoverOptions::default()
    };
    let (mut index, stats) = Index::build(&root, &opts)?;

    let mut report = Report {
        files: stats.files,
        declarations: stats.declarations,
        files_with_parse_errors: stats.files_with_parse_errors,
        cold_index_ms: stats.elapsed_ms,
        warm_start_ms: None,
        update_one_ms: None,
        update_1000_ms: None,
        coverage: None,
        peak_rss_mb: 0,
        cases_passed: 0,
        cases_failed: 0,
        failures: Vec::new(),
    };

    if let Some(cache) = &args.cache {
        index.save_cache(cache)?;
        drop(index);
        let (warm, warm_stats) = Index::build_with_cache(&root, &opts, cache)?;
        index = warm;
        report.warm_start_ms = Some(warm_stats.elapsed_ms);
        let paths = discover(&root, &opts)?;
        report.update_one_ms = Some(index.update(&paths[..paths.len().min(1)], &[]).elapsed_ms);
        report.update_1000_ms = Some(
            index
                .update(&paths[..paths.len().min(1000)], &[])
                .elapsed_ms,
        );
    }
    if let Some(dir) = &args.coverage {
        report.coverage = Some(coverage(&mut index, &root, &root.join(dir), &opts)?);
    }
    report.peak_rss_mb = peak_rss_bytes() / (1024 * 1024);

    let Some(cases_path) = &args.cases else {
        return Ok(report);
    };
    let text = std::fs::read_to_string(cases_path)
        .with_context(|| format!("reading {}", cases_path.display()))?;
    let file: CaseFile = toml::from_str(&text)?;
    for case in file.cases {
        let expected: BTreeSet<String> = case.expect.iter().cloned().collect();
        let actual = match case.kind.as_str() {
            "definition" => definition_case(&mut index, &root, &case.at),
            other => Err(anyhow::anyhow!("unsupported case kind {other}")),
        };
        match actual {
            Ok(actual) if actual == expected => report.cases_passed += 1,
            Ok(actual) => {
                report.cases_failed += 1;
                report.failures.push(Failure {
                    name: case.name,
                    expected: expected.into_iter().collect(),
                    actual: actual.into_iter().collect(),
                });
            }
            Err(err) => {
                report.cases_failed += 1;
                report.failures.push(Failure {
                    name: case.name,
                    expected: expected.into_iter().collect(),
                    actual: vec![format!("error: {err:#}")],
                });
            }
        }
    }
    Ok(report)
}

fn coverage(
    index: &mut Index,
    root: &Path,
    dir: &Path,
    opts: &DiscoverOptions,
) -> anyhow::Result<Coverage> {
    let dir = dir.canonicalize()?;
    let mut member_accesses = 0;
    let mut resolved = 0;
    for path in discover(root, opts)?
        .into_iter()
        .filter(|p| p.starts_with(&dir))
    {
        let source = std::fs::read(&path)?;
        for offset in member_access_offsets(&source) {
            member_accesses += 1;
            if !index.definition(&path, &source, offset).is_empty() {
                resolved += 1;
            }
        }
    }
    let percent = if member_accesses == 0 {
        0.0
    } else {
        (resolved as f64 * 1000.0 / member_accesses as f64).round() / 10.0
    };
    Ok(Coverage {
        member_accesses,
        resolved,
        percent,
    })
}

fn definition_case(index: &mut Index, root: &Path, at: &str) -> anyhow::Result<BTreeSet<String>> {
    let (path, line, col) = parse_position(root, at)?;
    let source = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let offset = LineIndex::new(&source)
        .offset(&source, line - 1, col - 1)
        .with_context(|| format!("{at} is outside the file"))?;
    index
        .definition(&path, &source, offset)
        .into_iter()
        .map(|location| {
            let text = std::fs::read(&location.path)?;
            let (line, col) = LineIndex::new(&text).position(&text, location.start);
            let relative = location.path.strip_prefix(root).unwrap_or(&location.path);
            Ok(format!("{}:{}:{}", relative.display(), line + 1, col + 1))
        })
        .collect()
}

fn parse_position(root: &Path, at: &str) -> anyhow::Result<(PathBuf, u32, u32)> {
    let mut parts = at.rsplitn(3, ':');
    let col = parts.next().context("missing column")?.parse::<u32>()?;
    let line = parts.next().context("missing line")?.parse::<u32>()?;
    let path = parts.next().context("missing path")?;
    if line == 0 || col == 0 {
        bail!("{at}: lines and columns are 1-based");
    }
    Ok((root.join(path), line, col))
}

/// Peak resident memory of this process.
fn peak_rss_bytes() -> u64 {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: getrusage fills the struct it is given and only reads RUSAGE_SELF.
    let usage = unsafe {
        if libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) != 0 {
            return 0;
        }
        usage.assume_init()
    };
    let max_rss = usage.ru_maxrss as u64;
    // ru_maxrss is in bytes on macOS and in KiB on Linux.
    if cfg!(target_os = "macos") {
        max_rss
    } else {
        max_rss * 1024
    }
}
