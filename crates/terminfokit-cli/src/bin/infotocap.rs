// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use terminfokit::Compiler;
use terminfokit::database::SearchPath;
use terminfokit::termcap::{ConvertOptions, from_entry};
use terminfokit_cli::{DiagnosticFormat, Reporter};

#[derive(Debug, Parser)]
#[command(name = "tik-infotocap", version, about = "Convert terminfo to termcap")]
struct Args {
    /// Use ncurses lossy conversion.
    #[arg(short = 'K')]
    compatibility: bool,
    /// Disable the 1023-byte limit.
    #[arg(short = 'T')]
    unlimited: bool,
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    #[arg(short = 'w', default_value_t = 60)]
    width: usize,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
    files: Vec<String>,
}

fn main() -> ExitCode {
    main_from(std::env::args().collect())
}

pub fn main_from(arguments: Vec<String>) -> ExitCode {
    let args = Args::parse_from(arguments);
    let reporter = Reporter::new("tik-infotocap", args.diagnostic_format);
    if args.files.is_empty()
        && io::stdin().is_terminal()
        && let Ok(term) = std::env::var("TERM")
        && !term.is_empty()
    {
        return match SearchPath::from_env().load(&term) {
            Ok(entry) => convert_entry(&entry, &args, reporter),
            Err(error) => {
                reporter.error("TIKC406", error.to_string());
                ExitCode::from(1)
            }
        };
    }
    let files = if args.files.is_empty() {
        vec!["-".to_owned()]
    } else {
        args.files.clone()
    };
    for file in files {
        let source = match read(&file) {
            Ok(value) => value,
            Err(error) => {
                reporter.error("TIKC401", error.to_string());
                return ExitCode::from(1);
            }
        };
        let compilation = match Compiler::new().compile(&source) {
            Ok(value) => value,
            Err(error) => {
                reporter.error("TIKC402", error.to_string());
                return ExitCode::from(1);
            }
        };
        for item in compilation.entries() {
            let status = convert_entry(item.entry(), &args, reporter);
            if status != ExitCode::SUCCESS {
                return status;
            }
        }
    }
    ExitCode::SUCCESS
}

fn convert_entry(entry: &terminfokit::Entry, args: &Args, reporter: Reporter) -> ExitCode {
    let mut options = if args.compatibility {
        ConvertOptions::ncurses_6_6()
    } else {
        ConvertOptions::default()
    };
    if args.unlimited {
        options = options.unlimited();
    }
    options = options.with_width(args.width);
    match from_entry(entry, options) {
        Ok(value) => {
            for warning in value.warnings() {
                reporter.warning(
                    "TIKC403",
                    format!("{}: {}", warning.capability(), warning.message()),
                );
            }
            if args.verbose != 0 {
                reporter.info("TIKC404", format!("converted {}", entry.names().primary()));
            }
            print!("{}", value.source());
            ExitCode::SUCCESS
        }
        Err(error) => {
            reporter.error("TIKC405", error.to_string());
            ExitCode::from(1)
        }
    }
}

fn read(path: &str) -> io::Result<Vec<u8>> {
    if path == "-" {
        let mut value = Vec::new();
        io::stdin().read_to_end(&mut value)?;
        Ok(value)
    } else {
        fs::read(path)
    }
}
