use std::process::ExitCode;

#[allow(dead_code)]
#[path = "captoinfo.rs"]
mod captoinfo_command;
#[allow(dead_code)]
#[path = "infocmp.rs"]
mod infocmp_command;
#[allow(dead_code)]
#[path = "infotocap.rs"]
mod infotocap_command;
#[allow(dead_code)]
#[path = "tic.rs"]
mod tic_command;
#[allow(dead_code)]
#[path = "tput.rs"]
mod tput_command;

fn main() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().collect();
    if arguments.len() < 2 {
        usage();
        return ExitCode::from(2);
    }
    let command = arguments.remove(1);
    match command.as_str() {
        "-h" | "--help" | "help" => {
            usage();
            ExitCode::SUCCESS
        }
        "-V" | "--version" => {
            println!("terminfokit {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "compile" | "tic" => {
            arguments[0] = "tic".into();
            tic_command::main_from(arguments)
        }
        "inspect" | "infocmp" => {
            arguments[0] = "infocmp".into();
            infocmp_command::main_from(arguments)
        }
        "query" | "tput" => {
            arguments[0] = "tput".into();
            tput_command::main_from(arguments)
        }
        "doctor" => {
            arguments[0] = "terminfokit doctor".into();
            terminfokit_cli::doctor_from(arguments)
        }
        "convert" => convert(arguments),
        _ => {
            eprintln!("terminfokit: unknown command {command:?}");
            usage();
            ExitCode::from(2)
        }
    }
}

fn convert(mut arguments: Vec<String>) -> ExitCode {
    if arguments.len() < 2 {
        eprintln!("terminfokit: convert requires a conversion name");
        return ExitCode::from(2);
    }
    let conversion = arguments.remove(1);
    match conversion.as_str() {
        "termcap-to-terminfo" => {
            arguments[0] = "captoinfo".into();
            captoinfo_command::main_from(arguments)
        }
        "terminfo-to-termcap" => {
            arguments[0] = "infotocap".into();
            infotocap_command::main_from(arguments)
        }
        _ => {
            eprintln!("terminfokit: unknown conversion {conversion:?}");
            ExitCode::from(2)
        }
    }
}

fn usage() {
    println!(
        "Usage: terminfokit <COMMAND> [OPTIONS]\n\n\
Commands:\n  compile   Compile terminfo source (alias: tic)\n  inspect   Inspect or compare entries (alias: infocmp)\n  query     Query capabilities (alias: tput)\n  convert   Convert termcap/terminfo source\n  doctor    Diagnose lookup and the selected entry"
    );
}
