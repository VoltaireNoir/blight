#![cfg(feature = "cli")]

use std::env;

mod cli;

fn main() {
    cli::PanicReporter::init();

    let config = match cli::parse(env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            cli::print_err(e);
            return;
        }
    };

    match cli::execute(config) {
        Err(e) => cli::print_err(e),
        Ok(msg) => cli::print_ok(msg),
    }
}
