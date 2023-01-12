#![warn(clippy::pedantic)]
//! # About
//! blight is primarily a CLI backlight utility for Linux which is focused on providing hassle-free backlight control.
//! However, the parts which blight relies on to make backlight changes, are also exposed through the library aspect of this crate, which can be used like any other Rust library
//! by using the command `cargo add blight` in your Rust project. The CLI utility, on the other hand, can be installed by running `cargo install blight`.
//! This documentation only covers the library aspect, for CLI related docs, visit the project's [Github repo](https://github.com/voltaireNoir/blight).
//!
//! Two features of blight that standout:
//! 1. Prioritizing device detection in this order: iGPU>dGPU>ACPI>Fallback device.
//! 2. Smooth backlight change by writing in increments/decrements of 1 with a few milliseconds of delay. \
//! > **IMPORTANT:** You need write permission for the file `/sys/class/backlight/{your_device}/brightness` to change brightness.
//! > The CLI utility comes with a helper script that let's you gain access to the brightness file (which may not always work), which you can run by using the command `sudo blight setup`.
//! > If you're only using blight as a dependency, you can read about gaining file permissions [here](https://wiki.archlinux.org/title/Backlight#ACPI).

use err::BlibError;
use std::{borrow::Cow, fs, path::PathBuf, thread, time::Duration};

pub mod err;
pub use err::BlResult;

/// Linux backlight directory location. All backlight hardware devices appear here.
pub const BLDIR: &str = "/sys/class/backlight";

/// This enum is used to specify the direction in which the backlight should be changed in the [change_bl] and [sweep] functions.
/// Inc -> Increase, Dec -> Decrease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Inc,
    Dec,
}

/// This enum is used to specify the kind of backlight change to carry out while calling the [change_bl] function. \
///
/// Regular change applies the calculated change directly, whereas the sweep change occurs in incremental steps.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    #[default]
    Regular,
    Sweep,
}

/// An abstraction of a backlight device containing a name, current and max backlight values, and some related functionality.
///
/// A Device instance is created by using the [constructor][Device::new], values are read from /sys/class/backlight/ directory based on the detected GPU device.
/// The constructor uses the default detection method unless a device name is passed as an argument. Based on whether a device is detected, the constructor will either return Some(Device) or None,
/// if no device is detected. \
/// This is how the devices are priorirized: ``AmdGPU or Intel > Nvdia > ACPI > Any Fallback Device``, unless a device name is passed as an argument.
/// # Examples
/// ```ignore
/// let bl = Device::new(None)?;
/// bl.write_value(50)?;
/// ```
#[derive(Debug, Clone)]
pub struct Device {
    name: String,
    current: u16,
    max: u16,
    device_dir: String,
}

impl Device {
    /// Constructor for creating a [Device] instance.
    ///
    /// By default, it uses the priority detection method unless ``Some(device_name)`` is passed as an argument, then that name will be used to create an instance of that device if it exists.
    /// # Errors
    /// Possible errors that can result from this function include:
    /// * [``BlibError::NoDeviceFound``]
    /// * [``BlibError::ReadBlDir``]
    /// * [``BlibError::ReadCurrent``]
    /// * [``BlibError::ReadMax``]
    pub fn new(name: Option<Cow<str>>) -> BlResult<Device> {
        let name = if let Some(n) = name {
            PathBuf::from(format!("{BLDIR}/{n}/brightness"))
                .is_file()
                .then_some(n)
                .ok_or(BlibError::NoDeviceFound)?
        } else {
            Self::detect_device(BLDIR)?
        };
        let device = Self::load(&name)?;
        Ok(device)
    }

    /// Returns the name of the current device
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns the current brightness value of the current device
    pub fn current(&self) -> u16 {
        self.current
    }

    /// Returns the max brightness value of the current device
    pub fn max(&self) -> u16 {
        self.max
    }

    fn load(name: &str) -> BlResult<Device> {
        let device_dir = format!("{BLDIR}/{name}");
        Ok(Device {
            current: Self::get_current(&device_dir)?,
            max: Self::get_max(&device_dir)?,
            device_dir,
            name: name.into(),
        })
    }

