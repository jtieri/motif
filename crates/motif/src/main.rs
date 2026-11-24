//! The motif command.

mod import;

use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// Import, browse and preview desktop color themes.
#[derive(Debug, Parser)]
#[command(name = "motif", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Import(import::Args),
}

fn main() -> ExitCode {
    // Usage errors, --help and --version exit here, with status 2 or 0.
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Import(args) => import::run(&args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            error(&err);
            ExitCode::FAILURE
        }
    }
}

/// Prints an error and its chain of causes on one line.
fn error(err: &dyn Error) {
    let mut line = format!("motif: {err}");
    let mut source = err.source();
    while let Some(cause) = source {
        line.push_str(": ");
        line.push_str(&cause.to_string());
        source = cause.source();
    }
    diagnostic(&line);
}

/// Prints a warning.
fn warning(message: &dyn fmt::Display) {
    diagnostic(&format!("motif: warning: {message}"));
}

/// Writes a line to standard error. A failure to do so cannot be reported
/// anywhere, so it is ignored.
fn diagnostic(line: &str) {
    let _ = writeln!(io::stderr().lock(), "{line}");
}
