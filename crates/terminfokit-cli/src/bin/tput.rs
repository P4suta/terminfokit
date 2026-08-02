// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
#[cfg(unix)]
use std::io::IsTerminal;
use std::io::{self, BufRead, Write};
use std::process::{Command, ExitCode};

use clap::{ArgAction, Parser};
use terminal_size::{Height, Width, terminal_size};
use terminfokit::caps::{
    BooleanCap, CapabilityId, Lookup, NameNamespace, NumericCap, ParameterType, StringCap,
};
use terminfokit::database::load_from_env;
use terminfokit::expand::{Expander, OutputEvent, PaddingContext, Param, Program};
use terminfokit::{BooleanState, CapabilityState, Entry, ExtendedValue};
use terminfokit_cli::{DiagnosticFormat, Reporter};

#[derive(Debug, Parser)]
#[command(
    name = "tput",
    version,
    about = "Query or expand terminfo capabilities"
)]
struct Args {
    /// Set the terminal name.
    #[arg(short = 'T')]
    terminal: Option<String>,
    /// Read one command per input line.
    #[arg(short = 'S')]
    batch: bool,
    /// Keep scrollback after clear.
    #[arg(short = 'x')]
    no_clear_scrollback: bool,
    /// Skip iprog during init and reset.
    #[arg(long)]
    no_init_program: bool,
    /// Increase verbosity.
    #[arg(short = 'v', action = ArgAction::Count)]
    verbose: u8,
    /// Capability and its parameters.
    #[arg(allow_hyphen_values = true)]
    operands: Vec<String>,
    #[arg(long, value_enum, default_value_t = DiagnosticFormat::Human)]
    diagnostic_format: DiagnosticFormat,
}

fn main() -> ExitCode {
    main_from(std::env::args().collect())
}

pub fn main_from(arguments: Vec<String>) -> ExitCode {
    let args = Args::parse_from(arguments);
    ExitCode::from(run(args))
}

fn run(args: Args) -> u8 {
    let reporter = Reporter::new("tput", args.diagnostic_format);
    if !args.batch && args.operands.is_empty() {
        reporter.error("TIKC206", "capability required");
        return 2;
    }
    let explicit_terminal = args.terminal.is_some();
    let terminal = match args
        .terminal
        .or_else(|| std::env::var("TERM").ok())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None => {
            reporter.error("TIKC201", "terminal required; use -T or TERM");
            return 2;
        }
    };
    let entry = match load_from_env(&terminal) {
        Ok(value) => value,
        Err(error) => {
            reporter.error("TIKC202", error.to_string());
            return 3;
        }
    };
    if args.verbose != 0 {
        reporter.info(
            "TIKC203",
            format!(
                "loaded {} ({})",
                entry.names().primary(),
                entry.names().description().unwrap_or("no description")
            ),
        );
    }
    let mut expander = Expander::new();
    let execution = ExecutionOptions {
        no_clear_scrollback: args.no_clear_scrollback,
        no_init_program: args.no_init_program,
        verbose: args.verbose != 0,
        allow_environment_size: !explicit_terminal,
        reporter,
    };
    if args.batch {
        let mut status = 0;
        for line in io::stdin().lock().lines() {
            let line = match line {
                Ok(value) => value,
                Err(error) => {
                    reporter.error("TIKC204", error.to_string());
                    return io_status(&error);
                }
            };
            let operands: Vec<String> = line.split_whitespace().map(str::to_owned).collect();
            if operands.is_empty() {
                continue;
            }
            let arity = operand_arity(&entry, &operands[0]);
            let result = if arity.is_some_and(|arity| operands.len() <= arity + 1) {
                execute(&entry, &operands, execution, &mut expander)
            } else {
                reporter.error(
                    "TIKC205",
                    "each batch line needs one capability and its parameters",
                );
                4
            };
            if result > 4 {
                return result;
            }
            if result == 4 {
                status = 4;
            }
        }
        status
    } else {
        execute_many(&entry, &args.operands, execution, &mut expander)
    }
}

