//! # blight
//! **blight is a hassle-free CLI for managing backlight on Linux laptops with hybrid GPU configuration.** \
//! \
//! Run `blight` to display all supported commands and options\
//! > Note: *You need write permission to `/sys/class/backlight/{your_device}/brightness` for this utility to work.*

use std::fs;
use colored::*;
use futures::executor::block_on;
use std::{
    env,
    process::{self, Command},
    thread,
    time::Duration,
};

const BLDIR: &str = "/sys/class/backlight";

/// This enum is used to specify the direction in which the backlight should be changed.
/// Inc -> Increase, Dec -> Decrease.
pub enum Direction {
    Inc,
    Dec
}

/// This enum is used to specify the kind of backlight change to carry out. \
/// Regular change applies the calculated change directly, whereas the sweep change occurs in incremental steps.
pub enum Change {
    Sweep,
    Regular,
}

/// Contains name of the detected GPU device and its current and max brightness values.
#[derive(Debug)]
pub struct Device {
    pub name: String,
    pub current: u16,
    pub max: u16,
}

impl Device {
    /// Creates a new Device instance by reading values from /sys/class/backlight/ directory based on the detected GPU device.\
    /// Returns the Device struct wrapped in Some() or returns None when no known device is detected \
    /// Intel and AmdGPU are prioritized, if they are absent Nvidia is used, otherwise it falls back on ACPI.
    pub fn new() -> Option<Device> {
        let name = Self::detect_device()?;
        Some(block_on(Self::load(name)))
    }

    async fn load(name: String) -> Device {
        Device { current: Self::get_current(&name).await,
                 max: Self::get_max(&name).await,
                 name,
        }
    }

    fn detect_device() -> Option<String> {
        let dirs = fs::read_dir(BLDIR).expect("Failed to read dir");
        let mut nv: bool = false;
        let mut acpi: bool = false;
        for entry in dirs {
                let  name = entry.unwrap().file_name();
                if let Some(name) = name.to_str() {
                    if !nv && name.contains("nvidia") { nv = true };
                    if !acpi && name.contains("acpi") { acpi = true };

                    if name.contains("amdgpu") || name.contains("intel") {
                        return Some(name.to_string())
                    }
                }
            };

        if nv { Some(String::from("nvidia_0")) }
        else if acpi { Some(String::from("acpi_video0")) }
        else { None }
    }

