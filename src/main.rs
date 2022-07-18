use std::env;
use blight::{Direction,Device};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (inc, dec) = (Direction::Inc, Direction::Dec);

    if args.len() > 1 {
        match &args[1..] {
            [dir,step_size] if dir == "inc" => change_bl(step_size, inc),
            [dir,step_size] if dir == "dec" => change_bl(step_size, dec),
            [case,value] if case == "set" => set_bl(value),
            [case,value] if case == "set" => set_bl(value),
            [dir] if dir == "inc" => change_bl("2", inc),
            [dir] if dir == "dec" => change_bl("2", dec),
            _ => print_help(),
        }
    } else {
        print_help();
    }
}

fn calculate_change(current: u16, max: u16, step_size: u16, dir: Direction) -> u16 {
    let step: u16 = (max as f32 * (step_size as f32 / 100.0 )) as u16;
    let change: u16 = match dir {
            Direction::Inc => current.saturating_add(step),
            Direction::Dec => current.saturating_sub(step),
    };

    if change > max {
        max
    } else {
        change
    }

}

fn change_bl(step_size: &str, dir: Direction) {
    let step_size: u16 = match step_size.parse() {
        Ok(n) => n,
        Err(_) => {
            println!("Invalid step size: use a positive integer");
            return
        },
    };
    let device = Device::new();
    let change = calculate_change(device.current, device.max, step_size, dir);
    if change != device.current {
        device.write_value(change);
    }
}

fn set_bl(val: &str) {
    let val: u16 = match val.parse() {
        Ok(n) => n,
        Err(_) => {
            println!("Invalid value: use a positive integer");
            return
        }
    };
    let device = Device::new();
    if (val <= device.max) & (val != device.current) {
        device.write_value(val);
    }
}

fn print_help() {
    let help_str = "blight automatically detects GPU device, and updates brightness accordingly.\n
blight inc [ocerride step size] - increase brightness
blight dec [override step size] - decrease brightness
blight set [value] - set custom brightness value

Examples:
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)
    ";
    println!("{help_str}")
}