#[derive(Clone, Copy)]
struct ExecutionOptions {
    no_clear_scrollback: bool,
    no_init_program: bool,
    verbose: bool,
    allow_environment_size: bool,
    reporter: Reporter,
}

fn execute_many(
    entry: &Entry,
    operands: &[String],
    options: ExecutionOptions,
    expander: &mut Expander,
) -> u8 {
    let mut status = 0;
    let mut index = 0;
    while index < operands.len() {
        let arity = operand_arity(entry, &operands[index]).unwrap_or(0);
        let end = (index + 1 + arity).min(operands.len());
        status = status.max(execute(entry, &operands[index..end], options, expander));
        index = end;
    }
    status
}

fn operand_arity(entry: &Entry, name: &str) -> Option<usize> {
    if matches!(name, "longname" | "init" | "reset") {
        return Some(0);
    }
    if let Some(cap) = standard_capability(name) {
        return Some(match cap {
            CapabilityId::String(cap) => cap.metadata().parameters().len(),
            CapabilityId::Boolean(_) | CapabilityId::Number(_) => 0,
            _ => 0,
        });
    }
    entry
        .extended()
        .iter()
        .find(|cap| cap.name() == name)
        .map(|cap| match cap.state() {
            CapabilityState::Value(ExtendedValue::String(value)) => Program::parse(value)
                .map(|program| usize::from(program.analyze().parameter_count()))
                .unwrap_or(0),
            _ => 0,
        })
}

fn execute(
    entry: &Entry,
    operands: &[String],
    options: ExecutionOptions,
    expander: &mut Expander,
) -> u8 {
    let name = operands[0].as_str();
    if matches!(name, "init" | "reset") {
        return initialize(
            entry,
            name == "reset",
            options.no_init_program,
            options.verbose,
            options.reporter,
            expander,
        );
    }
    if name == "longname" {
        println!(
            "{}",
            entry
                .names()
                .verbose_name()
                .unwrap_or(entry.names().primary())
        );
        return 0;
    }
    if name == "clear" {
        let CapabilityState::Value(value) = entry.string(StringCap::CLEAR_SCREEN) else {
            return 1;
        };
        let status = emit(
            value,
            &[],
            &[],
            padding_context(entry),
            options.reporter,
            expander,
        );
        if status != 0 {
            return status;
        }
        if !options.no_clear_scrollback
            && let Some(CapabilityState::Value(ExtendedValue::String(value))) = entry
                .extended()
                .iter()
                .find(|capability| capability.name() == "E3")
                .map(|capability| capability.state())
        {
            return emit(
                value,
                &[],
                &[],
                padding_context(entry),
                options.reporter,
                expander,
            );
        }
        return 0;
    }
    if let Some(cap) = standard_capability(name) {
        return match cap {
            CapabilityId::Boolean(cap) => match entry.boolean(cap) {
                BooleanState::Set => 0,
                _ => 1,
            },
            CapabilityId::Number(cap) => {
                if let Some(value) = terminal_dimension(entry, cap, options.allow_environment_size)
                {
                    println!("{value}");
                    return 0;
                }
                match entry.number(cap) {
                    CapabilityState::Value(value) => {
                        println!("{value}");
                        0
                    }
                    CapabilityState::Absent | CapabilityState::Cancelled => {
                        println!("-1");
                        1
                    }
                }
            }
            CapabilityId::String(cap) => match entry.string(cap) {
                CapabilityState::Value(value) => emit(
                    value,
                    &operands[1..],
                    cap.metadata().parameters(),
                    padding_context(entry),
                    options.reporter,
                    expander,
                ),
                CapabilityState::Absent | CapabilityState::Cancelled => 1,
            },
            _ => 4,
        };
    }
    if let Some(cap) = entry.extended().iter().find(|cap| cap.name() == name) {
        return match cap.state() {
            CapabilityState::Value(ExtendedValue::Boolean) => 0,
            CapabilityState::Absent | CapabilityState::Cancelled => 1,
            CapabilityState::Value(ExtendedValue::Number(value)) => {
                println!("{value}");
                0
            }
            CapabilityState::Value(ExtendedValue::String(value)) => emit(
                value,
                &operands[1..],
                &[],
                padding_context(entry),
                options.reporter,
                expander,
            ),
            CapabilityState::Value(_) => 4,
        };
    }
    options
        .reporter
        .error("TIKC208", format!("unknown capability {name:?}"));
    4
}

