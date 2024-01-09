use anyhow::Result;
use clap::Parser;

#[derive(Debug, Parser)]
pub struct GenKeypair {}

impl GenKeypair {
    pub fn run(self) -> Result<()> {
        println!("Not implemented yet");
        Ok(())
    }
}
