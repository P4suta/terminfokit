use std::io::Write;
use std::process::{Command, Output, Stdio};

fn run_with_stdin(program: &str, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn CLI");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait for CLI")
}

fn transport(source: &[u8]) -> String {
    let output = run_with_stdin(env!("CARGO_BIN_EXE_tic"), &["-x", "-Q1", "-"], source);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn unsupported_operations_are_usage_errors_before_external_lookup() {
    let output = Command::new(env!("CARGO_BIN_EXE_tput"))
        .arg("init")
        .env_remove("TERM")
        .output()
        .expect("run tput");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("no terminal name"));

    let output = Command::new(env!("CARGO_BIN_EXE_infocmp"))
        .arg("-e")
        .env_remove("TERM")
        .output()
        .expect("run infocmp");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("outside v1"));
}

#[test]
fn integrated_commands_use_the_compatibility_implementations() {
    let source = b"demo|demo terminal,am,cols#80,clear=ok,\n";
    let compatibility = run_with_stdin(env!("CARGO_BIN_EXE_tic"), &["-c", "-"], source);
    let integrated = run_with_stdin(
        env!("CARGO_BIN_EXE_terminfokit"),
        &["compile", "-c", "-"],
        source,
    );
    assert_eq!(integrated.status.code(), compatibility.status.code());
    assert_eq!(integrated.stdout, compatibility.stdout);
    assert_eq!(integrated.stderr, compatibility.stderr);

    let compatibility = run_with_stdin(env!("CARGO_BIN_EXE_captoinfo"), &["-"], b"d:am:\n");
    let integrated = run_with_stdin(
        env!("CARGO_BIN_EXE_terminfokit"),
        &["convert", "termcap-to-terminfo", "-"],
        b"d:am:\n",
    );
    assert_eq!(integrated.status.code(), compatibility.status.code());
    assert_eq!(integrated.stdout, compatibility.stdout);
    assert_eq!(integrated.stderr, compatibility.stderr);
}

#[test]
fn tput_matches_numeric_extended_and_clear_x_rules() {
    let encoded = transport(b"demo|demo terminal,clear=base,E3=scroll,xnum#7,\n");
    let command = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_tput"))
            .args(args)
            .env("TERM", "demo")
            .env("TERMINFO", &encoded)
            .output()
            .expect("run tput")
    };

    let output = command(&["colors"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"-1\n");

    let output = command(&["xnum"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"7\n");

    let output = command(&["clear"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"basescroll");

    let output = command(&["-x", "clear"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, b"base");

    let output = command(&["colors", "xnum"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"-1\n7\n");

    let mut child = Command::new(env!("CARGO_BIN_EXE_tput"))
        .arg("-S")
        .env("TERM", "demo")
        .env("TERMINFO", &encoded)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tput -S");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"colors\nxnum\nunknown-capability\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(4));
    assert_eq!(output.stdout, b"-1\n7\n");
}

#[test]
fn doctor_reports_inline_origin_and_statuses_without_mutation() {
    let encoded = transport(b"doctor-demo|diagnostic terminal,cols#90,lines#30,colors#256,\n");
    let output = Command::new(env!("CARGO_BIN_EXE_terminfokit"))
        .args(["doctor", "-T", "doctor-demo"])
        .env("TERMINFO", encoded)
        .output()
        .expect("run doctor");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("origin=inline:hex"));
    assert!(stdout.contains("name.primary=doctor-demo"));
    assert!(stdout.contains("size=90x30"));
    assert!(stdout.contains("colors=256"));

    let output = Command::new(env!("CARGO_BIN_EXE_terminfokit"))
        .arg("doctor")
        .env_remove("TERM")
        .env_remove("TERMINFO")
        .output()
        .expect("run doctor without TERM");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn ndjson_diagnostics_are_one_structured_record_per_line() {
    let output = Command::new(env!("CARGO_BIN_EXE_tic"))
        .args(["--diagnostic-format", "ndjson", "-R", "SVr2"])
        .output()
        .expect("run tic");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).expect("diagnostic UTF-8");
    let lines: Vec<_> = stderr.lines().collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with('{'));
    assert!(lines[0].contains("\"code\":\"TIKC011\""));
    assert!(lines[0].contains("\"severity\":\"error\""));
}

#[test]
fn translation_frontends_share_binary_safe_parsers() {
    let output = run_with_stdin(
        env!("CARGO_BIN_EXE_captoinfo"),
        &["-"],
        b"# non-utf8: \xff\ndemo|demo terminal:am:co#80:cl=\\E[H\\E[2J:\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.windows(8).any(|value| value == b"cols#80,"));

    let output = run_with_stdin(
        env!("CARGO_BIN_EXE_infotocap"),
        &["-"],
        b"# non-utf8: \xff\ndemo|demo terminal,am,cols#80,clear=\\E[H,\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.windows(5).any(|value| value == b"co#80"));
}

#[test]
fn tic_check_accepts_non_utf8_comments_without_installing() {
    let output = run_with_stdin(
        env!("CARGO_BIN_EXE_tic"),
        &["-c", "-"],
        b"# arbitrary byte: \xff\ndemo,am,cols#80,\n",
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
}

#[test]
fn infocmp_source_file_mode_matches_aliases_and_reports_unmatched_entries() {
    let directory =
        std::env::temp_dir().join(format!("terminfokit-infocmp-source-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let left = directory.join("left.info");
    let right = directory.join("right.info");
    std::fs::write(&left, b"a|shared,cols#80,\nleft-only,am,\n").unwrap();
    std::fs::write(&right, b"shared|b,cols#81,\nright-only,am,\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_infocmp"))
        .arg("-F")
        .arg(&left)
        .arg(&right)
        .output()
        .expect("run infocmp -F");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("comparing a to shared."));
    assert!(stdout.contains("left-only: no match"));
    assert!(stdout.contains("right-only: no match"));

    std::fs::remove_dir_all(directory).unwrap();
}