fn standard_capability(name: &str) -> Option<CapabilityId> {
    for namespace in [
        NameNamespace::Short,
        NameNamespace::Long,
        NameNamespace::Termcap,
    ] {
        match terminfokit::caps::lookup(namespace, name) {
            Lookup::Found(cap) => return Some(cap),
            Lookup::Ambiguous(caps) => return caps.first().copied(),
            Lookup::NotFound => {}
            _ => {}
        }
    }
    None
}

fn initialize(
    entry: &Entry,
    reset: bool,
    no_init_program: bool,
    verbose: bool,
    reporter: Reporter,
    expander: &mut Expander,
) -> u8 {
    initialize_with_backend(
        entry,
        reset,
        no_init_program,
        verbose,
        reporter,
        expander,
        &mut SystemTerminalMode,
    )
}

trait TerminalModeBackend {
    fn configure(&mut self, entry: &Entry, reset: bool) -> io::Result<bool>;
}

struct SystemTerminalMode;

impl TerminalModeBackend for SystemTerminalMode {
    fn configure(&mut self, entry: &Entry, reset: bool) -> io::Result<bool> {
        configure_terminal_mode(entry, reset)
    }
}

fn initialize_with_backend(
    entry: &Entry,
    reset: bool,
    no_init_program: bool,
    verbose: bool,
    reporter: Reporter,
    expander: &mut Expander,
    terminal_mode: &mut impl TerminalModeBackend,
) -> u8 {
    if !no_init_program
        && let CapabilityState::Value(program) = entry.string(StringCap::INIT_PROG)
        && !program.is_empty()
    {
        let status = match Command::new(path_from_bytes(program)).status() {
            Ok(status) => status,
            Err(error) => {
                reporter.error("TIKC215", error.to_string());
                return io_status(&error);
            }
        };
        if !status.success() {
            reporter.error("TIKC216", format!("init program exited with {status}"));
            return 4;
        }
    }

    match terminal_mode.configure(entry, reset) {
        Ok(true) => {}
        Ok(false) if verbose => {
            reporter.warning("TIKC217", "terminal mode unavailable on this platform")
        }
        Ok(false) => {}
        Err(error) => {
            reporter.error("TIKC218", error.to_string());
            return io_status(&error);
        }
    }

    let initial = if reset {
        [
            reset_or_init(entry, StringCap::RESET_1STRING, StringCap::INIT_1STRING),
            reset_or_init(entry, StringCap::RESET_2STRING, StringCap::INIT_2STRING),
        ]
    } else {
        [
            value(entry, StringCap::INIT_1STRING),
            value(entry, StringCap::INIT_2STRING),
        ]
    };
    for sequence in initial.into_iter().flatten() {
        let status = emit(
            sequence,
            &[],
            &[],
            padding_context(entry),
            reporter,
            expander,
        );
        if status != 0 {
            return status;
        }
    }

    let status = initialize_margins(entry, reporter, expander);
    if status != 0 {
        return status;
    }
    let status = initialize_tabs(entry, reporter, expander);
    if status != 0 {
        return status;
    }

    let file = if reset {
        reset_or_init(entry, StringCap::RESET_FILE, StringCap::INIT_FILE)
    } else {
        value(entry, StringCap::INIT_FILE)
    };
    if let Some(path) = file {
        let bytes = match fs::read(path_from_bytes(path)) {
            Ok(bytes) => bytes,
            Err(error) => {
                reporter.error("TIKC219", error.to_string());
                return io_status(&error);
            }
        };
        if let Err(error) = io::stdout().lock().write_all(&bytes) {
            reporter.error("TIKC220", error.to_string());
            return io_status(&error);
        }
    }

    let final_string = if reset {
        reset_or_init(entry, StringCap::RESET_3STRING, StringCap::INIT_3STRING)
    } else {
        value(entry, StringCap::INIT_3STRING)
    };
    if let Some(sequence) = final_string {
        return emit(
            sequence,
            &[],
            &[],
            padding_context(entry),
            reporter,
            expander,
        );
    }
    0
}

