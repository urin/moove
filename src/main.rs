#![doc = include_str!("../README.md")]

use moove::*;

use clap::Parser;
use colored::*;
use std::io::IsTerminal;

#[doc(hidden)]
fn main() {
    let mut args = CommandLine::parse();
    if let Ok(env) = std::env::var("MOOVE_OPTIONS") {
        let env_args = CommandLine::parse_from(
            std::env::args()
                .take(1)
                .chain(env.split_ascii_whitespace().map(|o| o.to_string())),
        );
        args.dry_run = args.dry_run || env_args.dry_run;
        args.verbose = args.verbose || env_args.verbose;
        args.quiet = args.quiet || env_args.quiet;
        args.absolute = args.absolute || env_args.absolute;
        args.directory = args.directory || env_args.directory;
        args.with_hidden = args.with_hidden || env_args.with_hidden;
    }
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        args.oops = true;
        let mut line = String::new();
        while let Ok(size) = stdin.read_line(&mut line) {
            if size == 0 {
                break;
            }
            args.paths
                .push(line.trim_end_matches(['\r', '\n']).to_owned());
            line.clear();
        }
    }
    if args.paths.is_empty() {
        args.paths.push(".".to_owned());
    }
    match try_main(&args) {
        Err(err) => {
            if !args.quiet {
                eprintln!("{} {:?}", "Error:".bright_red().bold(), err);
            }
            std::process::exit(2);
        }
        Ok(processed) => {
            if !args.quiet {
                if processed == 0 {
                    println!("{} {}", "Info:".bright_cyan(), "Nothing to do".dimmed());
                } else {
                    println!(
                        "{} Processed total {}",
                        "Success:".green().bold(),
                        processed.to_string().cyan()
                    );
                }
            }
        }
    }
}
