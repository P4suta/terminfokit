// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Offline output comparison with pinned ncurses `tic`.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

/// Pinned ncurses release used by the full differential suite.
pub const NCURSES_VERSION: &str = "6.6";
/// SHA-256 of the pinned ncurses 6.6 source archive.
pub const ARCHIVE_SHA256: &str = "355b4cbbed880b0381a04c46617b7656e362585d52e9cf84a67e2009b749ff11";
/// SHA-256 of the unmodified ncurses 6.6 misc/terminfo.src fixture.
pub const SOURCE_SHA256: &str = "75673b421c25032306f7cdf26df57978c86ed9cf3d3fb16a6479233775f4f961";
/// Logical entry count required from the pinned source.
pub const SOURCE_ENTRY_COUNT: usize = 1_861;

#[derive(Debug, Clone)]
pub struct OracleConfig {
    pub tic: PathBuf,
    pub source: PathBuf,
    pub output: PathBuf,
    pub extended: bool,
}

impl OracleConfig {
    pub fn run(&self) -> io::Result<Output> {
        fs::create_dir_all(&self.output)?;
        let mut command = Command::new(&self.tic);
        if self.extended {
            command.arg("-x");
        }
        command
            .arg("-o")
            .arg(&self.output)
            .arg(&self.source)
            .output()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DifferenceKind {
    MissingActual,
    MissingOracle,
    Bytes,
    AllowlistMismatch,
    StaleAllowlist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    pub relative_path: PathBuf,
    pub kind: DifferenceKind,
    pub hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowlistedDifference {
    pub relative_path: PathBuf,
    pub ncurses_version: String,
    pub reason: String,
    pub fixture: String,
    pub kind: DifferenceKind,
    pub expected_hash: String,
    /// Inclusive ISO-8601 calendar date (YYYY-MM-DD).
    pub expires: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonContext {
    pub ncurses_version: String,
    pub fixture: String,
    /// ISO-8601 calendar date (YYYY-MM-DD), supplied by the release job.
    pub today: String,
}

pub fn compare_trees(
    oracle: &Path,
    actual: &Path,
    allowlist: &[AllowlistedDifference],
    context: &ComparisonContext,
) -> io::Result<Vec<Difference>> {
    let oracle_files = files(oracle)?;
    let actual_files = files(actual)?;
    let paths: BTreeSet<_> = oracle_files
        .keys()
        .chain(actual_files.keys())
        .cloned()
        .collect();
    let mut differences = Vec::new();
    let mut used = BTreeSet::new();
    let eligible: BTreeSet<_> = allowlist
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            item.ncurses_version == context.ncurses_version && item.fixture == context.fixture
        })
        .map(|(index, _)| index)
        .collect();
    for path in paths {
        let oracle_value = oracle_files.get(&path);
        let actual_value = actual_files.get(&path);
        let kind = match (oracle_value, actual_value) {
            (Some(_), None) => Some(DifferenceKind::MissingActual),
            (None, Some(_)) => Some(DifferenceKind::MissingOracle),
            (Some(left), Some(right)) if left != right => Some(DifferenceKind::Bytes),
            _ => None,
        };
        if let Some(kind) = kind {
            let hash = difference_hash(&path, &kind, oracle_value, actual_value);
            let matching: Vec<_> = allowlist
                .iter()
                .enumerate()
                .filter(|(index, item)| eligible.contains(index) && item.relative_path == path)
                .collect();
            if matching.len() == 1 {
                let (index, item) = matching[0];
                used.insert(index);
                if allowlist_matches(item, &kind, &hash, context) {
                    continue;
                }
                differences.push(Difference {
                    relative_path: path,
                    kind: DifferenceKind::AllowlistMismatch,
                    hash,
                });
            } else if matching.is_empty() {
                differences.push(Difference {
                    relative_path: path,
                    kind,
                    hash,
                });
            } else {
                for (index, _) in matching {
                    used.insert(index);
                }
                differences.push(Difference {
                    relative_path: path,
                    kind: DifferenceKind::AllowlistMismatch,
                    hash,
                });
            }
        }
    }
    for (index, item) in allowlist.iter().enumerate() {
        if eligible.contains(&index) && !used.contains(&index) {
            differences.push(Difference {
                relative_path: item.relative_path.clone(),
                kind: DifferenceKind::StaleAllowlist,
                hash: item.expected_hash.clone(),
            });
        }
    }
    Ok(differences)
}

fn allowlist_matches(
    item: &AllowlistedDifference,
    kind: &DifferenceKind,
    hash: &str,
    context: &ComparisonContext,
) -> bool {
    !item.reason.trim().is_empty()
        && !item.expected_hash.is_empty()
        && item.ncurses_version == context.ncurses_version
        && item.fixture == context.fixture
        && &item.kind == kind
        && item.expected_hash.eq_ignore_ascii_case(hash)
        && valid_date(&item.expires)
        && valid_date(&context.today)
        && item.expires.as_str() >= context.today.as_str()
}

fn valid_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
        && value[5..7]
            .parse::<u8>()
            .is_ok_and(|month| (1..=12).contains(&month))
        && value[8..10]
            .parse::<u8>()
            .is_ok_and(|day| (1..=31).contains(&day))
}

fn difference_hash(
    path: &Path,
    kind: &DifferenceKind,
    oracle: Option<&Vec<u8>>,
    actual: Option<&Vec<u8>>,
) -> String {
    let mut digest = Sha256::new();
    digest.update(path.to_string_lossy().as_bytes());
    digest.update([0]);
    digest.update(match kind {
        DifferenceKind::MissingActual => b"missing-actual".as_slice(),
        DifferenceKind::MissingOracle => b"missing-oracle".as_slice(),
        DifferenceKind::Bytes => b"bytes".as_slice(),
        DifferenceKind::AllowlistMismatch => b"allowlist-mismatch".as_slice(),
        DifferenceKind::StaleAllowlist => b"stale-allowlist".as_slice(),
    });
    for value in [oracle, actual] {
        digest.update([0]);
        match value {
            Some(bytes) => {
                digest.update((bytes.len() as u64).to_le_bytes());
                digest.update(bytes);
            }
            None => digest.update(u64::MAX.to_le_bytes()),
        }
    }
    lower_hex(&digest.finalize())
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn files(root: &Path) -> io::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let mut output = BTreeMap::new();
    collect(root, root, &mut output)?;
    Ok(output)
}

fn collect(
    root: &Path,
    directory: &Path,
    output: &mut BTreeMap<PathBuf, Vec<u8>>,
) -> io::Result<()> {
    for item in fs::read_dir(directory)? {
        let item = item?;
        let path = item.path();
        let kind = item.file_type()?;
        if kind.is_dir() {
            collect(root, &path, output)?;
        } else if kind.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(io::Error::other)?
                .to_owned();
            output.insert(relative, fs::read(path)?);
        }
    }
    Ok(())
}