fn initialize_margins(entry: &Entry, reporter: Reporter, expander: &mut Expander) -> u8 {
    if let Some(sequence) = value(entry, StringCap::CLEAR_MARGINS) {
        return emit(
            sequence,
            &[],
            &[],
            padding_context(entry),
            reporter,
            expander,
        );
    }
    let right = match entry.number(NumericCap::COLUMNS) {
        CapabilityState::Value(columns) => columns.get().saturating_sub(1),
        CapabilityState::Absent | CapabilityState::Cancelled => 79,
    }
    .to_string();
    if let Some(sequence) = value(entry, StringCap::SET_LR_MARGIN) {
        return emit(
            sequence,
            &["0".into(), right],
            &[ParameterType::Number, ParameterType::Number],
            padding_context(entry),
            reporter,
            expander,
        );
    }
    let parameterized = [
        (StringCap::SET_LEFT_MARGIN_PARM, "0".to_owned()),
        (StringCap::SET_RIGHT_MARGIN_PARM, right),
    ];
    if parameterized
        .iter()
        .all(|(cap, _)| value(entry, *cap).is_some())
    {
        for (cap, parameter) in parameterized {
            let status = emit(
                value(entry, cap).unwrap_or_default(),
                &[parameter],
                &[ParameterType::Number],
                padding_context(entry),
                reporter,
                expander,
            );
            if status != 0 {
                return status;
            }
        }
        return 0;
    }
    for cap in [StringCap::SET_LEFT_MARGIN, StringCap::SET_RIGHT_MARGIN] {
        if let Some(sequence) = value(entry, cap) {
            let status = emit(
                sequence,
                &[],
                &[],
                padding_context(entry),
                reporter,
                expander,
            );
            if status != 0 {
                return status;
            }
        }
    }
    0
}

fn initialize_tabs(entry: &Entry, reporter: Reporter, expander: &mut Expander) -> u8 {
    let CapabilityState::Value(interval) = entry.number(NumericCap::INIT_TABS) else {
        return 0;
    };
    let interval = interval.get();
    if interval == 8 || interval <= 0 {
        return 0;
    }
    let (Some(clear), Some(set)) = (
        value(entry, StringCap::CLEAR_ALL_TABS),
        value(entry, StringCap::SET_TAB),
    ) else {
        return 0;
    };
    for sequence in [value(entry, StringCap::CARRIAGE_RETURN), Some(clear)]
        .into_iter()
        .flatten()
    {
        let status = emit(
            sequence,
            &[],
            &[],
            padding_context(entry),
            reporter,
            expander,
        );
        if status != 0 {
            return status;
        }
    }
    let columns = match entry.number(NumericCap::COLUMNS) {
        CapabilityState::Value(columns) => columns.get(),
        CapabilityState::Absent | CapabilityState::Cancelled => 80,
    };
    let Some(right) = value(entry, StringCap::CURSOR_RIGHT) else {
        return 0;
    };
    let mut column = interval;
    while column < columns {
        for _ in 0..interval {
            let status = emit(right, &[], &[], padding_context(entry), reporter, expander);
            if status != 0 {
                return status;
            }
        }
        let status = emit(set, &[], &[], padding_context(entry), reporter, expander);
        if status != 0 {
            return status;
        }
        column = column.saturating_add(interval);
    }
    0
}

fn value(entry: &Entry, cap: StringCap) -> Option<&[u8]> {
    match entry.string(cap) {
        CapabilityState::Value(value) => Some(value),
        CapabilityState::Absent | CapabilityState::Cancelled => None,
    }
}

