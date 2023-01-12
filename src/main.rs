use std::env;

mod utils;

fn main() {
    if utils::is_running() {
        utils::print_err("Another instance of blight is already running", None);
        return;
    }

    let config = match utils::parse(env::args().skip(1)) {
        Ok(c) => c,
        Err((e, t)) => {
            utils::print_err(e, t);
            return;
        }
    };

    match utils::execute(config) {
        Err((e, t)) => utils::print_err(e, t),
        Ok(msg) => utils::print_ok(msg),
    }
}
