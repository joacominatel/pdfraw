//! Command-line surface.
//!
//! The binary is what most people meet first, and a stale install is
//! invisible without a way to ask it what it is. These tests run the real
//! executable rather than calling into the library, because the argument
//! parsing and the exit codes are the thing under test.

use std::process::{Command, Output};

/// Run the built `pdfraw` binary with `args`.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_pdfraw"))
        .args(args)
        .output()
        .expect("failed to run the pdfraw binary")
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn version_flag_reports_the_crate_version() {
    for flag in ["--version", "-V"] {
        let out = run(&[flag]);
        assert!(
            out.status.success(),
            "{flag} exited {:?}",
            out.status.code()
        );
        assert_eq!(
            stdout(&out).trim(),
            format!("pdfraw {}", env!("CARGO_PKG_VERSION")),
            "{flag} printed the wrong thing"
        );
    }
}

#[test]
fn version_is_sourced_from_the_manifest() {
    // Hard-coding the string would let the binary claim a version it is not.
    let out = run(&["--version"]);
    let printed = stdout(&out);
    let version = printed
        .trim()
        .strip_prefix("pdfraw ")
        .expect("no version printed");
    assert_eq!(version, env!("CARGO_PKG_VERSION"));
}

#[test]
fn help_flag_exits_zero_on_stdout() {
    for flag in ["--help", "-h"] {
        let out = run(&[flag]);
        assert!(
            out.status.success(),
            "{flag} exited {:?}",
            out.status.code()
        );
        assert!(
            stdout(&out).contains("usage:"),
            "{flag} printed no usage line"
        );
    }
}

#[test]
fn version_wins_over_a_file_argument() {
    // Asking for the version must not depend on the rest of the line being
    // valid — that is precisely when you need it.
    let out = run(&["--version", "definitely-not-a-file.pdf"]);
    assert!(out.status.success());
    assert!(stdout(&out).starts_with("pdfraw "));
}

#[test]
fn no_arguments_is_an_error_with_usage() {
    let out = run(&[]);
    assert!(!out.status.success(), "expected a non-zero exit");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("usage:"), "no usage on stderr: {err:?}");
}

#[test]
fn an_unknown_flag_is_an_error() {
    let out = run(&["--nope"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--nope"));
}

#[test]
fn a_missing_input_file_is_an_error_not_a_panic() {
    let out = run(&["definitely-not-a-file.pdf"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("definitely-not-a-file.pdf"),
        "the error does not name the file: {err:?}"
    );
    assert!(!err.contains("panicked"), "the binary panicked: {err:?}");
}
