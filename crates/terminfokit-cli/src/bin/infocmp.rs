use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use terminfokit::caps::{BooleanCap, NumericCap, StringCap};
use terminfokit::database::{DirectoryDatabase, SearchPath, TransportEncoding, encode_transport};
use terminfokit::format::{
    CapabilitySort, FormatOptions, Layout, NameStyle, SourceFormatter, escape,
};
use terminfokit::{CapabilityState, Compiler, Entry};
use terminfokit_cli::{DiagnosticFormat, Reporter};

#[derive(Debug, Parser)]
#[command(
    name = "infocmp",
    version,
    about = "Decompile or compare compiled terminfo entries"
)]
struct Args {
    /// Emit termcap instead of terminfo source.
    #[arg(short = 'C')]
    termcap: bool,
    /// Use the conservative termcap compatibility profile.
    #[arg(short = 'K')]
    termcap_compatibility: bool,
    /// Do not enforce the termcap size limit.
    #[arg(short = 'T')]
    termcap_unlimited: bool,
    /// Resolve inheritance when translating (compiled entries are already resolved).
    #[arg(short = 'r')]
    resolve: bool,
    /// Emit a compact single-line entry.
    #[arg(short = '0')]
    compact: bool,
    /// Emit one capability per line.
    #[arg(short = '1')]
    one_per_line: bool,
    /// Include user-defined capabilities.
    #[arg(short = 'x')]
    extended: bool,
    /// Use terminfo short names.
    #[arg(short = 'I', conflicts_with = "long_names")]
    short_names: bool,
    /// Use long C variable names.
    #[arg(short = 'L')]
    long_names: bool,
    /// Database root for the first terminal.
    #[arg(short = 'A')]
    database_a: Option<PathBuf>,
    /// Database root for the second terminal.
    #[arg(short = 'B')]
    database_b: Option<PathBuf>,
    /// Print the effective database search roots.
    #[arg(short = 'D')]
    directories: bool,
    /// Compare two terminfo source files by matching entry aliases.
    #[arg(short = 'F')]
    source_files: bool,
    /// Emit the first entry as overrides plus use= of the second entry.
    #[arg(short = 'u')]
    relative: bool,
    /// Ignore padding specifications while comparing string capabilities.
    #[arg(short = 'p')]
    ignore_padding: bool,
    /// Emit compiled entries as hex (1), base64 (2), or both (3).
    #[arg(short = 'Q', value_name = "ENCODING")]
    transport: Option<u8>,
    /// Archaic terminfo subset selection is intentionally unsupported.
    #[arg(short = 'R', value_name = "SUBSET", hide = true)]
    unsupported_subset: Option<String>,
    /// C initializer generation is intentionally unsupported.
    #[arg(short = 'e', hide = true)]
    unsupported_c_initializer: bool,
    /// Extended C initializer generation is intentionally unsupported.
    #[arg(short = 'E', hide = true)]
    unsupported_extended_initializer: bool,
    /// Initialization analysis output is intentionally unsupported.
    #[arg(short = 'i', hide = true)]
    unsupported_initialization: bool,
    /// Show differing capabilities (the default for multiple terminals).
    #[arg(short = 'd')]
    differences: bool,
    /// Request common-capability comparison output.
    #[arg(short = 'c')]
    common: bool,
    /// Request absent-in-both comparison output.
    #[arg(short = 'n')]
    neither: bool,
    /// Suppress the comparison heading.
    #[arg(short = 'q')]
    quiet: bool,
    /// Output wrapping width.
    #[arg(short = 'w', default_value_t = 60)]
    width: usize,
    /// Sort capabilities by database (d), terminfo (i), long (l), or termcap (c) name.
    #[arg(short = 's', value_parser = ["d", "i", "l", "c"])]
    sort: Option<String>,
    #[arg(action = ArgAction::Append)]
    terminals: Vec<String>,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
}

fn main() -> ExitCode {
    main_from(std::env::args().collect())
}

pub fn main_from(arguments: Vec<String>) -> ExitCode {
    let args = Args::parse_from(arguments);
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => ExitCode::from(code),
    }
}