/// Configuration for a full ncurses comparison.
#[derive(Debug, Clone)]
pub struct FullRunConfig {
    pub archive: PathBuf,
    pub work: PathBuf,
    pub today: String,
    pub allowlist: Vec<AllowlistedDifference>,
}

/// Result for one normal or extended fixture.
#[derive(Debug, Clone)]
pub struct FixtureReport {
    pub fixture: String,
    pub oracle_files: usize,
    pub actual_files: usize,
    pub differences: Vec<Difference>,
    pub roundtrip_mismatches: Vec<RoundtripMismatch>,
}

/// A file that did not decode and re-encode exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoundtripMismatch {
    /// Path relative to the compiled database root.
    pub relative_path: PathBuf,
    /// Decode, encode, or byte-comparison failure.
    pub reason: String,
}

/// Result of a full pinned conformance run.
#[derive(Debug, Clone)]
pub struct FullRunReport {
    pub logical_entries: usize,
    pub fixtures: Vec<FixtureReport>,
}

impl FullRunReport {
    /// Returns true only when every tree and oracle re-encode comparison is exact.
    pub fn is_exact(&self) -> bool {
        self.fixtures.iter().all(|fixture| {
            fixture.differences.is_empty() && fixture.roundtrip_mismatches.is_empty()
        })
    }
}

