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
//!
//! # Usage
//! ```ignore
//! use blight::{BlResult, Change, Device, Direction, Delay};
//!
//! fn main() -> BlResult<()> {
//!     // Using the helper functions
//!     blight::change_bl(5, Change::Regular, Direction::Inc, None)?; // Increases brightness by 5%
//!     blight::set_bl(50, Some("nvidia_0".into()))?; // Sets brightness value (not percentage) to 50
//!
//!     // Doing it manually
//!     let mut dev = Device::new(None)?;
//!     let new = dev.calculate_change(5, Direction::Dec); // safely calculate value to write
//!     dev.write_value(new)?; // decreases brightness by 5%
//!     dev.reload(); // reloads current brightness value (important)
//!     let new = dev.calculate_change(5, Direction::Inc);
//!     dev.sweep_write(new, Delay::default()); // smoothly increases brightness by 5%
//!     Ok(())
//! }
//! ```

#[cfg(not(target_os = "linux"))]
compile_error!("blight is only supported on linux");

use err::BlibError;
use std::{
    borrow::Cow,
    error::Error,
    fs::{self, File},
    io::prelude::*,
    ops::Deref,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

pub mod err;
pub use err::BlResult;

/// Linux backlight directory location. All backlight hardware devices appear here.
pub const BLDIR: &str = "/sys/class/backlight";
const CURRENT_FILE: &str = "brightness";
const MAX_FILE: &str = "max_brightness";

/// This enum is used to specify the direction in which the backlight should be changed in the [``change_bl``] and [``Device::calculate_change``] functions.
/// Inc -> Increase, Dec -> Decrease.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Direction {
    Inc,
    Dec,
}

/// This enum is used to specify the kind of backlight change to carry out while calling the [``change_bl``] function. \
///
/// Regular change applies the calculated change directly, whereas the sweep change occurs in incremental steps.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Change {
    #[default]
    Regular,
    Sweep,
}

/// A wrapper type for [``std::time::Duration``] used for specifying delay between each iteration of the loop in [``Device::sweep_write``].
///
/// Delay implements the Default trait, which always returns a Delay of 25ms (recommended delay for smooth brightness transisions).
/// The struct also provides the [``from_millis``][Delay::from_millis] constructor, if you'd like to set your own duration in milliseconds.
/// If you'd like to set the delay duration using units other than milliseconds, then you can use the From trait to create Delay using [Duration][std::time::Duration].
#[derive(Debug, Clone, Copy)]
pub struct Delay(Duration);

impl From<Duration> for Delay {
    fn from(value: Duration) -> Self {
        Self(value)
    }
}