fn run(mut args: Args) -> Result<(), u8> {
    let reporter = Reporter::new("infocmp", args.diagnostic_format);
    if args.unsupported_subset.is_some() {
        reporter.error(
            "TIKC108",
            "-R archaic subset output is outside v1; use -I/-L and filter capabilities explicitly",
        );
        return Err(2);
    }
    if args.unsupported_c_initializer || args.unsupported_extended_initializer {
        reporter.error(
            "TIKC109",
            "C initializer output (-e/-E) is outside v1; use the terminfokit Rust API or infocmp -I",
        );
        return Err(2);
    }
    if args.unsupported_initialization {
        reporter.error(
            "TIKC110",
            "initialization analysis (-i) is outside v1; query the relevant capabilities with tput",
        );
        return Err(2);
    }
    if args.source_files {
        return compare_source_files(&args, reporter);
    }
    if args.directories {
        for root in SearchPath::from_env().roots() {
            println!("{}", root.display());
        }
        if args.terminals.is_empty() {
            return Ok(());
        }
    }
    if args.terminals.is_empty() {
        match std::env::var("TERM") {
            Ok(term) if !term.is_empty() => args.terminals.push(term),
            _ => {
                reporter.error("TIKC101", "TERM is not set and no terminal was named");
                return Err(2);
            }
        }
    }
    let mut entries = Vec::new();
    for (index, name) in args.terminals.iter().enumerate() {
        let explicit = if index == 0 {
            args.database_a.as_ref()
        } else {
            args.database_b.as_ref()
        };
        let result = if let Some(root) = explicit {
            DirectoryDatabase::new(root).load(name)
        } else {
            SearchPath::from_env().load(name)
        };
        match result {
            Ok(entry) => entries.push(entry),
            Err(error) => {
                reporter.error("TIKC102", error.to_string());
                return Err(3);
            }
        }
    }
    if let Some(mode) = args.transport {
        if !(1..=3).contains(&mode) {
            reporter.error(
                "TIKC111",
                "-Q accepts only 1 (hex), 2 (base64), or 3 (both)",
            );
            return Err(2);
        }
        for entry in &entries {
            if mode != 2 {
                match encode_transport(entry, TransportEncoding::Hex) {
                    Ok(value) => println!("{value}"),
                    Err(error) => {
                        reporter.error("TIKC112", error.to_string());
                        return Err(1);
                    }
                }
            }
            if mode != 1 {
                match encode_transport(entry, TransportEncoding::Base64) {
                    Ok(value) => println!("{value}"),
                    Err(error) => {
                        reporter.error("TIKC112", error.to_string());
                        return Err(1);
                    }
                }
            }
        }
        return Ok(());
    }
    if entries.len() == 1 {
        if args.relative {
            reporter.error("TIKC103", "-u requires exactly two terminal names");
            return Err(2);
        }
        if args.termcap {
            if args.resolve && !args.quiet {
                reporter.info("TIKC104", "compiled entries are already fully resolved");
            }
            let mut options = if args.termcap_compatibility {
                terminfokit::termcap::ConvertOptions::bsd()
            } else {
                terminfokit::termcap::ConvertOptions::ncurses_6_6()
            };
            if args.termcap_unlimited {
                options = options.unlimited();
            }
            match terminfokit::termcap::from_entry(&entries[0], options) {
                Ok(value) => {
                    for warning in value.warnings() {
                        reporter.warning(
                            "TIKC105",
                            format!("{}: {}", warning.capability(), warning.message()),
                        );
                    }
                    print!("{}", value.source());
                    return Ok(());
                }
                Err(error) => {
                    reporter.error("TIKC106", error.to_string());
                    return Err(1);
                }
            }
        }
        let formatter = formatter(&args);
        print!("{}", formatter.format(&entries[0]));
        return Ok(());
    }
    if args.relative {
        if args.termcap {
            reporter.error("TIKC107", "-u cannot be combined with -C");
            return Err(2);
        }
        print!(
            "{}",
            entries[0]
                .relative_to_many(&entries[1..].iter().collect::<Vec<_>>())
                .format(&formatter(&args))
        );
        return Ok(());
    }
    for index in 1..entries.len() {
        compare(
            &args.terminals[0],
            &entries[0],
            &args.terminals[index],
            &entries[index],
            &args,
        );
    }
    Ok(())
}

fn compare_source_files(args: &Args, reporter: Reporter) -> Result<(), u8> {
    if args.terminals.len() != 2 {
        reporter.error("TIKC113", "-F requires exactly two source filenames");
        return Err(2);
    }
    let mut compilations = Vec::new();
    for path in &args.terminals {
        let source = fs::read(path).map_err(|error| {
            reporter.error("TIKC114", format!("{path}: {error}"));
            1u8
        })?;
        let compilation = Compiler::new().compile(&source).map_err(|error| {
            if error.diagnostics().is_empty() {
                reporter.error("TIKC115", format!("{path}: {error}"));
            } else {
                for diagnostic in error.diagnostics() {
                    reporter.diagnostic(diagnostic);
                }
            }
            1u8
        })?;
        compilations.push(compilation);
    }

    let left = compilations[0].entries();
    let right = compilations[1].entries();
    let mut right_matches = vec![0usize; right.len()];
    for left_entry in left {
        let matches: Vec<_> = right
            .iter()
            .enumerate()
            .filter(|(_, right_entry)| entries_share_name(left_entry.entry(), right_entry.entry()))
            .collect();
        match matches.as_slice() {
            [] => println!(
                "{}: no match in {}",
                left_entry.entry().names().primary(),
                args.terminals[1]
            ),
            [(index, right_entry)] => {
                right_matches[*index] += 1;
                compare(
                    left_entry.entry().names().primary(),
                    left_entry.entry(),
                    right_entry.entry().names().primary(),
                    right_entry.entry(),
                    args,
                );
            }
            _ => {
                for (index, _) in &matches {
                    right_matches[*index] += 1;
                }
                println!(
                    "{}: {} matches in {}",
                    left_entry.entry().names().primary(),
                    matches.len(),
                    args.terminals[1]
                );
            }
        }
    }
    for (index, right_entry) in right.iter().enumerate() {
        if right_matches[index] == 0 {
            println!(
                "{}: no match in {}",
                right_entry.entry().names().primary(),
                args.terminals[0]
            );
        }
    }
    Ok(())
}

