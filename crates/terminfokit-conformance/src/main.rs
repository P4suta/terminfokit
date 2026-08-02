use std::path::PathBuf;
use std::process::ExitCode;

use terminfokit_conformance::{FullRunConfig, run_full, verify_reencode_tree};

fn main() -> ExitCode {
    let mut arguments = std::env::args_os().skip(1);
    let Some(archive) = arguments.next() else {
        eprintln!("usage: terminfokit-conformance <ncurses-6.6.tar.gz> <work-dir> <YYYY-MM-DD>");
        return ExitCode::from(2);
    };
    if archive == "--verify-tree" {
        let Some(root) = arguments.next() else {
            eprintln!("usage: terminfokit-conformance --verify-tree <database-root>");
            return ExitCode::from(2);
        };
        if arguments.next().is_some() {
            eprintln!("terminfokit-conformance: unexpected extra argument");
            return ExitCode::from(2);
        }
        let mismatches = match verify_reencode_tree(&PathBuf::from(root)) {
            Ok(mismatches) => mismatches,
            Err(error) => {
                eprintln!("terminfokit-conformance: {error}");
                return ExitCode::from(1);
            }
        };
        for mismatch in &mismatches {
            eprintln!(
                "re-encode mismatch: {}: {}",
                mismatch.relative_path.display(),
                mismatch.reason
            );
        }
        println!("re-encode mismatches: {}", mismatches.len());
        return if mismatches.is_empty() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    let Some(work) = arguments.next() else {
        eprintln!("usage: terminfokit-conformance <ncurses-6.6.tar.gz> <work-dir> <YYYY-MM-DD>");
        return ExitCode::from(2);
    };
    let Some(today) = arguments.next().and_then(|value| value.into_string().ok()) else {
        eprintln!("usage: terminfokit-conformance <ncurses-6.6.tar.gz> <work-dir> <YYYY-MM-DD>");
        return ExitCode::from(2);
    };
    if arguments.next().is_some() {
        eprintln!("terminfokit-conformance: unexpected extra argument");
        return ExitCode::from(2);
    }
    let config = FullRunConfig {
        archive: PathBuf::from(archive),
        work: PathBuf::from(work),
        today,
        allowlist: Vec::new(),
    };
    let report = match run_full(&config) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("terminfokit-conformance: {error}");
            return ExitCode::from(1);
        }
    };
    println!("logical entries: {}", report.logical_entries);
    for fixture in &report.fixtures {
        println!(
            "{}: oracle={} actual={} differences={} reencode-mismatches={}",
            fixture.fixture,
            fixture.oracle_files,
            fixture.actual_files,
            fixture.differences.len(),
            fixture.roundtrip_mismatches.len()
        );
        for difference in &fixture.differences {
            eprintln!(
                "{}: {:?}: {} ({})",
                fixture.fixture,
                difference.kind,
                difference.relative_path.display(),
                difference.hash
            );
        }
        for mismatch in &fixture.roundtrip_mismatches {
            eprintln!(
                "{}: re-encode mismatch: {}: {}",
                fixture.fixture,
                mismatch.relative_path.display(),
                mismatch.reason
            );
        }
    }
    if report.is_exact() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
