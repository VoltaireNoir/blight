use std::env;

mod utils;

fn main() {
    if blight::is_running() {
        utils::print_err("Another instance of blight is already running");
        return;
    }

    let config = match utils::parse(env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            utils::print_err(e);
            return;
        }
    };

    match utils::execute(config) {
        Err(e) => utils::print_err(e),
        Ok(msg) => utils::print_ok(msg),
    }
}