impl Deref for Delay {
    type Target = Duration;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Delay {
    fn default() -> Self {
        Self(Duration::from_millis(25))
    }
}

impl Delay {
    pub fn from_millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }
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
    current: u32,
    max: u32,
    path: PathBuf, // Brightness file path
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
        let name = name
            .and_then(|n| Some(n))
            .unwrap_or(Cow::from(Self::detect_device(BLDIR)?));
        let mut path = Self::construct_path(BLDIR, &name);
        path.push(MAX_FILE);
        if !path.is_file() {
            return Err(BlibError::NoDeviceFound);
        };
        let max = Self::read_value(&path).map_err(|_| BlibError::ReadMax)?;
        path.set_file_name(CURRENT_FILE);
        let current = Self::read_value(&path).map_err(|_| BlibError::ReadCurrent)?;
        Ok(Device {
            current,
            max,
            path,
            name: name.into_owned(),
        })
    }

    fn construct_path(bldir: &str, device_name: &str) -> PathBuf {
        let mut buf = PathBuf::with_capacity(bldir.len() + device_name.len() + 1);
        buf.push(bldir);
        buf.push(device_name);
        buf
    }

    /// Returns the name of the current device
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the current brightness value of the current device
    pub fn current(&self) -> u32 {
        self.current
    }

    /// Returns the device's current brightness percentage (not rounded)
    pub fn current_percent(&self) -> f64 {
        (self.current as f64 / self.max as f64) * 100.
    }

    /// Returns the max brightness value of the current device
    pub fn max(&self) -> u32 {
        self.max
    }

    /// Returns absolute path that points to the device directory in `/sys/class/backlight`
    pub fn device_path(&self) -> PathBuf {
        let mut buf = self.path.to_path_buf();
        buf.pop();
        buf
    }

    fn detect_device(bldir: &str) -> BlResult<String> {
        let dirs: Vec<_> = fs::read_dir(bldir)
            .map_err(BlibError::ReadBlDir)?
            .filter_map(|d| d.ok().map(|d| d.file_name()))
            .collect();

        let (mut nv, mut ac): (Option<usize>, Option<usize>) = (None, None);

        for (i, entry) in dirs.iter().enumerate() {
            let name = entry.to_string_lossy();
            if name.contains("amd") || name.contains("intel") {
                return Ok(name.into_owned());
            } else if nv.is_none() && (name.contains("nvidia") | name.contains("nv")) {
                nv = Some(i);
            } else if ac.is_none() && name.contains("acpi") {
                ac = Some(i);
            }
        }

        let to_str = |i: usize| Ok(dirs[i].to_string_lossy().into_owned());

        if let Some(nv) = nv {
            to_str(nv)
        } else if let Some(ac) = ac {
            to_str(ac)
        } else if !dirs.is_empty() {
            to_str(0)
        } else {
            Err(BlibError::NoDeviceFound)
        }
    }

    fn open_bl_file(&self) -> Result<File, std::io::Error> {
        fs::File::options().write(true).open(&self.path)
    }

    /// Reloads current value for the current device in place.
    /// # Panics
    /// The method panics if the current value fails to be read from the filesystem.
    pub fn reload(&mut self) {
        self.current = Device::read_value(&self.path).unwrap();
    }

    fn read_value<P: AsRef<Path>>(path: P) -> Result<u32, Box<dyn Error>> {
        let max: u32 = fs::read_to_string(path)?.trim().parse()?;
        Ok(max)
    }

    /// Writes to the brightness file containted in /sys/class/backlight/ dir of the respective detected device, which will result in change of brightness if successful and if the chosen device is the correct one.
    /// # Errors
    /// - [``BlibError::WriteNewVal``] - on write failure
    pub fn write_value(&self, value: u32) -> BlResult<()> {
        if value > self.max {
            return Err(BlibError::ValueTooLarge {
                given: value,
                supported: self.max,
            });
        }
        let convert = |err| BlibError::WriteNewVal {
            err,
            dev: self.name.clone(),
        };
        write!(self.open_bl_file().map_err(convert)?, "{value}").map_err(convert)?;
        Ok(())
    }

    /// Writes to the brightness file starting from the current value in a loop, increasing 1% on each iteration with some delay until target value is reached,
    /// creating a smooth brightness transition.
    ///
    /// This method takes a target value, which can be computed with the help of [``Device::calculate_change``] or can also be manually entered.
    /// The delay between each iteration of the loop can be set using the [``Delay``] type, or the default can be used by calling [``Delay::default()``],
    /// which sets the delay of 25ms/iter (recommended).
    ///
    /// Note: Nothing is written to the brightness file if the provided value is the same as current brightness value or is larger than the max brightness value.
    /// # Example
    /// ```ignore
    /// Device::new(None)?
    ///     .sweep_write(50, Delay::default())?;
    /// ```
    /// # Errors
    /// Possible errors that can result from this function include:
    /// * [``BlibError::SweepError``]
    pub fn sweep_write(&self, value: u32, delay: Delay) -> Result<(), BlibError> {
        let mut bfile = self.open_bl_file().map_err(BlibError::SweepError)?;
        let mut rate = (f64::from(self.max) * 0.01) as u32;
        let mut current = self.current;
        let dir = if value > self.current {
            Direction::Inc
        } else {
            Direction::Dec
        };

        while !(current == value
            || value > self.max
            || (current == 0 && dir == Direction::Dec)
            || (current == self.max && dir == Direction::Inc))
        {
            match dir {
                Direction::Inc => {
                    if (current + rate) > value {
                        rate = value - current;
                    }
                    current += rate;
                }
                Direction::Dec => {
                    if rate > current {
                        rate = current;
                    } else if (current - rate) < value {
                        rate = current - value;
                    }
                    current -= rate;
                }
            }
            bfile.rewind().map_err(BlibError::SweepError)?;
            write!(bfile, "{current}").map_err(BlibError::SweepError)?;
            thread::sleep(*delay);
        }
        Ok(())
    }

    /// Calculates the new value to be written to the brightness file based on the provided step-size (percentage) and direction,
    /// using the current and max values of the detected GPU device. (Always guaranteed to be valid)
    ///
    /// For example, if the currecnt value is 10 and max is 100, and you want to increase it by 10% (step_size),
    /// the method will return 20, which can be directly written to the device.
    ///
    pub fn calculate_change(&self, step_size: u32, dir: Direction) -> u32 {
        let step: u32 = (self.max as f32 * (step_size as f32 / 100.0)) as u32;
        let change: u32 = match dir {
            Direction::Inc => self.current.saturating_add(step),
            Direction::Dec => self.current.saturating_sub(step),
        };

        if change > self.max {
            self.max
        } else {
            change
        }
    }
}