/// Compares both fixtures from a downloaded archive using tar, sh, and make.
pub fn run_full(config: &FullRunConfig) -> io::Result<FullRunReport> {
    verify_hash(&config.archive, ARCHIVE_SHA256, "ncurses archive")?;
    if config.work.exists() {
        fs::remove_dir_all(&config.work)?;
    }
    fs::create_dir_all(&config.work)?;
    run_command(
        Command::new("tar")
            .arg("-xzf")
            .arg(&config.archive)
            .arg("-C")
            .arg(&config.work),
        "extract ncurses archive",
    )?;
    let ncurses = config.work.join("ncurses-6.6");
    let source = ncurses.join("misc").join("terminfo.src");
    verify_hash(&source, SOURCE_SHA256, "terminfo.src")?;
    let document = terminfokit::source::parse(&fs::read(&source)?)
        .map_err(|error| io::Error::other(error.to_string()))?;
    if document.entries().len() != SOURCE_ENTRY_COUNT {
        return Err(io::Error::other(format!(
            "terminfo.src contains {} logical entries, expected {SOURCE_ENTRY_COUNT}",
            document.entries().len()
        )));
    }

    run_command(
        Command::new("sh")
            .arg("./configure")
            .args([
                "--without-shared",
                "--without-debug",
                "--without-ada",
                "--without-cxx",
                "--without-cxx-binding",
            ])
            .current_dir(&ncurses),
        "configure ncurses",
    )?;
    run_command(
        Command::new("make").arg("-j2").current_dir(&ncurses),
        "build ncurses tic",
    )?;
    let tic = ncurses.join("progs").join("tic");

    let source_bytes = fs::read(&source)?;
    let mut fixtures = Vec::new();
    for (fixture, extended) in [("normal", false), ("extended", true)] {
        let fixture_root = config.work.join(fixture);
        let oracle = fixture_root.join("oracle");
        let actual = fixture_root.join("actual");
        let output = OracleConfig {
            tic: tic.clone(),
            source: source.clone(),
            output: oracle.clone(),
            extended,
        }
        .run()?;
        if !output.status.success() {
            return Err(command_error("run ncurses tic", &output));
        }

        let compilation = terminfokit::Compiler::new()
            .options(terminfokit::CompilerOptions::new().with_extended(extended))
            .compile(&source_bytes)
            .map_err(|error| io::Error::other(error.to_string()))?;
        if compilation.entries().len() != SOURCE_ENTRY_COUNT {
            return Err(io::Error::other(format!(
                "{fixture}: compiled {} entries, expected {SOURCE_ENTRY_COUNT}",
                compilation.entries().len()
            )));
        }
        let database = terminfokit::database::DirectoryDatabase::new(&actual);
        for item in compilation.entries() {
            database
                .install(
                    item.entry(),
                    terminfokit::database::InstallOptions::new()
                        .with_layout(terminfokit::database::DirectoryLayout::Letter)
                        .with_encode_options(
                            terminfokit::EncodeOptions::new().with_extended(extended),
                        ),
                )
                .map_err(|error| io::Error::other(error.to_string()))?;
        }

        let context = ComparisonContext {
            ncurses_version: NCURSES_VERSION.into(),
            fixture: fixture.into(),
            today: config.today.clone(),
        };
        let differences = compare_trees(&oracle, &actual, &config.allowlist, &context)?;
        let roundtrip_mismatches = verify_reencode_tree(&oracle)?;
        fixtures.push(FixtureReport {
            fixture: fixture.into(),
            oracle_files: files(&oracle)?.len(),
            actual_files: files(&actual)?.len(),
            differences,
            roundtrip_mismatches,
        });
    }
    Ok(FullRunReport {
        logical_entries: document.entries().len(),
        fixtures,
    })
}

