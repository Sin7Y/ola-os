use std::{io::Read, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use colored::Colorize;
use ethereum_types::{Secret, H256};
use ola_wallet_sdk::key_store::OlaKeyPair;

use crate::path::ExpandedPathbufParser;

#[derive(Debug, Parser)]
pub struct FromKey {
    #[clap(long, help = "Overwrite the file if it already exists")]
    force: bool,
    #[clap(long, help = "Take the private key from stdin instead of prompt")]
    private_key_stdin: bool,
    #[clap(
        long,
        help = "Supply password from command line option instead of prompt"
    )]
    password: Option<String>,
    #[clap(
        value_parser = ExpandedPathbufParser,
        help = "Path to save the JSON keystore"
    )]
    file: PathBuf,
}

impl FromKey {
    pub fn run(self) -> Result<()> {
        if self.password.is_some() {
            eprintln!(
                "{}",
                "WARNING: setting passwords via --password is generally considered insecure, \
                as they will be stored in your shell history or other log files."
                    .bright_magenta()
            );
        }

        if self.file.exists() && !self.force {
            anyhow::bail!("keystore file already exists");
        }

        let private_key = if self.private_key_stdin {
            let mut buffer = String::new();
            std::io::stdin().read_to_string(&mut buffer)?;

            buffer
        } else {
            rpassword::prompt_password("Enter private key: ")?
        };
        let private_key = private_key.trim_start_matches("0x");

        let password = if let Some(password) = self.password {
            password
        } else {
            rpassword::prompt_password("Enter password: ")?
        };

        let secret = Self::from_hex_be(private_key)?;
        let key = OlaKeyPair::new(secret)?;
        key.save_as_keystore(&self.file, &password)?;

        println!(
            "Created new encrypted keystore file: {}",
            std::fs::canonicalize(self.file)?.display()
        );

        Ok(())
    }

    fn from_hex_be(value: &str) -> Result<Secret> {
        let value = value.trim_start_matches("0x");

        let hex_chars_len = value.len();
        let expected_hex_length = 64;

        let parsed_bytes: [u8; 32] = if hex_chars_len == expected_hex_length {
            let mut buffer = [0u8; 32];
            hex::decode_to_slice(value, &mut buffer)?;
            buffer
        } else if hex_chars_len < expected_hex_length {
            let mut padded_hex = str::repeat("0", expected_hex_length - hex_chars_len);
            padded_hex.push_str(value);

            let mut buffer = [0u8; 32];
            hex::decode_to_slice(&padded_hex, &mut buffer)?;
            buffer
        } else {
            anyhow::bail!("Key out of range.");
        };
        Ok(H256(parsed_bytes))
    }
}
