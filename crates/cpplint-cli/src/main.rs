mod commands;
mod options;

use commands::{run_check_command, run_rule_command};
use options::{ParsedCommand, parse_cli};
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_cli() {
        ParsedCommand::Check(args) => run_check_command(&args),
        ParsedCommand::Rule(args) => run_rule_command(&args),
    }
}
