use anyhow::Result;
use clap::{Parser, Subcommand};

mod keystore;
use keystore::Keystore;

#[derive(Debug, Parser)]
pub struct Signer {
    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Keystore management commands")]
    Keystore(Keystore),
}

impl Signer {
    pub fn run(self) -> Result<()> {
        match self.command {
            Subcommands::Keystore(cmd) => cmd.run(),
        }
    }
}
