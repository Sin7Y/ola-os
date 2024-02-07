#![feature(path_file_prefix)]

// use clap::{arg, ArgMatches, Command, Parser};
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;

use compile::Compile;
use subcommands::{Call, Deploy, Invoke, SetPubKey, Signer, Transaction};
pub mod compile;
pub mod errors;
pub mod path;
pub mod subcommands;
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
    #[clap(about = "Signer management commands")]
    Signer(Signer),
    #[clap(about = "Send an invoke transaction from an account contract")]
    Invoke(Invoke),
    #[clap(about = "Deploy contract via the Universal Deployer Contract")]
    Deploy(Deploy),
    #[clap(about = "Set public key to default account management contract")]
    SetPubKey(SetPubKey),
    #[clap(
        about = "Executes a new message call immediately without creating a transaction on the blockchain"
    )]
    Call(Call),
    #[clap(alias = "tx", about = "Get Ola transaction by hash")]
    Transaction(Transaction),
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
            Subcommands::Signer(cmd) => cmd.run(),
            Subcommands::Invoke(cmd) => cmd.run().await,
            Subcommands::Deploy(cmd) => cmd.run().await,
            Subcommands::SetPubKey(cmd) => cmd.run().await,
            Subcommands::Call(cmd) => cmd.run().await,
            Subcommands::Transaction(cmd) => cmd.run().await,
        },
    }
}