fn entries_share_name(left: &Entry, right: &Entry) -> bool {
    core::iter::once(left.names().primary())
        .chain(left.names().aliases().iter().map(String::as_str))
        .any(|name| {
            name == right.names().primary()
                || right.names().aliases().iter().any(|alias| alias == name)
        })
}

fn formatter(args: &Args) -> SourceFormatter {
    let layout = if args.compact {
        Layout::Compact
    } else if args.one_per_line {
        Layout::OnePerLine
    } else {
        Layout::Wrapped { width: args.width }
    };
    let names = if args.long_names {
        NameStyle::Long
    } else {
        NameStyle::Short
    };
    let sort = match args.sort.as_deref() {
        Some("d") => CapabilitySort::Storage,
        Some("l") => CapabilitySort::Long,
        Some("c") => CapabilitySort::Termcap,
        Some("i") => CapabilitySort::Short,
        _ if args.long_names => CapabilitySort::Long,
        _ => CapabilitySort::Short,
    };
    SourceFormatter::new(
        FormatOptions::new()
            .with_names(names)
            .with_layout(layout)
            .with_sort(sort)
            .with_extended(args.extended),
    )
}

fn compare(left_name: &str, left: &Entry, right_name: &str, right: &Entry, args: &Args) {
    let normalized_left;
    let normalized_right;
    let (left, right) = if args.ignore_padding {
        normalized_left = without_padding(left);
        normalized_right = without_padding(right);
        (&normalized_left, &normalized_right)
    } else {
        (left, right)
    };
    let diff = left.diff(right);
    if !args.quiet {
        println!("comparing {left_name} to {right_name}.");
    }
    let left_states = states(left);
    let right_states = states(right);
    if args.common {
        for (name, value, absent) in &left_states {
            if !absent
                && right_states
                    .iter()
                    .any(|(other_name, other_value, _)| other_name == name && other_value == value)
            {
                println!("    {name}: {value}");
            }
        }
    }
    if args.neither {
        for (name, _, absent) in &left_states {
            if *absent
                && right_states
                    .iter()
                    .any(|(other_name, _, other_absent)| other_name == name && *other_absent)
            {
                println!("    {name}: absent");
            }
        }
    }
    if args.differences || (!args.common && !args.neither) {
        for item in diff.differences() {
            println!("    {}: {} != {}", item.name(), item.left(), item.right());
        }
    }
}

fn without_padding(entry: &Entry) -> Entry {
    let mut normalized = entry.clone();
    for cap in StringCap::ALL {
        if let CapabilityState::Value(value) = entry.string(*cap) {
            let mut bytes = Vec::new();
            for event in terminfokit::expand::parse_padding(value) {
                if let terminfokit::expand::OutputEvent::Bytes(value) = event {
                    bytes.extend_from_slice(value);
                }
            }
            let _ = normalized.set_string(*cap, bytes);
        }
    }
    for capability in entry.extended() {
        if let CapabilityState::Value(terminfokit::ExtendedValue::String(value)) =
            capability.state()
        {
            let mut bytes = Vec::new();
            for event in terminfokit::expand::parse_padding(value) {
                if let terminfokit::expand::OutputEvent::Bytes(value) = event {
                    bytes.extend_from_slice(value);
                }
            }
            let _ = normalized
                .set_extended(capability.name(), terminfokit::ExtendedValue::String(bytes));
        }
    }
    normalized
}

fn states(entry: &Entry) -> Vec<(String, String, bool)> {
    let mut values = Vec::with_capacity(BooleanCap::COUNT + NumericCap::COUNT + StringCap::COUNT);
    for cap in BooleanCap::ALL {
        let state = entry.boolean(*cap);
        values.push((
            cap.short_name().into(),
            format!("{state:?}"),
            state.is_absent(),
        ));
    }
    for cap in NumericCap::ALL {
        let state = entry.number(*cap);
        values.push((
            cap.short_name().into(),
            format!("{state:?}"),
            state.is_absent(),
        ));
    }
    for cap in StringCap::ALL {
        let state = entry.string(*cap);
        let text = match state {
            CapabilityState::Absent => "Absent".into(),
            CapabilityState::Cancelled => "Cancelled".into(),
            CapabilityState::Value(value) => escape(value),
        };
        values.push((cap.short_name().into(), text, state.is_absent()));
    }
    values
}