fn verify_hash(path: &Path, expected: &str, label: &str) -> io::Result<()> {
    let actual = lower_hex(&Sha256::digest(fs::read(path)?));
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(io::Error::other(format!(
            "{label} hash mismatch: expected {expected}, got {actual}"
        )))
    }
}

fn run_command(command: &mut Command, label: &str) -> io::Result<()> {
    let output = command
        .output()
        .map_err(|error| io::Error::new(error.kind(), format!("{label}: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(command_error(label, &output))
    }
}

fn command_error(label: &str, output: &Output) -> io::Error {
    io::Error::other(format!(
        "{label} failed with {}:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    ))
}

/// Decodes and re-encodes every compiled file below a database root.
pub fn verify_reencode_tree(root: &Path) -> io::Result<Vec<RoundtripMismatch>> {
    let mut mismatches = Vec::new();
    for (path, bytes) in files(root)? {
        let document = match terminfokit::binary::decode(&bytes) {
            Ok(document) => document,
            Err(error) => {
                mismatches.push(RoundtripMismatch {
                    relative_path: path,
                    reason: format!("decode failed: {error}"),
                });
                continue;
            }
        };
        let format = match document.magic() {
            terminfokit::Magic::Legacy => terminfokit::NumberFormat::Legacy,
            terminfokit::Magic::ExtendedNumbers => terminfokit::NumberFormat::Extended,
        };
        let encoded = match document
            .to_bytes_with(terminfokit::EncodeOptions::new().with_number_format(format))
        {
            Ok(encoded) => encoded,
            Err(error) => {
                mismatches.push(RoundtripMismatch {
                    relative_path: path,
                    reason: format!("encode failed: {error}"),
                });
                continue;
            }
        };
        if encoded != bytes {
            mismatches.push(RoundtripMismatch {
                relative_path: path,
                reason: "re-encoded bytes differ".into(),
            });
        }
    }
    Ok(mismatches)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_comparison_requires_explicit_allowlist() {
        let base =
            std::env::temp_dir().join(format!("terminfokit-conformance-{}", std::process::id()));
        let oracle = base.join("oracle");
        let actual = base.join("actual");
        fs::create_dir_all(&oracle).unwrap();
        fs::create_dir_all(&actual).unwrap();
        fs::write(oracle.join("entry"), b"one").unwrap();
        fs::write(actual.join("entry"), b"two").unwrap();
        let context = ComparisonContext {
            ncurses_version: "6.6".into(),
            fixture: "entry".into(),
            today: "2026-08-02".into(),
        };
        let differences = compare_trees(&oracle, &actual, &[], &context).unwrap();
        assert_eq!(differences.len(), 1);
        let allowed = [AllowlistedDifference {
            relative_path: "entry".into(),
            ncurses_version: "6.6".into(),
            reason: "fixture".into(),
            fixture: "entry".into(),
            kind: DifferenceKind::Bytes,
            expected_hash: differences[0].hash.clone(),
            expires: "2026-12-31".into(),
        }];
        assert!(
            compare_trees(&oracle, &actual, &allowed, &context)
                .unwrap()
                .is_empty()
        );
        fs::write(actual.join("entry"), b"changed").unwrap();
        assert_eq!(
            compare_trees(&oracle, &actual, &allowed, &context).unwrap()[0].kind,
            DifferenceKind::AllowlistMismatch
        );
        fs::write(actual.join("entry"), b"one").unwrap();
        assert_eq!(
            compare_trees(&oracle, &actual, &allowed, &context).unwrap()[0].kind,
            DifferenceKind::StaleAllowlist
        );
        fs::remove_dir_all(base).unwrap();
    }
}
