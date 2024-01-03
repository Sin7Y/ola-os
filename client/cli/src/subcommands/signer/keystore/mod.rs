use anyhow::Result;
use clap::{Parser, Subcommand};

mod new;
use new::New;

mod inspect_private;
use inspect_private::InspectPrivate;

mod inspect;
use inspect::Inspect;

#[derive(Debug, Parser)]
pub struct Keystore {
    #[clap(subcommand)]
    command: Subcommands,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    #[clap(about = "Randomly generate a new keystore")]
    New(New),
    InspectPrivate(InspectPrivate),
    Inspect(Inspect),
}

impl Keystore {
    pub fn run(self) -> Result<()> {
        match self.command {
            Subcommands::New(cmd) => cmd.run(),
            Subcommands::InspectPrivate(cmd) => cmd.run(),
            Subcommands::Inspect(cmd) => cmd.run(),
        }
    }
}