    fn detect_device(bldir: &str) -> BlResult<Cow<str>> {
        let dirs = fs::read_dir(bldir)
            .map_err(BlibError::ReadBlDir)?
            .filter_map(|d| d.ok().map(|d| d.file_name()));

        let (mut nv, mut ac, mut fl): (Option<String>, Option<String>, Option<String>) =
            (None, None, None);

        for entry in dirs {
            let name = entry.to_string_lossy();
            if name.contains("amdgpu") || name.contains("intel") {
                return Ok(name.into_owned().into());
            } else if nv.is_none() && (name.contains("nvidia") | name.contains("nv")) {
                nv = Some(name.into());
                break;
            } else if ac.is_none() && name.contains("acpi") {
                ac = Some(name.into());
                break;
            }
            fl = Some(name.into());
        }

        if let Some(nv) = nv {
            Ok(nv.into())
        } else if let Some(ac) = ac {
            Ok(ac.into())
        } else if let Some(fl) = fl {
            Ok(fl.into())
        } else {
            Err(BlibError::NoDeviceFound)
        }
    }

    /// Reloads max and current values for the current device in place.
    /// # Panics
    /// The method panics if either max or current values fail to be read from the filesystem.
    pub fn reload(&mut self) {
        let dd = &self.device_dir;
        *self = Device {
            max: Device::get_max(dd).unwrap(),
            current: Device::get_current(dd).unwrap(),
            name: std::mem::take(&mut self.name),
            device_dir: std::mem::take(&mut self.device_dir),
        };
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
    /// Writes to the brightness file containted in /sys/class/backlight/ dir of the respective detected device, which will result in change of brightness if successful and if the chosen device is the correct one.
    pub fn write_value(&self, value: u16) -> BlResult<()> {
        fs::write(
            format!("{}/brightness", self.device_dir),
            format!("{value}"),
        )
        .map_err(|err| BlibError::WriteNewVal {
            err,
            dev: self.name.clone(),
        })?;

        Ok(())
    }
}

/// Calculates the new value to be written to the brightness file based on the provided step-size (percentage) and direction,
/// for the given current and max values of the detected GPU device.
///
/// This function is used internally in [change_bl] function to calculate the final change value based on the given percentage.
#[must_use]
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

/// A helper function to change backlight based on step-size (percentage), [Change] type and [Direction].
///
/// Regular change uses [calculated change][calculate_change] value based on step size and is applied instantly.
/// Sweep change on the other hand, occurs gradually, producing a fade or sweeping effect. (For more info, read about [sweep])
/// # Errors
/// Possible errors that can result from this function include:
/// * All errors that can result from [``Device::new``]
/// * [``BlibError::WriteNewVal``]
pub fn change_bl(
    step_size: u16,
    ch: Change,
    dir: Direction,
    device_name: Option<Cow<str>>,
) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    let change = calculate_change(device.current, device.max, step_size, dir);
    if change != device.current {
        match ch {
            Change::Sweep => sweep(&device, change, dir)?,
            Change::Regular => device.write_value(change)?,
        }
    }
    Ok(())
}

/// A helper function which takes a brightness value and writes the value to the brightness file
/// as long as the given value falls under the min and max bounds of the detected backlight device.
///
/// *Note: Unlike [change_bl], this function does not calculate any change, it writes the given value directly.*
/// # Examples
/// ```ignore
/// blight::set_bl(15, None)?;
/// ```
/// ```ignore
/// blight::set_bl(50, Some("nvidia_0".into()))?;
/// ````
/// # Errors
/// Possible errors that can result from this function include:
/// * All errors that can result from [``Device::new``]
/// * [``BlibError::WriteNewVal``]
pub fn set_bl(val: u16, device_name: Option<Cow<str>>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    if val <= device.max && val != device.current {
        device.write_value(val)?;
    }
    Ok(())
}

/// This function takes a borrow of a Device instance, a [calculated change][calculate_change] value and the [Direction].
///
/// It writes to the brightness file in an increment of 1 on each loop until change value is reached.
/// Each loop has a delay of 25ms, to produce to a smooth sweeping effect when executed.
/// # Errors
/// Possible errors that can result from this function include:
/// * [``BlibError::WriteNewVal``]
pub fn sweep(device: &Device, change: u16, dir: Direction) -> Result<(), BlibError> {
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
                val -= 1;
            }
        }
    }
    Ok(())
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
        let ch = calculate_change(10, 100, 10, Direction::Inc);
        assert_eq!(ch, 20);
    }

    #[test]
    fn dec_calculation() {
        let ch = calculate_change(30, 100, 10, Direction::Dec);
        assert_eq!(ch, 20);
    }

    #[test]
    fn inc_calculation_max() {
        let ch = calculate_change(90, 100, 20, Direction::Inc);
        assert_eq!(ch, 100);
    }

    #[test]
    fn dec_calculation_max() {
        let ch = calculate_change(10, 100, 20, Direction::Dec);
        assert_eq!(ch, 0);
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
            fs::remove_dir_all(TESTDIR).expect("Failed to clean up testing backlight directory.");
        }
    }
}
