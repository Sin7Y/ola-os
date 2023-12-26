#![feature(path_file_prefix)]

// use clap::{arg, ArgMatches, Command, Parser};
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

use compile::Compile;
pub mod compile;
pub mod errors;
pub mod path;
pub mod utils;

#[derive(Debug, Parser)]
#[clap(author, about)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Subcommands>,
    #[clap(long = "version", short = 'V', help = "Print version info and exit")]
    version: bool,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Compile ola source files to abi and binary")]
    Compile(Compile),
}

#[tokio::main]
async fn main() {
    if let Err(err) = run_command(Cli::parse()).await {
        eprintln!("{}", format!("Error: {err}").red());
        std::process::exit(1);
    }
}

async fn run_command(cli: Cli) -> Result<()> {
    match (cli.version, cli.command) {
        (false, None) => Ok(Cli::command().print_help()?),
        (true, _) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        (false, Some(command)) => match command {
            Subcommands::Compile(cmd) => cmd.run(),
        },
    }
}
