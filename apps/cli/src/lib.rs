use std::env;

mod args;
mod catalog;
#[path = "io/mod.rs"]
mod cli_io;
mod commands;
mod errors;
mod execute;
pub mod internal_metadata;
mod parser;
mod spec;
#[cfg(test)]
mod tests;
mod web_assets;
mod web_payload;
mod web_server;

use crate::args::CliError;

pub fn main() {
    let args: Vec<String> = env::args().collect();
    let exit_code = match run(&args) {
        Ok(code) => code,
        Err(err) => {
            eprint!("{}", errors::render_text(&err));
            1
        }
    };
    std::process::exit(exit_code);
}

fn run(raw_args: &[String]) -> Result<i32, CliError> {
    commands::run::run_root_command(raw_args)
}
