#![doc = include_str!("../README.md")]

use moove::*;

use clap::Parser;
use colored::*;

#[doc(hidden)]
fn main() {
    let mut args = CommandLine::parse();
    if let Ok(options) = std::env::var("MOOVE_OPTIONS") {
        args.update_from(
            std::env::args().chain(options.split_ascii_whitespace().map(|o| o.to_string())),
        )
    };
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