    async fn get_max(device: &str) -> u16 {
        let max: u16 = fs::read_to_string(format!("{BLDIR}/{device}/max_brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        max
    }

    async fn get_current(device: &str) -> u16 {
        let current: u16 = fs::read_to_string(format!("{BLDIR}/{device}/brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        current
    }
    /// This method is used to write to the brightness file containted in /sys/class/backlight/ dir of the respective detected device.\
    /// It takes in a brightness value, and writes to othe relavant brightness file.
    pub fn write_value(&self, value: u16) {
        fs::write(format!("{BLDIR}/{}/brightness",self.name), format!("{value}"))
            .expect("Couldn't write to brightness file");
    }

}

/// Calculates the new value to be written to the brightness file based on the provided step-size (percentage) and direction,
/// for the given current and max values of the detected GPU device.
pub fn calculate_change(current: u16, max: u16, step_size: u16, dir: &Direction) -> u16 {
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

/// Changes backlight based on step-size (percentage), change type and direction.\
/// Regular change uses calculated change value based on step size and is applied instantly\
/// Sweep change on the other hand, occurs gradually, producing a fade or sweeping effect.
pub fn change_bl(step_size: &str, ch: Change, dir: Direction) {
    let step_size: u16 = step_size.parse().unwrap_or_else(|_| {
        println!("{}", "Invalid step size: use a positive integer".red().bold());
        process::exit(1)
    });

    let device = Device::new().unwrap_or_else(|| {
        println!("{}", "Error: No known device detected on system".red().bold());
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

/// This function takes a brightness value, creates a Device struct, and writes the value to the brightness file
/// as long as the given value falls under the min and max bounds.\
/// Unlike change_bl, this function does not calculate any change, it writes the given value directly.
pub fn set_bl(val: &str) {
    let val: u16 = val.parse().unwrap_or_else(|_| {
        println!("{}", "Invalid value: use a positive integer".red().bold());
        process::exit(1)
    });

    let device = Device::new().unwrap_or_else(|| {
        println!("{}", "Error: No known device detected on system".red());
        process::exit(1)
    });

    if (val <= device.max) & (val != device.current) {
        device.write_value(val);
    }
}

/// This function takes a borrow of Device struct, a calculated change value and the direction.\
/// It writes to the relavant brightness file in an increment of 1 on each loop until change value is reached.\
/// Each loop has a delay of 25ms, to produce to a smooth sweeping effect when executed.
pub fn sweep(device: &Device, change: u16, dir: &Direction) {
    match dir {
        Direction::Inc => {
            let mut val = device.current + 1;

            while val <= change {
                device.write_value(val);
                thread::sleep(Duration::from_millis(25));
                val += 1;
            }
        }
        Direction::Dec => {
            let mut val = device.current - 1;

            while val >= change {
                device.write_value(val);
                thread::sleep(Duration::from_millis(25));
                if val == 0 { break }
                val -= 1
            }
        }
    }
}

/// This function is the current way of determining whether another instance of blight is running.\
/// This method depends on pgrep but this may be replaced with a better implementation in the future.
pub fn is_running() -> bool {
    let out = Command::new("pgrep")
        .arg("-x")
        .arg(
            env::current_exe()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
        )
        .output()
        .expect("Process command failed");
    let out = String::from_utf8(out.stdout).expect("Failed to convert");
    if out.trim().len() > 6 {
        true
    } else {
        false
    }
}

/// This function creates a Device instance and prints the detected device, along with its current and max brightness values.
pub fn print_status() {
    let device = Device::new().unwrap_or_else(|| {
        println!("{}", "Error: No known device detected on system".red());
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

/// This function prints helpful information about the CLI, such as available commands and examples.
pub fn print_help() {
    let title = "blight automatically detects GPU device, and updates brightness accordingly.";
    let commands = "\
blight inc [opt val] - increase by 2%
blight dec [opt val] - decrease by 2%
blight set [val] - set custom brightness value
blight sweep-up [opt val] - smoothly increase by 10%
blight sweep-down [opt val] - smoothly decrease by 10%
blight status - backlight device status";
    let exampels = "\
Examples:
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)
    blight sweep-up 15 (smoothly increases brightness by 15%)";

    println!(
        "{}\n\n{}\n\n{}",
        title.blue().bold(),
        commands.green().bold(),
        exampels.bright_yellow()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecting_device() {
        let name = Device::detect_device();
        assert!(name.is_some());
        println!("Detected device name: {}",name.unwrap());
    }

    #[test]
    fn writing_value() {
        let d = Device::new().unwrap();
        d.write_value(50);
        let r = fs::read_to_string(format!("{BLDIR}/{}/brightness",d.name)).expect("failed to read during test");
        let res = r.trim();
        assert_eq!("50",res,"Result was {res}")
    }

    #[test]
    fn current_value() {
        let current = block_on(Device::get_current("nvidia_0"));
        let expected = fs::read_to_string(format!("{BLDIR}/nvidia_0/brightness")).unwrap();
        assert_eq!(current.to_string(),expected.trim())
    }

    #[test]
    fn inc_calculation() {
        let ch = calculate_change(10, 100, 10, &Direction::Inc);
        assert_eq!(ch, 20)
    }

    #[test]
    fn dec_calculation() {
        let ch = calculate_change(30, 100, 10, &Direction::Dec);
        assert_eq!(ch, 20)
    }

    #[test]
    fn inc_calculation_max() {
        let ch = calculate_change(90, 100, 20, &Direction::Inc);
        assert_eq!(ch, 100)
    }

    #[test]
    fn dec_calculation_max() {
        let ch = calculate_change(10, 100, 20, &Direction::Dec);
        assert_eq!(ch, 0)
    }

}