/// A helper function to change backlight based on step-size (percentage), [Change] type and [Direction].
///
/// Regular change uses [calculated change][Device::calculate_change] value based on step size and is applied instantly.
/// Sweep change on the other hand, occurs gradually, producing a fade or sweeping effect. (For more info, read about [``Device::sweep_write``])
/// > Note: No change is applied if the final calculated value is the same as current brightness value
/// # Errors
/// Possible errors that can result from this function include:
/// * All errors that can result from [``Device::new``]
/// * [``BlibError::WriteNewVal``]
pub fn change_bl(
    step_size: u32,
    ch: Change,
    dir: Direction,
    device_name: Option<Cow<str>>,
) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    let change = device.calculate_change(step_size, dir);
    if change != device.current {
        match ch {
            Change::Sweep => device.sweep_write(change, Delay::default())?,
            Change::Regular => device.write_value(change)?,
        }
    }
    Ok(())
}

/// A helper function which takes a brightness value and writes the value to the brightness file
/// as long as the given value falls under the min and max bounds of the detected backlight device and is different from the current value.
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
/// * [``BlibError::ValueTooLarge``]
pub fn set_bl(val: u32, device_name: Option<Cow<str>>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    if val != device.current {
        device.write_value(val)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    const TESTDIR: &str = "testbldir";

    #[test]
    fn path_construction() {
        assert_eq!(
            Device::construct_path(BLDIR, "generic"),
            PathBuf::from("/sys/class/backlight/generic")
        );
    }

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
        let name = "generic";
        setup_test_env(&[name]).unwrap();
        let d = Device {
            name: name.to_string(),
            max: 100,
            current: 50,
            path: test_path(name),
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
        let name = "generic";
        setup_test_env(&[name]).unwrap();
        let current = Device::read_value(test_path(name)).unwrap();
        assert_eq!(current, 50);
        clean_up();
    }

    #[test]
    fn current_percent() {
        let device = Device {
            name: "".into(),
            current: 5,
            max: 255,
            path: test_path(""),
        };
        assert_eq!(device.current_percent().round(), 2.0);
    }

    #[test]
    fn inc_calculation() {
        let d = Device {
            name: String::new(),
            current: 10,
            max: 100,
            path: test_path(""),
        };
        let ch = d.calculate_change(10, Direction::Inc);
        assert_eq!(ch, 20);
    }

    #[test]
    fn dec_calculation() {
        let d = Device {
            name: String::new(),
            current: 30,
            max: 100,
            path: test_path(""),
        };
        let ch = d.calculate_change(10, Direction::Dec);
        assert_eq!(ch, 20);
    }

    #[test]
    fn inc_calculation_max() {
        let d = Device {
            name: String::new(),
            current: 90,
            max: 100,
            path: test_path(""),
        };
        let ch = d.calculate_change(20, Direction::Inc);
        assert_eq!(ch, 100);
    }

    #[test]
    fn dec_calculation_max() {
        let d = Device {
            name: String::new(),
            current: 10,
            max: 100,
            path: test_path(""),
        };
        let ch = d.calculate_change(20, Direction::Dec);
        assert_eq!(ch, 0);
    }

    #[test]
    fn sweeping() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let mut d = test_device("generic");
        d.sweep_write(100, Delay::default()).unwrap();
        d.reload();
        assert_eq!(d.current, 100);
        d.sweep_write(0, Delay::default()).unwrap();
        d.reload();
        assert_eq!(d.current, 0);
        clean_up();
    }

    #[test]
    fn sweep_bounds() {
        clean_up();
        setup_test_env(&["generic"]).unwrap();
        let mut d = test_device("generic");
        d.write_value(0).unwrap();
        d.sweep_write(u32::MAX, Delay::default()).unwrap();
        d.reload();
        assert_eq!(d.current, 0);
        clean_up();
    }

    fn setup_test_env(dirs: &[&str]) -> Result<(), Box<dyn Error>> {
        fs::create_dir(TESTDIR)?;
        for dir in dirs {
            fs::create_dir(format!("{TESTDIR}/{dir}"))?;
            fs::write(format!("{TESTDIR}/{dir}/brightness"), "50")?;
            fs::write(format!("{TESTDIR}/{dir}/max_brightness"), "100")?;
        }
        Ok(())
    }

    fn test_device(name: &str) -> Device {
        Device {
            name: name.into(),
            current: 50,
            max: 100,
            path: test_path(name),
        }
    }

    fn test_path(name: &str) -> PathBuf {
        let mut path = Device::construct_path(TESTDIR, name);
        path.push(CURRENT_FILE);
        path
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