fn reset_or_init(entry: &Entry, reset: StringCap, init: StringCap) -> Option<&[u8]> {
    value(entry, reset).or_else(|| value(entry, init))
}

#[cfg(unix)]
fn path_from_bytes(value: &[u8]) -> std::path::PathBuf {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    std::path::PathBuf::from(OsStr::from_bytes(value))
}

#[cfg(not(unix))]
fn path_from_bytes(value: &[u8]) -> std::path::PathBuf {
    std::path::PathBuf::from(String::from_utf8_lossy(value).into_owned())
}

#[cfg(unix)]
fn configure_terminal_mode(entry: &Entry, reset: bool) -> io::Result<bool> {
    use std::fs::OpenOptions;
    use std::os::fd::AsFd;

    fn update(fd: impl AsFd, entry: &Entry, reset: bool) -> io::Result<()> {
        use nix::sys::termios::{
            ControlFlags, InputFlags, LocalFlags, OutputFlags, SetArg, tcgetattr, tcsetattr,
        };

        let mut attributes = tcgetattr(fd.as_fd()).map_err(io::Error::other)?;
        attributes
            .output_flags
            .insert(OutputFlags::OPOST | OutputFlags::ONLCR);
        attributes
            .control_flags
            .remove(ControlFlags::CSIZE | ControlFlags::PARENB);
        attributes
            .control_flags
            .insert(ControlFlags::CS8 | ControlFlags::CREAD);
        attributes
            .local_flags
            .insert(LocalFlags::ISIG | LocalFlags::ICANON | LocalFlags::ECHO | LocalFlags::IEXTEN);
        attributes
            .input_flags
            .insert(InputFlags::BRKINT | InputFlags::ICRNL);
        if reset || entry.boolean(BooleanCap::XON_XOFF) == BooleanState::Set {
            attributes.input_flags.insert(InputFlags::IXON);
        } else {
            attributes.input_flags.remove(InputFlags::IXON);
        }
        tcsetattr(fd.as_fd(), SetArg::TCSADRAIN, &attributes).map_err(io::Error::other)
    }

    let stderr = io::stderr();
    if stderr.is_terminal() {
        update(&stderr, entry, reset)?;
        return Ok(true);
    }
    let stdout = io::stdout();
    if stdout.is_terminal() {
        update(&stdout, entry, reset)?;
        return Ok(true);
    }
    let stdin = io::stdin();
    if stdin.is_terminal() {
        update(&stdin, entry, reset)?;
        return Ok(true);
    }
    let tty = OpenOptions::new().read(true).write(true).open("/dev/tty")?;
    update(&tty, entry, reset)?;
    Ok(true)
}

#[cfg(not(unix))]
fn configure_terminal_mode(_entry: &Entry, _reset: bool) -> io::Result<bool> {
    Ok(false)
}

fn emit(
    capability: &[u8],
    operands: &[String],
    signature: &[ParameterType],
    padding_context: PaddingContext,
    reporter: Reporter,
    expander: &mut Expander,
) -> u8 {
    let mut params = Vec::with_capacity(operands.len());
    for (index, value) in operands.iter().enumerate() {
        match signature.get(index) {
            Some(ParameterType::Bytes) => params.push(Param::Bytes(value.as_bytes())),
            Some(ParameterType::Number) => match value.parse::<i64>() {
                Ok(value) => params.push(Param::Number(value)),
                Err(_) => {
                    reporter.error(
                        "TIKC209",
                        format!("parameter {} must be a number", index + 1),
                    );
                    return 4;
                }
            },
            None => params.push(
                value
                    .parse::<i64>()
                    .map_or_else(|_| Param::Bytes(value.as_bytes()), Param::Number),
            ),
        }
    }
    let expanded = if operands.is_empty() {
        capability.to_vec()
    } else {
        let program = match Program::parse(capability) {
            Ok(value) => value,
            Err(error) => {
                reporter.error("TIKC210", error.to_string());
                return 4;
            }
        };
        let mut expanded = Vec::new();
        if let Err(error) = expander.run_into(&program, &params, &mut expanded) {
            reporter.error("TIKC211", error.to_string());
            return 4;
        }
        expanded
    };
    let mut stdout = io::stdout().lock();
    for event in terminfokit::expand::parse_padding(&expanded) {
        match event {
            OutputEvent::Bytes(bytes) => {
                if let Err(error) = stdout.write_all(bytes) {
                    reporter.error("TIKC212", error.to_string());
                    return io_status(&error);
                }
            }
            OutputEvent::Delay(delay) => {
                let Some(duration) = delay.effective(padding_context) else {
                    continue;
                };
                if let Err(error) = stdout.flush() {
                    reporter.error("TIKC213", error.to_string());
                    return io_status(&error);
                }
                std::thread::sleep(std::time::Duration::from_micros(duration * 100));
            }
        }
    }
    if let Err(error) = stdout.flush() {
        reporter.error("TIKC214", error.to_string());
        return io_status(&error);
    }
    0
}

