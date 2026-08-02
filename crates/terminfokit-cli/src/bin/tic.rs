use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use terminfokit::binary::{EncodeOptions, NumberFormat};
use terminfokit::database::{
    DirectoryDatabase, InstallOptions, SearchPath, TransportEncoding, default_install_root,
    encode_transport_with,
};
use terminfokit::format::{FormatOptions, Layout, NameStyle, SourceFormatter};
use terminfokit::{Compiler, CompilerOptions};
use terminfokit_cli::{DiagnosticFormat, Reporter};

#[derive(Debug, Parser)]
#[command(
    name = "tic",
    version,
    about = "Compile terminfo source without a C ncurses dependency"
)]
struct Args {
    /// Translate resolved entries to termcap source instead of installing.
    #[arg(short = 'C')]
    termcap_output: bool,
    /// Translate resolved entries to terminfo source using short names.
    #[arg(short = 'I', conflicts_with_all = ["termcap_output", "long_source"])]
    terminfo_source: bool,
    /// Translate resolved entries to terminfo source using long names.
    #[arg(short = 'L', conflicts_with_all = ["termcap_output", "terminfo_source"])]
    long_source: bool,
    /// Emit translated source on one line.
    #[arg(short = '0', conflicts_with = "one_per_line")]
    compact: bool,
    /// Emit one capability per translated-source line.
    #[arg(short = '1')]
    one_per_line: bool,
    /// Width used for translated source.
    #[arg(short = 'w', default_value_t = 60)]
    width: usize,
    /// Emit compiled entries as hex (1), base64 (2), or both (3).
    #[arg(short = 'Q', num_args = 0..=1, default_missing_value = "1")]
    transport: Option<u8>,
    /// Use the conservative BSD termcap compatibility profile.
    #[arg(short = 'K')]
    termcap_compatibility: bool,
    /// Do not enforce the historical termcap size limit.
    #[arg(short = 'T')]
    termcap_unlimited: bool,
    /// Resolve inheritance before translation (resolution is always enabled).
    #[arg(short = 'r')]
    resolve: bool,
    /// Preserve user-defined capabilities.
    #[arg(short = 'x')]
    extended: bool,
    /// Retain dot-prefixed commented-out capabilities (implies -x).
    #[arg(short = 'a')]
    retain_commented: bool,
    /// Compile only these comma-separated entry names (all input remains available to use=).
    #[arg(short = 'e', value_delimiter = ',')]
    entries: Vec<String>,
    /// Output database root.
    #[arg(short = 'o')]
    output: Option<PathBuf>,
    /// Check only; do not install entries.
    #[arg(short = 'c')]
    check: bool,
    /// Print the effective database search roots.
    #[arg(short = 'D')]
    directories: bool,
    /// Print a compilation summary.
    #[arg(short = 's')]
    summary: bool,
    /// Archaic terminfo subset selection is intentionally unsupported.
    #[arg(short = 'R', value_name = "SUBSET", hide = true)]
    unsupported_subset: Option<String>,
    /// Increase verbosity (`-vv` and ncurses-style `-v3` are accepted).
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
    /// Source file, or `-` for standard input.
    #[arg(allow_hyphen_values = true)]
    source: Option<String>,
}

fn main() -> ExitCode {
    main_from(std::env::args().collect())
}

