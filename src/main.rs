use blight::{Device, Direction, Change};
use std::{env,thread,time::Duration,process::{self,Command}};
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
            [case] if case == "status" => print_status(),
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
    let step_size: u16 = step_size.parse().unwrap_or_else(|_| {
        println!("{}","Invalid step size: use a positive integer".red().bold());
        process::exit(1)
    });

    let device = Device::new().unwrap_or_else(|| {
            println!("{}","Error: No known device detected on system".red());
            process::exit(1)
    });

    let change = calculate_change(device.current, device.max, step_size, &dir);
    if change != device.current {
        match ch {
            Change::Sweep => sweep(&device, change, &dir),
            Change::Regular => device.write_value(change),
        }
    }
}

fn set_bl(val: &str) {
    let val: u16 = val.parse().unwrap_or_else(|_| {
        println!("{}","Invalid value: use a positive integer".red().bold());
        process::exit(1)
    });

    let device = Device::new().unwrap_or_else(|| {
            println!("{}","Error: No known device detected on system".red());
            process::exit(1)
    });

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
    let out = Command::new("pgrep")
        .arg("-x")
        .arg(env::current_exe().unwrap().file_name().unwrap().to_str().unwrap())
        .output()
        .expect("Process command failed");
    let out = String::from_utf8(out.stdout).expect("Failed to convert");
    if out.trim().len() > 6 { true } else { false }
}

fn print_status() {
    let device = Device::new().unwrap_or_else(|| {
            println!("{}","Error: No known device detected on system".red());
            process::exit(1)
    });

    println!(
        "{}\nDetected device: {}\nCurrent Brightness: {}\nMax Brightness {}",
        "Device status".bold(),
        device.name.green(),
        device.current.to_string().green(),
        device.max.to_string().green()
    );
}

fn print_help() {
    let title = "blight automatically detects GPU device, and updates brightness accordingly.";
    let commands = "\
blight inc [opt val] - increase by 2%
blight dec [opt val] - decrease by 2%
blight set [val] - set custom brightness value
blight sweep-up [opt val] - smoothly increase by 10%
blight sweep-down [opt val] - smoothly decrease by 10%
blight status - backlight device status";
    let exampels = "Examples:
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)
    blight sweep-up 15 (smoothly increases brightness by 15%)";
    println!("{}\n\n{}\n\n{}",title.blue().bold(),commands.green().bold(),exampels.bright_yellow());
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
