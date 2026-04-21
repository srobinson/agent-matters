//! `agent-matters` binary entry point.
//!
//! Thin parse and dispatch shell. The clap surface lives in
//! [`agent_matters_cli::Cli`]; dispatch glue lives in
//! [`agent_matters_cli::dispatch`].

use agent_matters_cli::{Cli, dispatch};
use clap::Parser;

fn main() {
    let cli = Cli::parse();
    match dispatch(cli) {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            eprintln!("agent-matters: {err:#}");
            std::process::exit(1);
        }
    }
}
