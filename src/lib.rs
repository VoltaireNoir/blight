//! # About
//! **blight is a hassle-free CLI for managing backlight on Linux; one that plays well with hybrid GPU configuration and proprietary drivers..** \
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
use err::BlibError;
use std::{env, fs, path::PathBuf, process::Command, thread, time::Duration};

pub mod err;
pub mod setup;

const BLDIR: &str = "/sys/class/backlight";
const SAVEDIR: &str = "/.local/share/blight";

type BlResult<T> = Result<T, BlibError>;

/// This enum is used to specify the direction in which the backlight should be changed.
/// Inc -> Increase, Dec -> Decrease.
#[derive(Clone, Copy)]
pub enum Direction {
    Inc,
    Dec,
}

/// This enum is used to specify the kind of backlight change to carry out. \
/// Regular change applies the calculated change directly, whereas the sweep change occurs in incremental steps.
#[derive(Default)]
pub enum Change {
    #[default]
    Regular,
    Sweep,
}

/// Contains name of the detected GPU device and its current and max brightness values.
#[derive(Debug)]
pub struct Device {
    name: String,
    current: u16,
    max: u16,
    device_dir: String,
}

impl Device {
    /// Creates a new Device instance by reading values from /sys/class/backlight/ directory based on the detected GPU device.\
    /// Returns the Device struct wrapped in Some() or returns None when no known device is detected \
    /// This is how the devices are priorirized AmdGPU or Intel > Nvdia > ACPI > Any Fallback Device
    pub fn new(name: Option<String>) -> BlResult<Device> {
        let name = if let Some(n) = name {
            PathBuf::from(format!("{BLDIR}/{n}/brightness"))
                .is_file()
                .then_some(n)
                .ok_or(BlibError::NoDeviceFound)?
        } else {
            Self::detect_device(BLDIR)?
        };
        let device = Self::load(name)?;
        Ok(device)
    }

    fn load(name: String) -> BlResult<Device> {
        let device_dir = format!("{BLDIR}/{name}");
        Ok(Device {
            current: Self::get_current(&device_dir)?,
            max: Self::get_max(&device_dir)?,
            device_dir,
            name,
        })
    }

    fn detect_device(bldir: &str) -> BlResult<String> {
        let dirs = fs::read_dir(bldir).expect("Failed to read dir");
        let (mut nv, mut ac, mut fl): (Option<String>, Option<String>, Option<String>) =
            (None, None, None);

        for entry in dirs {
            let name = entry.unwrap().file_name();
            if let Some(name) = name.to_str() {
                if name.contains("amdgpu") || name.contains("intel") {
                    return Ok(name.to_string());
                } else if nv.is_none() && (name.contains("nvidia") | name.contains("nv")) {
                    nv = Some(name.to_string());
                } else if ac.is_none() && name.contains("acpi") {
                    ac = Some(name.to_string());
                } else {
                    fl = Some(name.to_string());
                }
            }
        }

        if let Some(nv) = nv {
            Ok(nv)
        } else if let Some(ac) = ac {
            Ok(ac)
        } else if let Some(fl) = fl {
            Ok(fl)
        } else {
            Err(BlibError::NoDeviceFound)
        }
    }

    fn get_max(device_dir: &str) -> BlResult<u16> {
        let max: u16 = fs::read_to_string(format!("{device_dir}/max_brightness"))
            .or(Err(BlibError::ReadMax))?
            .trim()
            .parse()
            .or(Err(BlibError::ReadMax))?;
        Ok(max)
    }

