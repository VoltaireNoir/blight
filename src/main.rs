use blight::{Change, Direction};
use std::env;

fn main() {
    if blight::is_running() {
        return;
    }

    let args: Vec<String> = env::args().collect();
    let (inc, dec) = (Direction::Inc, Direction::Dec);
    let (reg, sweep) = (Change::Regular, Change::Sweep);

    if args.len() > 1 {
        match &args[1..] {
            [dir, step_size] if dir == "inc" => blight::change_bl(step_size, reg, inc),
            [dir, step_size] if dir == "dec" => blight::change_bl(step_size, reg, dec),
            [case, value] if case == "set" => blight::set_bl(value),
            [case, value] if case == "set" => blight::set_bl(value),
            [case, value] if case == "sweep-up" => blight::change_bl(value, sweep, inc),
            [case, value] if case == "sweep-down" => blight::change_bl(value, sweep, dec),
            [dir] if dir == "inc" => blight::change_bl("2", reg, inc),
            [dir] if dir == "dec" => blight::change_bl("2", reg, dec),
            [case] if case == "sweep-up" => blight::change_bl("10", sweep, inc),
            [case] if case == "sweep-down" => blight::change_bl("10", sweep, dec),
            [case] if case == "status" => blight::print_status(),
            _ => blight::print_help(),
        }
    } else {
        blight::print_help();
    }
}
