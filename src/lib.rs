//! # About
//! **blight is a hassle-free CLI for managing backlight on Linux laptops with hybrid GPU configuration.** \
//! \
//! > **Note:** You need write permission for the file `/sys/class/backlight/{your_device}/brightness` for this utility to work.
//! > Read more about it [here](https://wiki.archlinux.org/title/Backlight#ACPI).
//! ## Common commands
//! - `blight inc`
//! - `blight dec`
//! - `blight set`
//! - `blight status`
//! - `blight sweep-up`
//! - `blight sweep-down`
//!
//! Run `blight` in terminal to display all supported commands and options

use colored::*;
use futures::executor::block_on;
use std::{
    fs,
    env,
    thread,
    process::{self, Command},
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
    device_dir: String,
}

impl Device {
    /// Creates a new Device instance by reading values from /sys/class/backlight/ directory based on the detected GPU device.\
    /// Returns the Device struct wrapped in Some() or returns None when no known device is detected \
    /// This is how the devices are priorirized AmdGPU or Intel > Nvdia > ACPI > Any Fallback Device
    pub fn new() -> Option<Device> {
        let name = Self::detect_device(BLDIR)?;
        Some(block_on(Self::load(name)))
    }

    async fn load(name: String) -> Device {
        let device_dir = format!("{BLDIR}/{name}");
        Device { current: Self::get_current(&device_dir).await,
                 max: Self::get_max(&device_dir).await,
                 device_dir,
                 name,
        }
    }

    fn detect_device(bldir: &str) -> Option<String> {
        let dirs = fs::read_dir(bldir).expect("Failed to read dir");
        let mut nv: bool = false;
        let mut acpi: bool = false;
        let mut count: u8 = 0;
        let mut fallback = String::new();

        for entry in dirs {
            let  name = entry.unwrap().file_name();
            if let Some(name) = name.to_str() {
                if !nv && name.contains("nvidia") { nv = true };
                if !acpi && name.contains("acpi") { acpi = true };

                if name.contains("amdgpu") || name.contains("intel") {
                    return Some(name.to_string())
                }
                fallback = name.to_string();
            }
            count += 1;
        };

        if count == 0 { return None }

        if nv { Some(String::from("nvidia_0")) }
        else if acpi { Some(String::from("acpi_video0")) }
        else { Some(fallback) }
    }