    fn get_current(device_dir: &str) -> BlResult<u16> {
        let current: u16 = fs::read_to_string(format!("{device_dir}/brightness"))
            .or(Err(BlibError::ReadCurrent))?
            .trim()
            .parse()
            .or(Err(BlibError::ReadCurrent))?;
        Ok(current)
    }
    /// This method is used to write to the brightness file containted in /sys/class/backlight/ dir of the respective detected device.\
    /// It takes in a brightness value, and writes to othe relavant brightness file.
    pub fn write_value(&self, value: u16) -> BlResult<()> {
        fs::write(
            format!("{}/brightness", self.device_dir),
            format!("{value}"),
        )
        .or_else(|err| {
            Err(BlibError::WriteNewVal {
                err,
                dev: self.name.clone(),
            })
        })?;

        Ok(())
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

/// Changes backlight based on step-size (percentage), change type and direction.
/// Regular change uses calculated change value based on step size and is applied instantly
/// Sweep change on the other hand, occurs gradually, producing a fade or sweeping effect.
pub fn change_bl(
    step_size: u16,
    ch: Change,
    dir: Direction,
    device_name: Option<String>,
) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    let change = calculate_change(device.current, device.max, step_size, &dir);
    if change != device.current {
        match ch {
            Change::Sweep => sweep(&device, change, &dir)?,
            Change::Regular => device.write_value(change)?,
        }
    }
    Ok(())
}

/// This function takes a brightness value, creates a Device struct, and writes the value to the brightness file
/// as long as the given value falls under the min and max bounds.
/// Unlike change_bl, this function does not calculate any change, it writes the given value directly.
pub fn set_bl(val: u16, device_name: Option<String>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    if val <= device.max && val != device.current {
        device.write_value(val)?;
    }
    Ok(())
}

/// This function takes a borrow of Device struct, a calculated change value and the direction.
/// It writes to the relavant brightness file in an increment of 1 on each loop until change value is reached.
/// Each loop has a delay of 25ms, to produce to a smooth sweeping effect when executed.
pub fn sweep(device: &Device, change: u16, dir: &Direction) -> Result<(), BlibError> {
    match dir {
        Direction::Inc => {
            let mut val = device.current + 1;

            while val <= change {
                device.write_value(val)?;
                thread::sleep(Duration::from_millis(25));
                val += 1;
            }
        }
        Direction::Dec => {
            let mut val = device.current - 1;

            while val >= change {
                device.write_value(val)?;
                thread::sleep(Duration::from_millis(25));
                if val == 0 {
                    break;
                }
                val -= 1
            }
        }
    }
    Ok(())
}

/// Saves current brightness value to "$HOME/.local/share/blight/save"
pub fn save(device_name: Option<String>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;
    let mut savedir = PathBuf::from(env::var("HOME").unwrap() + SAVEDIR);

    if !savedir.exists() && fs::create_dir_all(&savedir).is_err() {
        return Err(BlibError::CreateSaveDir(savedir));
    }

    savedir.push("blight.save");

    if fs::write(&savedir, format!("{} {}", device.name, device.current)).is_err() {
        return Err(BlibError::WriteToSaveFile(savedir));
    };

    Ok(())
}

/// Restores brightness value from "$HOME/.local/share/blight/save" if it exists.
pub fn restore() -> Result<(), BlibError> {
    let save = PathBuf::from((env::var("HOME").unwrap() + SAVEDIR) + "/blight.save");

    let restore = if save.is_file() {
        fs::read_to_string(save).map_err(BlibError::ReadFromSave)?
    } else {
        return Err(BlibError::NoSaveFound);
    };

    let (device_name, val) = restore.split_once(' ').unwrap();
    let device = Device::new(Some(device_name.to_string()))?;

    let value: u16 = val.parse().or(Err(BlibError::SaveParseErr))?;
    device.write_value(value)?;
    Ok(())
}

/// This function is the current way of determining whether another instance of blight is running.
/// This method depends on pgrep but this may be replaced with a better implementation in the future.
pub fn is_running() -> bool {
    let out = Command::new("pgrep")
        .arg("-x")
        .arg(env::current_exe().unwrap().file_name().unwrap())
        .output()
        .expect("Process command failed");
    let out = String::from_utf8(out.stdout).expect("Failed to convert");
    out.trim().len() > 6
}

fn check_write_perm(device_name: &str, bldir: &str) -> Result<(), std::io::Error> {
    let path = format!("{bldir}/{device_name}/brightness");
    fs::read_to_string(&path)
        .and_then(|contents| fs::write(&path, contents))
        .and(Ok(()))
}

/// This function creates a Device instance and prints the detected device, along with its current and max brightness values.
pub fn print_status(device_name: Option<String>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    let write_perm = match check_write_perm(&device.name, BLDIR) {
        Ok(_) => "Ok".green(),
        Err(err) => format!("{err}").red(),
    };

    println!(
        "{}\nDetected device: {}\nWrite permission: {}\nCurrent brightness: {}\nMax brightness {}",
        "Device status".bold(),
        device.name.green(),
        write_perm,
        device.current.to_string().green(),
        device.max.to_string().green()
    );
    Ok(())
}

/// Reads backlight directory (sys/class/backlight) and prints it's contents
pub fn print_devices() {
    println!("{}", "Detected Devices".bold());
    fs::read_dir(BLDIR)
        .expect("Failed to read Backlight Directory")
        .for_each(|d| println!("{}", d.unwrap().file_name().to_string_lossy().green()));
}

/// This function prints helpful information about the CLI, such as available commands and examples.
pub fn print_help() {
    let title = "blight: A backlight utility for Linux that plays well with hybrid GPUs";
    let quote = "\"And man said, \'let there b-light\' and there was light.\" - Some Book 1:3";
    let commands = "\
opt: Optional, val: Value, dev: Device name

inc [opt val] [opt dev] -> increase by 2%
dec [opt val] [opt dev] -> decrease by 2%
set [val] [opt dev] -> set custom brightness value
sweep-up [opt val] [opt dev] -> smoothly increase by 10%
sweep-down [opt val] [opt dev] -> smoothly decrease by 10%
save [opt dev] -> save current brightness value to restore later
restore [opt dev] -> restore saved brightness value

setup -> installs udev rules and adds user to video group (run with sudo)
status [opt dev] -> backlight device status
list -> list all backlight devices
help -> displays help";

    let exampels = "\
Examples:
    sudo blight setup
    blight status intel_backlight
    blight inc (increases brightness by 2% - default step size)
    blight dec 10 (increases brightness by 10%)
    blight sweep-up 15 (smoothly increases brightness by 15%)
    blight inc 2 nvidia_0 (increases nvidia_0's brightness by 2%)";

    println!(
        "{}\n\n{}\n\n{}\n{}\n\n{}",
        title.blue().bold(),
        quote,
        "Commands".bold(),
        commands.green().bold(),
        exampels.bright_yellow()
    );
}

pub fn print_welcome() {
    println!(
        "{}\n",
        "blight: A backlight utility for Linux".blue().bold()
    );
    let cc = "\
inc -> increase brightness by 2%
dec -> decrease brightness by 2%
set -> set custom brightness value
sweep-up -> increase brightness smoothly by 10%
sweep-down -> decrease brightness smoothly by 10%
status -> show backlight device info
setup -> gain write permission to brightness file
";
    println!("{}\n{}", "Common Commands".bold(), cc.green().bold());
    println!(
        "{}",
        "Use `blight help' to display all commands and options".yellow()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    const TESTDIR: &str = "testbldir";

    #[test]
    fn detecting_device_nvidia() {
        clean_up();
        setup_test_env(&["nvidia_0", "generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_ok());
        assert_eq!(name.unwrap(), "nvidia_0");
        clean_up();
    }

    #[test]
    fn detecting_device_amd() {
        clean_up();
        setup_test_env(&["nvidia_0", "generic", "amdgpu_x"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_ok());
        assert_eq!(name.unwrap(), "amdgpu_x");
        clean_up();
    }

    #[test]
    fn detecting_device_acpi() {
        clean_up();
        setup_test_env(&["acpi_video0", "generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_ok());
        assert_eq!(name.unwrap(), "acpi_video0");
        clean_up();
    }

    #[test]
    fn detecting_device_fallback() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let name = Device::detect_device(TESTDIR);
        assert!(name.is_ok());
        assert_eq!(name.unwrap(), "generic");
        clean_up();
    }

    #[test]
    fn writing_value() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let d = Device {
            name: "generic".to_string(),
            max: 100,
            current: 50,
            device_dir: format!("{TESTDIR}/generic"),
        };
        d.write_value(100).unwrap();
        let r = fs::read_to_string(format!("{TESTDIR}/generic/brightness"))
            .expect("failed to read test backlight value");
        let res = r.trim();
        assert_eq!("100", res, "Result was {res}");
        clean_up();
    }

    #[test]
    fn current_value() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let current = Device::get_current(&format!("{TESTDIR}/generic")).unwrap();
        assert_eq!(current.to_string(), "50");
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
    fn write_permission_not_ok() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        fs::File::open(format!("{TESTDIR}/generic/brightness"))
            .and_then(|f| {
                let mut p = f.metadata().unwrap().permissions();
                p.set_readonly(true);
                f.set_permissions(p)
            })
            .unwrap();
        assert!(check_write_perm("generic", TESTDIR).is_err());
        clean_up();
    }

    fn setup_test_env(dirs: &[&str]) -> Result<(), Box<dyn Error>> {
        fs::create_dir(TESTDIR)?;
        for dir in dirs {
            fs::create_dir(format!("{TESTDIR}/{dir}"))?;
            fs::write(format!("{TESTDIR}/{dir}/brightness"), "50")?;
            fs::write(format!("{TESTDIR}/{dir}/max"), "100")?;
        }
        Ok(())
    }

    fn clean_up() {
        if fs::read_dir(".")
            .unwrap()
            .any(|dir| dir.unwrap().file_name().as_os_str() == "testbldir")
        {
            fs::remove_dir_all(TESTDIR).expect("Failed to clean up testing backlight directory.")
        }
    }
}
