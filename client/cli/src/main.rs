#![feature(path_file_prefix)]

use clap::{arg, ArgMatches, Command};

use compile::compile_ola_to_current_dir;
pub mod compile;
pub mod errors;
pub mod utils;

fn main() {
    let app = || {
        Command::new("olatte")
            .version(env!("CARGO_PKG_VERSION"))
            .author(env!("CARGO_PKG_AUTHORS"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .subcommand(
                Command::new("compile")
                    .about("Compile ola source files")
                    .args(&[arg!(-i --input <INPUT> "Missing input ola file")])
                    .arg_required_else_help(true),
            )
    };
    let matches = app().get_matches();

    match matches.subcommand() {
        Some(("compile", matches)) => compile(matches),
        None | Some(_) => {
            app().print_help().unwrap();
            println!();
        }
    }
}

fn compile(matches: &ArgMatches) {
    let input_path = matches.get_one::<String>("input").expect("required");
    match compile_ola_to_current_dir(input_path.clone()) {
        Ok(_) => println!("compile success"),
        Err(e) => eprintln!("compile error: {}", e),
    }
}
