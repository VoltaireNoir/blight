use blight::{Device, Direction, Change};
use std::{env,thread,time::Duration,process::Command};
use colored::*;

fn main() {
    if is_running() { return }

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
            return
        }
    };
    let device = match Device::new() {
        Some(d) => d,
        None => {
            println!("{}","Error: No known device detected on system".red());
            return ;
        }
    };
    let change = calculate_change(device.current, device.max, step_size, &dir);
    if change != device.current {
        match ch {
            Change::Sweep => sweep(&device, change, &dir),
            Change::Regular => device.write_value(change),
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
    let device = match Device::new() {
        Some(d) => d,
        None => {
            println!("{}","Error: No known device detected on system".red());
            return;
        }
    };
    if (val <= device.max) & (val != device.current) {
        device.write_value(val);
    }
}

fn sweep(device: &Device, change: u16, dir: &Direction) {
    match dir {
        Direction::Inc => {
            let mut val = device.current + 1;

            while val <= change {
                device.write_value(val);
                thread::sleep(Duration::from_millis(25));
                val += 1;
            }
        },
        Direction::Dec => {
            let mut val = device.current - 1;

            while val >= change {
                device.write_value(val);
                thread::sleep(Duration::from_millis(25));
                if val != 0 {val -= 1} else {break};
            }
        }
    }
}

fn is_running() -> bool {
    let name = env::current_exe().unwrap().file_name().unwrap().to_string_lossy().into_owned();
    let out = Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .expect("Process command failed");
    let out = String::from_utf8(out.stdout).expect("Failed to convert");
    if out.trim().len() > 6 { true } else { false }
}

fn print_help() {
    let title = "blight automatically detects GPU device, and updates brightness accordingly.";
    let commands = "blight inc [ocerride step size] - increase brightness
blight dec [override step size] - decrease brightness
blight set [value] - set custom brightness value
blight sweep-up [override step size] - increase brightness smoothly (default by 10%)
blight sweep-down [override step size] - decrease brightness smoothly (default by 10%)";
    let exampels = "Examples:
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)
    blight sweep-up 15 (smoothly increases brightness by 15%)";
    println!("{}\n\n{}\n\n{}",title.blue().bold(),commands.magenta(),exampels.bright_yellow());
}

// Unit tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inc_calculation() {
        let ch = calculate_change(10, 100, 10, &Direction::Inc);
        assert_eq!(ch,20)
    }

    #[test]
    fn dec_calculation() {
        let ch = calculate_change(30, 100, 10, &Direction::Dec);
        assert_eq!(ch,20)
    }

    #[test]
    fn inc_calculation_max() {
        let ch = calculate_change(90, 100, 20, &Direction::Inc);
        assert_eq!(ch,100)
    }

    #[test]
    fn dec_calculation_max() {
        let ch = calculate_change(10, 100, 20, &Direction::Dec);
        assert_eq!(ch,0)
    }
}
