// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

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
            arguments[0] = "tik-tic".into();
            tic_command::main_from(arguments)
        }
        "inspect" | "infocmp" => {
            arguments[0] = "tik-infocmp".into();
            infocmp_command::main_from(arguments)
        }
        "query" | "tput" => {
            arguments[0] = "tik-tput".into();
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
        eprintln!("terminfokit: conversion required");
        return ExitCode::from(2);
    }
    let conversion = arguments.remove(1);
    match conversion.as_str() {
        "termcap-to-terminfo" => {
            arguments[0] = "tik-captoinfo".into();
            captoinfo_command::main_from(arguments)
        }
        "terminfo-to-termcap" => {
            arguments[0] = "tik-infotocap".into();
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
Commands:\n  compile   Compile terminfo (alias: tic, standalone: tik-tic)\n  inspect   Inspect entries (alias: infocmp, standalone: tik-infocmp)\n  query     Query capabilities (alias: tput, standalone: tik-tput)\n  convert   Convert termcap or terminfo\n  doctor    Inspect lookup\n\n\
The standalone binaries are prefixed so they do not shadow the ncurses tools\n\
of the same name."
    );
}
