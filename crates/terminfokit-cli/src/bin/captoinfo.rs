// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use terminfokit::CompilerOptions;
use terminfokit::database::SearchPath;
use terminfokit::format::{FormatOptions, Layout, SourceFormatter};
use terminfokit_cli::{DiagnosticFormat, Reporter};

#[derive(Debug, Parser)]
#[command(name = "captoinfo", version, about = "Convert termcap to terminfo")]
struct Args {
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    #[arg(short = 'w', default_value_t = 60)]
    width: usize,
    /// Resolve tc= using input and installed entries.
    #[arg(short = 'r')]
    resolve: bool,
    /// Keep user-defined capabilities.
    #[arg(short = 'x')]
    extended: bool,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
    files: Vec<String>,
}

fn main() -> ExitCode {
    main_from(std::env::args().collect())
}

pub fn main_from(arguments: Vec<String>) -> ExitCode {
    let args = Args::parse_from(arguments);
    let reporter = Reporter::new("captoinfo", args.diagnostic_format);
    let mut sources = Vec::new();
    if args.files.is_empty() && io::stdin().is_terminal() {
        if let Ok(termcap) = std::env::var("TERMCAP")
            && !termcap.is_empty()
        {
            if Path::new(&termcap).is_file() {
                match read(&termcap) {
                    Ok(source) => sources.push((termcap, source)),
                    Err(error) => {
                        reporter.error("TIKC301", error.to_string());
                        return ExitCode::from(1);
                    }
                }
            } else {
                sources.push(("TERMCAP".to_owned(), termcap.into_bytes()));
            }
        } else if let Ok(term) = std::env::var("TERM")
            && !term.is_empty()
        {
            match SearchPath::from_env().load(&term) {
                Ok(entry) => {
                    let formatter = SourceFormatter::new(
                        FormatOptions::new()
                            .with_layout(Layout::Wrapped { width: args.width })
                            .with_extended(args.extended),
                    );
                    print!("{}", formatter.format(&entry));
                    return ExitCode::SUCCESS;
                }
                Err(error) => {
                    reporter.error("TIKC306", error.to_string());
                    return ExitCode::from(1);
                }
            }
        } else {
            reporter.error(
                "TIKC307",
                "input required; use a file, stdin, TERMCAP, or TERM",
            );
            return ExitCode::from(2);
        }
    } else {
        let files = if args.files.is_empty() {
            vec!["-".to_owned()]
        } else {
            args.files.clone()
        };
        for file in files {
            match read(&file) {
                Ok(source) => sources.push((file, source)),
                Err(error) => {
                    reporter.error("TIKC301", error.to_string());
                    return ExitCode::from(1);
                }
            }
        }
    }
    for (file, source) in sources {
        match terminfokit::termcap::parse(&source) {
            Ok(document) => {
                if args.verbose != 0 {
                    reporter.info("TIKC302", format!("converted {file}"));
                }
                if args.resolve {
                    let options = CompilerOptions::new().with_extended(args.extended);
                    let search = SearchPath::from_env();
                    let compilation =
                        match terminfokit::termcap::compile(&document, options, Some(&search)) {
                            Ok(value) => value,
                            Err(error) => {
                                reporter.error("TIKC303", error.to_string());
                                return ExitCode::from(1);
                            }
                        };
                    let formatter = SourceFormatter::new(
                        FormatOptions::new()
                            .with_layout(Layout::Wrapped { width: args.width })
                            .with_extended(args.extended),
                    );
                    for item in compilation.entries() {
                        print!("{}", formatter.format(item.entry()));
                    }
                } else {
                    let value = document.to_terminfo_source_with_width(args.width);
                    if let Err(error) = io::stdout().write_all(&value) {
                        reporter.error("TIKC304", error.to_string());
                        return ExitCode::from(1);
                    }
                }
            }
            Err(error) => {
                reporter.error("TIKC305", error.to_string());
                return ExitCode::from(1);
            }
        }
    }
    ExitCode::SUCCESS
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