pub fn main_from(arguments: Vec<String>) -> ExitCode {
    let args = Args::parse_from(normalized_args(arguments));
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn normalized_args(arguments: Vec<String>) -> Vec<String> {
    arguments
        .into_iter()
        .flat_map(|arg| {
            if let Some(count) = arg
                .strip_prefix("-v")
                .filter(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
                .and_then(|rest| rest.parse::<usize>().ok())
            {
                std::iter::repeat_n("-v".to_owned(), count.min(255)).collect::<Vec<_>>()
            } else {
                vec![arg]
            }
        })
        .collect()
}

fn run(args: Args) -> Result<(), u8> {
    let reporter = Reporter::new("tic", args.diagnostic_format);
    if args.unsupported_subset.is_some() {
        reporter.error(
            "TIKC011",
            "-R archaic subset output is outside v1; use -I/-L source output and filter capabilities explicitly",
        );
        return Err(2);
    }
    let search = SearchPath::from_env();
    if args.directories {
        for root in search.roots() {
            println!("{}", root.display());
        }
        if args.source.is_none() {
            return Ok(());
        }
    }
    let source_name = args.source.as_deref().unwrap_or("-");
    let source = match read_source(source_name) {
        Ok(source) => source,
        Err(error) => {
            reporter.error("TIKC001", error.to_string());
            return Err(1);
        }
    };
    let options = CompilerOptions::new()
        .with_extended(args.extended || args.retain_commented)
        .with_retain_commented(args.retain_commented)
        .with_number_format(NumberFormat::Auto);
    let encode_options = EncodeOptions::new().with_extended(args.extended || args.retain_commented);
    let compilation = match Compiler::new()
        .options(options)
        .provider(&search)
        .compile(&source)
    {
        Ok(value) => value,
        Err(error) => {
            if error.diagnostics().is_empty() {
                reporter.error("TIKC002", error.to_string());
            } else {
                for diagnostic in error.diagnostics() {
                    reporter.diagnostic(diagnostic);
                }
            }
            return Err(1);
        }
    };
    for diagnostic in compilation.diagnostics() {
        reporter.diagnostic(diagnostic);
    }

    let selected: Vec<_> = compilation
        .entries()
        .iter()
        .filter(|item| {
            args.entries.is_empty()
                || args.entries.iter().any(|name| {
                    item.entry().names().primary() == name
                        || item
                            .entry()
                            .names()
                            .aliases()
                            .iter()
                            .any(|alias| alias == name)
                })
        })
        .collect();
    if !args.entries.is_empty() {
        for requested in &args.entries {
            if compilation.get(requested).is_none() {
                reporter.error(
                    "TIKC003",
                    format!("selected entry {requested:?} does not exist"),
                );
                return Err(1);
            }
        }
    }
    if args.termcap_output {
        if args.resolve && args.verbose != 0 {
            reporter.info("TIKC004", "emitting fully resolved termcap entries");
        }
        for item in &selected {
            let mut options = if args.termcap_compatibility {
                terminfokit::termcap::ConvertOptions::bsd()
            } else {
                terminfokit::termcap::ConvertOptions::ncurses_6_6()
            };
            if args.termcap_unlimited {
                options = options.unlimited();
            }
            match terminfokit::termcap::from_entry(item.entry(), options) {
                Ok(value) => {
                    for warning in value.warnings() {
                        reporter.warning(
                            "TIKC005",
                            format!("{}: {}", warning.capability(), warning.message()),
                        );
                    }
                    if !args.check {
                        print!("{}", value.source());
                    }
                }
                Err(error) => {
                    reporter.error("TIKC006", error.to_string());
                    return Err(1);
                }
            }
        }
        report_summary(&args, &compilation, selected.len(), reporter);
        return Ok(());
    }
    if let Some(mode) = args.transport {
        if !(1..=3).contains(&mode) {
            reporter.error(
                "TIKC012",
                "-Q accepts only 1 (hex), 2 (base64), or 3 (both)",
            );
            return Err(2);
        }
        if !args.check {
            for item in &selected {
                if mode != 2 {
                    match encode_transport_with(
                        item.entry(),
                        TransportEncoding::Hex,
                        encode_options,
                    ) {
                        Ok(value) => println!("{value}"),
                        Err(error) => {
                            reporter.error("TIKC013", error.to_string());
                            return Err(1);
                        }
                    }
                }
                if mode != 1 {
                    match encode_transport_with(
                        item.entry(),
                        TransportEncoding::Base64,
                        encode_options,
                    ) {
                        Ok(value) => println!("{value}"),
                        Err(error) => {
                            reporter.error("TIKC013", error.to_string());
                            return Err(1);
                        }
                    }
                }
            }
        }
        report_summary(&args, &compilation, selected.len(), reporter);
        return Ok(());
    }
    if args.terminfo_source || args.long_source {
        if !args.check {
            let layout = if args.compact {
                Layout::Compact
            } else if args.one_per_line {
                Layout::OnePerLine
            } else {
                Layout::Wrapped { width: args.width }
            };
            let formatter = SourceFormatter::new(
                FormatOptions::new()
                    .with_names(if args.long_source {
                        NameStyle::Long
                    } else {
                        NameStyle::Short
                    })
                    .with_layout(layout)
                    .with_extended(args.extended || args.retain_commented),
            );
            for item in &selected {
                print!("{}", formatter.format(item.entry()));
            }
        }
        report_summary(&args, &compilation, selected.len(), reporter);
        return Ok(());
    }
    if !args.check {
        let root = args
            .output
            .clone()
            .or_else(default_install_root)
            .ok_or_else(|| {
                reporter.error("TIKC007", "no output root; pass -o or set TERMINFO");
                1u8
            })?;
        let database = DirectoryDatabase::new(root);
        for item in &selected {
            match database.install(
                item.entry(),
                InstallOptions::default().with_encode_options(encode_options),
            ) {
                Ok(report) => {
                    if args.verbose != 0 {
                        reporter.info("TIKC008", format!("wrote {}", report.primary().display()));
                    }
                }
                Err(error) => {
                    reporter.error("TIKC009", error.to_string());
                    return Err(1);
                }
            }
        }
    }
    report_summary(&args, &compilation, selected.len(), reporter);
    Ok(())
}

fn report_summary(
    args: &Args,
    compilation: &terminfokit::Compilation,
    selected: usize,
    reporter: Reporter,
) {
    if args.summary {
        reporter.info(
            "TIKC010",
            format!(
                "{} entries compiled; {} selected; {} diagnostics",
                compilation.entries().len(),
                selected,
                compilation.diagnostics().len()
            ),
        );
    }
}

fn read_source(path: &str) -> io::Result<Vec<u8>> {
    if path == "-" {
        let mut source = Vec::new();
        io::stdin().read_to_end(&mut source)?;
        Ok(source)
    } else {
        fs::read(path)
    }
}
