// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

#![forbid(unsafe_code)]

use clap::{Parser, ValueEnum};
use terminfokit::error::{Diagnostic, DiagnosticLabel, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DiagnosticFormat {
    Human,
    #[value(alias = "json")]
    Ndjson,
}

#[derive(Debug, Clone, Copy)]
pub struct Reporter {
    program: &'static str,
    format: DiagnosticFormat,
}

#[derive(Debug, Parser)]
#[command(name = "terminfokit doctor", about = "Inspect terminfo lookup")]
struct DoctorArgs {
    /// Set the terminal name.
    #[arg(short = 'T')]
    terminal: Option<String>,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
}

/// Runs database diagnostics.
pub fn doctor_from(arguments: Vec<String>) -> std::process::ExitCode {
    use terminfokit::caps::{BooleanCap, NumericCap, StringCap};
    use terminfokit::database::{
        DirectoryLayout, LoadOrigin, SearchPath, TransportEncoding, load_from_env_report,
    };
    use terminfokit::{BooleanState, CapabilityState, Magic};

    let args = DoctorArgs::parse_from(arguments);
    let reporter = Reporter::new("terminfokit doctor", args.diagnostic_format);
    let terminal = match args
        .terminal
        .or_else(|| std::env::var("TERM").ok())
        .filter(|value| !value.is_empty())
    {
        Some(terminal) => terminal,
        None => {
            reporter.error("TIKC501", "terminal required; use -T or TERM");
            return std::process::ExitCode::from(2);
        }
    };

    println!("TERM={terminal}");
    let search = SearchPath::from_env();
    for (index, root) in search.roots().iter().enumerate() {
        println!("search[{index}]={}", root.display());
    }
    let report = match load_from_env_report(&terminal) {
        Ok(report) => report,
        Err(error) => {
            reporter.error("TIKC502", error.to_string());
            return std::process::ExitCode::from(3);
        }
    };
    match report.origin() {
        LoadOrigin::Inline { encoding } => println!(
            "origin=inline:{}",
            match encoding {
                TransportEncoding::Hex => "hex",
                TransportEncoding::Base64 => "base64",
            }
        ),
        LoadOrigin::Directory { path, layout } => println!(
            "origin=directory:{} layout={}",
            path.display(),
            match layout {
                DirectoryLayout::Auto => "auto",
                DirectoryLayout::Letter => "letter",
                DirectoryLayout::Hex => "hex",
                _ => "unknown",
            }
        ),
        _ => println!("origin=unknown"),
    }
    let entry = report.entry();
    println!("name.primary={}", entry.names().primary());
    println!("name.aliases={}", entry.names().aliases().join(","));
    println!(
        "name.verbose={}",
        entry.names().verbose_name().unwrap_or("")
    );
    println!(
        "format={}",
        match report.magic() {
            Magic::Legacy => "legacy-16",
            Magic::ExtendedNumbers => "extended-32",
        }
    );
    let number = |cap| match entry.number(cap) {
        CapabilityState::Value(value) => value.to_string(),
        CapabilityState::Absent | CapabilityState::Cancelled => "-".into(),
    };
    println!(
        "size={}x{}",
        number(NumericCap::COLUMNS),
        number(NumericCap::LINES)
    );
    println!(
        "colors={} pairs={}",
        number(NumericCap::MAX_COLORS),
        number(NumericCap::MAX_PAIRS)
    );
    println!(
        "capabilities=am:{} clear:{} cup:{} setaf:{}",
        entry.boolean(BooleanCap::AUTO_RIGHT_MARGIN) == BooleanState::Set,
        matches!(
            entry.string(StringCap::CLEAR_SCREEN),
            CapabilityState::Value(_)
        ),
        matches!(
            entry.string(StringCap::CURSOR_ADDRESS),
            CapabilityState::Value(_)
        ),
        matches!(
            entry.string(StringCap::SET_A_FOREGROUND),
            CapabilityState::Value(_)
        )
    );
    println!("extended={}", entry.extended().len());
    std::process::ExitCode::SUCCESS
}

impl Reporter {
    pub const fn new(program: &'static str, format: DiagnosticFormat) -> Self {
        Self { program, format }
    }

    pub fn error(self, code: &str, message: impl AsRef<str>) {
        self.message("error", code, message.as_ref());
    }

    pub fn warning(self, code: &str, message: impl AsRef<str>) {
        self.message("warning", code, message.as_ref());
    }

    pub fn info(self, code: &str, message: impl AsRef<str>) {
        self.message("info", code, message.as_ref());
    }

    pub fn diagnostic(self, value: &Diagnostic) {
        match self.format {
            DiagnosticFormat::Human => {
                if let Some(label) = value.primary() {
                    let span = label.span();
                    eprintln!(
                        "source-{}:{}: {} [{}]: {}",
                        span.source_id().get(),
                        span.range().start(),
                        severity(value.severity()),
                        value.code(),
                        value.message()
                    );
                } else {
                    eprintln!(
                        "{}: {} [{}]: {}",
                        self.program,
                        severity(value.severity()),
                        value.code(),
                        value.message()
                    );
                }
                for label in value.secondary() {
                    let span = label.span();
                    eprintln!(
                        "  source-{}:{}: {}",
                        span.source_id().get(),
                        span.range().start(),
                        label.message().unwrap_or("related location")
                    );
                }
                for note in value.notes() {
                    eprintln!("  note: {note}");
                }
            }
            DiagnosticFormat::Ndjson => {
                let primary = value
                    .primary()
                    .map_or_else(|| "null".to_owned(), label_json);
                let secondary = value
                    .secondary()
                    .iter()
                    .map(label_json)
                    .collect::<Vec<_>>()
                    .join(",");
                let notes = value
                    .notes()
                    .iter()
                    .map(|note| format!("\"{}\"", json(note)))
                    .collect::<Vec<_>>()
                    .join(",");
                eprintln!(
                    "{{\"program\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"message\":\"{}\",\"primary\":{primary},\"secondary\":[{secondary}],\"notes\":[{notes}]}}",
                    json(self.program),
                    severity(value.severity()),
                    json(value.code()),
                    json(value.message())
                );
            }
        }
    }

    fn message(self, severity: &str, code: &str, message: &str) {
        match self.format {
            DiagnosticFormat::Human => eprintln!("{}: {message}", self.program),
            DiagnosticFormat::Ndjson => eprintln!(
                "{{\"program\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"message\":\"{}\"}}",
                json(self.program),
                json(severity),
                json(code),
                json(message)
            ),
        }
    }
}

fn label_json(label: &DiagnosticLabel) -> String {
    let span = label.span();
    let message = label.message().map_or_else(
        || "null".to_owned(),
        |message| format!("\"{}\"", json(message)),
    );
    format!(
        "{{\"source\":{},\"start\":{},\"end\":{},\"message\":{message}}}",
        span.source_id().get(),
        span.range().start(),
        span.range().end()
    )
}

fn severity(value: Severity) -> &'static str {
    match value {
        Severity::Warning => "warning",
        Severity::Error => "error",
        _ => "diagnostic",
    }
}

fn json(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output
}