    async fn get_max(device_dir: &str) -> u16 {
        let max: u16 = fs::read_to_string(format!("{device_dir}/max_brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        max
    }

    async fn get_current(device_dir: &str) -> u16 {
        let current: u16 = fs::read_to_string(format!("{device_dir}/brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        current
    }
    /// This method is used to write to the brightness file containted in /sys/class/backlight/ dir of the respective detected device.\
    /// It takes in a brightness value, and writes to othe relavant brightness file. If it fails, it prints the error message with a helpful tip
    /// to stderr.
    pub fn write_value(&self, value: u16) {
        if let Err(err) = fs::write(format!("{}/brightness",self.device_dir), format!("{value}")) {
            let tip = format!("\
Make sure you have write permissions for the file '{BLDIR}/{}/brightness'
Visit https://wiki.archlinux.org/title/Backlight#Hardware_interfaces
if you're unsure what to do.",self.name).green();
            eprintln!(
                "Error: {}\nTip: {}",
                err.to_string().red(),
                tip,
            )
        }
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

trait ErrorHandler {
    type ReturnTarget: ?Sized;
    fn err_handler(self) -> Self::ReturnTarget;
}

impl ErrorHandler for Option<Device> {
    type ReturnTarget = Device;
    fn err_handler(self) -> Self::ReturnTarget {
        self.unwrap_or_else(|| {
            eprintln!("{}", "Error: No known device detected on system".red().bold());
            process::exit(1)
        })
    }
}

impl ErrorHandler for Result<u16, std::num::ParseIntError> {
    type ReturnTarget = u16;
    fn err_handler(self) -> Self::ReturnTarget {
        self.unwrap_or_else(|_| {
            eprintln!("{}", "Invalid step size: use a positive integer".red().bold());
            process::exit(1)
        })
    }
}

/// Changes backlight based on step-size (percentage), change type and direction.
/// Regular change uses calculated change value based on step size and is applied instantly
/// Sweep change on the other hand, occurs gradually, producing a fade or sweeping effect.
pub fn change_bl(step_size: &str, ch: Change, dir: Direction) {
    let step_size: u16 = step_size.parse().err_handler();

    let device = Device::new().err_handler();

    let change = calculate_change(device.current, device.max, step_size, &dir);
    if change != device.current {
        match ch {
            Change::Sweep => sweep(&device, change, &dir),
            Change::Regular => device.write_value(change),
        }
    }
}

/// This function takes a brightness value, creates a Device struct, and writes the value to the brightness file
/// as long as the given value falls under the min and max bounds.
/// Unlike change_bl, this function does not calculate any change, it writes the given value directly.
pub fn set_bl(val: &str) {
    let val: u16 = val.parse().err_handler();

    let device = Device::new().err_handler();

    if (val <= device.max) & (val != device.current) {
        device.write_value(val);
    }
}

/// This function takes a borrow of Device struct, a calculated change value and the direction.
/// It writes to the relavant brightness file in an increment of 1 on each loop until change value is reached.
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

/// This function is the current way of determining whether another instance of blight is running.
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

fn check_write_perm(device_name: &str, bldir: &str) -> Result<(), std::io::Error> {
    let path = format!("{bldir}/{device_name}/brightness");
    fs::read_to_string(&path)
        .and_then(|contents| fs::write(&path, contents))
        .and(Ok(()))
}

/// This function creates a Device instance and prints the detected device, along with its current and max brightness values.
pub fn print_status() {
    let device = Device::new().err_handler();

    let write_perm = match check_write_perm(&device.name, BLDIR) {
        Ok(_) => "Ok".green(),
        Err(err) => format!("{err}").red(),
    };

    println!(
        "{}\nDetected device: {}\nWrite Permission: {}\nCurrent Brightness: {}\nMax Brightness {}",
        "Device status".bold(),
        device.name.green(),
        write_perm,
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
    use std::error::Error;
    const TESTDIR: &str = "testbldir";

    #[test]
    fn detecting_device_nvidia() {
        clean_up();
        setup_test_env(&["nvidia_0","generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_some());
        assert_eq!(name.unwrap(),"nvidia_0");
        clean_up();
    }

    #[test]
    fn detecting_device_amd() {
        clean_up();
        setup_test_env(&["nvidia_0","generic","amdgpu_x"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_some());
        assert_eq!(name.unwrap(),"amdgpu_x");
        clean_up();
    }

    #[test]
    fn detecting_device_acpi() {
        clean_up();
        setup_test_env(&["acpi_video0","generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_some());
        assert_eq!(name.unwrap(),"acpi_video0");
        clean_up();
    }

    #[test]
    fn detecting_device_fallback() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_some());
        assert_eq!(name.unwrap(),"generic");
        clean_up();
    }

    #[test]
    fn writing_value() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let d = Device { name: "generic".to_string(), max:100, current: 50, device_dir: format!("{TESTDIR}/generic") };
        d.write_value(100);
        let r = fs::read_to_string(format!("{TESTDIR}/generic/brightness")).expect("failed to read test backlight value");
        let res = r.trim();
        assert_eq!("100",res,"Result was {res}");
        clean_up();
    }

    #[test]
    fn current_value() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let current = block_on(Device::get_current(&format!("{TESTDIR}/generic")));
        assert_eq!(current.to_string(),"50");
        clean_up();
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

    #[test]
    #[should_panic]
    fn write_permission_not_ok() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        fs::File::open(format!("{TESTDIR}/generic/brightness"))
            .and_then(|f| {
                let mut p = f.metadata().unwrap().permissions();
                p.set_readonly(true);
                f.set_permissions(p)
            });
        check_write_perm("generic", TESTDIR).unwrap()
    }

    fn setup_test_env(dirs: &[&str]) -> Result<(),Box<dyn Error>> {
        fs::create_dir(TESTDIR)?;
        for dir in dirs {
            fs::create_dir(format!("{TESTDIR}/{dir}"))?;
            fs::write(format!("{TESTDIR}/{dir}/brightness"), "50")?;
            fs::write(format!("{TESTDIR}/{dir}/max"), "100")?;
        }
        Ok(())
    }

    fn clean_up() {
        if fs::read_dir(".").unwrap().any(|dir| {dir.unwrap().file_name().as_os_str() == "testbldir"}) {
            fs::remove_dir_all(TESTDIR).expect("Failed to clean up testing backlight directory.")
        }
    }
}