fn terminal_dimension(entry: &Entry, cap: NumericCap, allow_environment: bool) -> Option<u32> {
    if !matches!(cap, NumericCap::COLUMNS | NumericCap::LINES) {
        return None;
    }
    if allow_environment {
        let variable = if cap == NumericCap::COLUMNS {
            "COLUMNS"
        } else {
            "LINES"
        };
        if let Some(value) = std::env::var(variable)
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value != 0)
        {
            return Some(value);
        }
    }
    if let Some(value) = terminal_size().map(|(Width(columns), Height(lines))| {
        u32::from(if cap == NumericCap::COLUMNS {
            columns
        } else {
            lines
        })
    }) {
        return Some(value);
    }
    if let CapabilityState::Value(value) = entry.number(cap) {
        return u32::try_from(value.get()).ok();
    }
    None
}

fn io_status(error: &io::Error) -> u8 {
    error
        .raw_os_error()
        .and_then(|value| u8::try_from(value).ok())
        .map_or(4, |value| 4u8.saturating_add(value))
}

fn padding_context(entry: &Entry) -> PaddingContext {
    let padding_baud_rate = match entry.number(NumericCap::PADDING_BAUD_RATE) {
        CapabilityState::Value(value) => u32::try_from(value.get()).unwrap_or(u32::MAX),
        _ => 0,
    };
    PaddingContext::new()
        .with_xon(entry.boolean(BooleanCap::XON_XOFF) == BooleanState::Set)
        .with_baud(u32::MAX)
        .with_padding_baud_rate(padding_baud_rate)
        .with_affected_lines(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeTerminalMode {
        reset: Option<bool>,
    }

    impl TerminalModeBackend for FakeTerminalMode {
        fn configure(&mut self, _entry: &Entry, reset: bool) -> io::Result<bool> {
            self.reset = Some(reset);
            Ok(true)
        }
    }

    #[test]
    fn explicit_init_and_reset_use_the_injected_terminal_backend() {
        let entry = Entry::builder("fake").unwrap().build();
        for reset in [false, true] {
            let mut backend = FakeTerminalMode::default();
            let status = initialize_with_backend(
                &entry,
                reset,
                true,
                false,
                Reporter::new("tput-test", DiagnosticFormat::Human),
                &mut Expander::new(),
                &mut backend,
            );
            assert_eq!(status, 0);
            assert_eq!(backend.reset, Some(reset));
        }
    }

    #[test]
    fn reset_strings_fall_back_individually() {
        let entry = Entry::builder("fallback")
            .unwrap()
            .string(StringCap::INIT_1STRING, b"init-one".to_vec())
            .unwrap()
            .string(StringCap::RESET_2STRING, b"reset-two".to_vec())
            .unwrap()
            .build();
        assert_eq!(
            reset_or_init(&entry, StringCap::RESET_1STRING, StringCap::INIT_1STRING),
            Some(b"init-one".as_slice())
        );
        assert_eq!(
            reset_or_init(&entry, StringCap::RESET_2STRING, StringCap::INIT_2STRING),
            Some(b"reset-two".as_slice())
        );
    }
}
