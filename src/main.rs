#![doc = include_str!("../README.md")]

use moove::*;

use anyhow::Result;
use clap::Parser;
use colored::*;

#[doc(hidden)]
fn main() -> Result<()> {
    let args = CommandLine::parse();
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
    Ok(())
}
