use blight::{Device, Direction, Change};
use std::{env,thread,time::Duration};
use colored::*;

fn main() {
    let args: Vec<String> = env::args().collect();
    let (inc, dec) = (Direction::Inc, Direction::Dec);
    let (reg, sweep) = (Change::Regular, Change::Sweep);

    if args.len() > 1 {
        match &args[1..] {
            [dir, step_size] if dir == "inc" => change_bl(step_size, reg, inc),
            [dir, step_size] if dir == "dec" => change_bl(step_size, reg, dec),
            [case, value] if case == "set" => set_bl(value),
            [case, value] if case == "set" => set_bl(value),
            [case, value] if case == "sweep-up" => change_bl(value, sweep, inc),
            [case, value] if case == "sweep-down" => change_bl(value, sweep, dec),
            [dir] if dir == "inc" => change_bl("2", reg, inc),
            [dir] if dir == "dec" => change_bl("2", reg, dec),
            [case] if case == "sweep-up" => change_bl("10", sweep, inc),
            [case] if case == "sweep-down" => change_bl("10", sweep, dec),
            _ => print_help(),
        }
    } else {
        print_help();
    }
}

fn calculate_change(current: u16, max: u16, step_size: u16, dir: &Direction) -> u16 {
    let step: u16 = (max as f32 * (step_size as f32 / 100.0)) as u16;
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

fn change_bl(step_size: &str, ch: Change, dir: Direction) {
    let step_size: u16 = match step_size.parse() {
        Ok(n) => n,
        Err(_) => {
            println!("{}","Invalid step size: use a positive integer".red().bold());
            return;
        }
    };
    let device = Device::new();
    let change = calculate_change(device.current, device.max, step_size, &dir);
    match ch {
        Change::Sweep => {
            if let Direction::Inc = &dir {
                let mut val = device.current + 1;

                while val <= change {
                    device.write_value(val);
                    thread::sleep(Duration::from_millis(30));
                    val += 1;
                }
            } else {
                let mut val = device.current - 1;

                while val >= change {
                    device.write_value(val);
                    thread::sleep(Duration::from_millis(30));
                    val -= 1;
                }
            }
        },
        Change::Regular => {
            if change != device.current {
                device.write_value(change);
            }
        }
    }
}

fn set_bl(val: &str) {
    let val: u16 = match val.parse() {
        Ok(n) => n,
        Err(_) => {
            println!("{}","Invalid value: use a positive integer".red().bold());
            return;
        }
    };
    let device = Device::new();
    if (val <= device.max) & (val != device.current) {
        device.write_value(val);
    }
}

fn print_help() {
    let title = "blight automatically detects GPU device, and updates brightness accordingly.";
    let commands = "blight inc [ocerride step size] - increase brightness
blight dec [override step size] - decrease brightness
blight set [value] - set custom brightness value";
    let exampels = "Examples:
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)";
    println!("{}\n\n{}\n\n{}",title.blue().bold(),commands.magenta(),exampels.bright_yellow());
}
