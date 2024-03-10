#![warn(clippy::pedantic)]
//! # About
//! blight is primarily a CLI backlight utility for Linux which is focused on providing hassle-free backlight control.
//! However, the parts which blight relies on to make backlight changes, are also exposed through the library aspect of this crate, which can be used like any other Rust library
//! by using the command `cargo add blight` in your Rust project. The CLI utility, on the other hand, can be installed by running `cargo install blight`.
//! This documentation only covers the library aspect, for CLI related docs, visit the project's [Github repo](https://github.com/voltaireNoir/blight).
//!
//! The latest version of the libary now supports [controlling LEDs][led] using the `/sys/class/leds` interface.
//!
//! Three features of blight that standout:
//! 1. Prioritizing backlight device detection in this order: iGPU>dGPU>ACPI>Fallback device.
//! 2. Smooth dimming by writing in increments/decrements of 1 with a few milliseconds of delay ([sweep write][Light::sweep_write]).
//! 3. The library has zero external dependencies.
//!
//! > **IMPORTANT:** You need write permission for the file `/sys/class/backlight/{your_device}/brightness` to change brightness.
//! > The CLI utility comes with a helper script that let's you gain access to the brightness file (which may not always work), which you can run by using the command `sudo blight setup`.
//! > If you're only using blight as a dependency, you can read about gaining file permissions [here](https://wiki.archlinux.org/title/Backlight#ACPI).
//!
//! **For LED specific documentation and usage, see [led module][led].**
//!
//! # Usage
//! ```no_run
//! use blight::{Change, Device, Direction, Delay, Light};
//!
//! fn main() -> blight::Result<()> {
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

use std::{
    borrow::Cow,
    fs::{self, File},
    io::prelude::*,
    ops::Deref,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

pub mod err;
pub mod led;
pub use err::{Error, ErrorKind, Result};

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
/// ```no_run
/// use blight::{Device, Light};
///
/// fn main() -> blight::Result<()> {
///   let mut bl = Device::new(None)?;
///   println!(
///     "Backlight device: {}, current brightness: {}, max brightness: {}",
///     bl.name(),
///     bl.current(),
///     bl.max()
///   );
///   bl.write_value(50)?;
///   bl.try_reload()?;
///   Ok(())
/// }
/// ```
#[derive(Debug)]
pub struct Device {
    name: String,
    current: u32,
    max: u32,
    path: PathBuf,
    brightness: File,
}

impl Device {
    /// Constructor for creating a [Device] instance.
    ///
    /// By default, it uses the priority detection method unless ``Some(device_name)`` is passed as an argument, then that name will be used to create an instance of that device if it exists.
    /// # Errors
    /// Possible errors that can result from this function include:
    /// * [``ErrorKind::NotFound``]
    /// * [``ErrorKind::ReadDir``]
    /// * [``ErrorKind::ReadCurrent``]
    /// * [``ErrorKind::ReadMax``]
    pub fn new(name: Option<Cow<str>>) -> Result<Device> {
        let name = match name {
            Some(val) => val,
            None => Self::detect_device(BLDIR)?.into(),
        };
        let info = utils::read_info(BLDIR, &name)?;
        Ok(Device {
            current: info.current,
            max: info.max,
            path: info.path,
            name: name.into_owned(),
            brightness: info.brightness,
        })
    }

    fn detect_device(bldir: &str) -> Result<String> {
        let dirs: Vec<_> = fs::read_dir(bldir)
            .map_err(|err| Error::from(ErrorKind::ReadDir { dir: BLDIR }).with_source(err))?
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
            Err(ErrorKind::NotFound.into())
        }
    }
}

impl private::Sealed for Device {}

impl Light for Device {
    type Value = u32;

    fn name(&self) -> &str {
        &self.name
    }

    fn current(&self) -> Self::Value {
        self.current
    }

    fn max(&self) -> Self::Value {
        self.max
    }

    fn set_current(&mut self, _: private::Internal, current: Self::Value) {
        self.current = current;
    }

    fn brightness_file(&mut self, _: private::Internal) -> &mut File {
        &mut self.brightness
    }

    /// Returns absolute path that points to the device directory in `/sys/class/backlight`
    fn device_path(&self) -> &Path {
        &self.path
    }
}

impl Dimmable for Device {}
impl Toggleable for Device {}

mod private {
    #[doc(hidden)]
    pub struct Internal;
    #[doc(hidden)]
    pub trait Sealed {}
}

/// Marker trait to signify that a backlight device or an LED is dimmable
pub trait Dimmable: Toggleable + private::Sealed {}

/// Marker trait to signify that an LED can only be toggled on/off
pub trait Toggleable: private::Sealed {}

/// The interface for controlling both backlight and LED devices
pub trait Light: private::Sealed {
    type Value: Into<u32> + TryFrom<u32> + PartialEq + Default;

    /// Name of the backlight/LED device
    fn name(&self) -> &str;

    /// Returns the current brightness value of the current device
    fn current(&self) -> Self::Value;

    /// Returns the max brightness value of the current device
    fn max(&self) -> Self::Value;

    /// Absolute path to the device interface in `/sys/class/..` directory
    fn device_path(&self) -> &Path;

    #[doc(hidden)]
    fn set_current(&mut self, _: private::Internal, current: Self::Value);
    #[doc(hidden)]
    fn brightness_file(&mut self, _: private::Internal) -> &mut File;

    /// Returns the device's current brightness percentage (not rounded)
    fn current_percent(&self) -> f64
    where
        Self: Dimmable,
    {
        let (current, max): (u32, u32) = (self.current().into(), self.max().into());
        (f64::from(current) / f64::from(max)) * 100.
    }

    /// Reloads current brightness value for the device
    ///
    /// Use the fallible [`Light::try_reload`] method if you want to handle the error and avoid causing panic at runtime.
    ///
    /// # Panics
    /// The method panics if the current value fails to be read from the filesystem.
    fn reload(&mut self) {
        self.try_reload()
            .expect("Failed to read current brightness value");
    }

    /// Reloads current brightness value for the device
    fn try_reload(&mut self) -> Result<()> {
        let current = utils::read_ascii_u32(self.brightness_file(private::Internal))
            .map_err(|err| Error::from(ErrorKind::ReadCurrent).with_source(err))?;
        self.set_current(
            private::Internal,
            Self::Value::try_from(current).unwrap_or_default(),
        );
        Ok(())
    }

    /// Write the given value to the brightness file of the device
    ///
    /// **Note: This does not update the current brightness value in the type.
    /// To update the value, call [`Light::reload`] or [`Light::try_reload`].**
    ///
    /// # Errors
    /// - [``ErrorKind::ValueTooLarge``] - if provided value is larger than the supported value
    /// - [``ErrorKind::WriteValue``] - on write failure
    fn write_value(&mut self, value: Self::Value) -> Result<()> {
        let (value, max): (u32, u32) = (value.into(), self.max().into());
        if value > max {
            return Err(ErrorKind::ValueTooLarge {
                given: value,
                supported: max,
            }
            .into());
        }
        let name = self.name().into();
        let convert = |err| Error::from(ErrorKind::WriteValue { device: name }).with_source(err);
        let file = self.brightness_file(private::Internal);
        write!(file, "{value}",).map_err(convert.clone())?;
        file.rewind().map_err(convert)?;
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
    /// ```no_run
    /// # use blight::{Device, Light, Delay};
    /// # fn main() -> blight::Result<()> {
    /// Device::new(None)?
    ///    .sweep_write(50, Delay::default())?;
    /// # Ok(())
    /// # }
    /// ```
    /// # Errors
    /// Possible errors that can result from this function include:
    /// * [``ErrorKind::SweepError``]
    fn sweep_write(&mut self, value: Self::Value, delay: Delay) -> Result<()>
    where
        Self: Dimmable,
    {
        let (mut current, value, max): (u32, u32, u32) =
            (self.current().into(), value.into(), self.max().into());
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let mut rate = (f64::from(max) * 0.01) as u32;
        let dir = if value > current {
            Direction::Inc
        } else {
            Direction::Dec
        };
        let bfile = self.brightness_file(private::Internal);
        let map_err = |err| Error::from(ErrorKind::SweepError).with_source(err);
        while !(current == value
            || value > max
            || (current == 0 && dir == Direction::Dec)
            || (current == max && dir == Direction::Inc))
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
            bfile.rewind().map_err(map_err)?;
            write!(bfile, "{current}").map_err(map_err)?;
            thread::sleep(*delay);
        }
        bfile.rewind().map_err(map_err)?;
        Ok(())
    }

    /// Calculates the new value to be written to the brightness file based on the provided step-size (percentage) and direction,
    /// using the current and max values of the backlight/LED device. (Always guaranteed to be valid)
    ///
    /// For example, if the current value is 10 and max is 100, and you want to increase it by 10% (`step_size`),
    /// the method will return 20, which can be directly written to the device.
    fn calculate_change(&self, step_size: Self::Value, dir: Direction) -> Self::Value
    where
        Self: Dimmable,
    {
        let (current, max, step_size): (u32, u32, u32) =
            (self.current().into(), self.max().into(), step_size.into());
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let step: u32 = (f64::from(max) * (f64::from(step_size) / 100.0)) as u32;
        let change = match dir {
            Direction::Inc => current.saturating_add(step),
            Direction::Dec => current.saturating_sub(step),
        }
        .min(max); // return max if calculated value is > max
        Self::Value::try_from(change).unwrap_or_default()
    }

    /// Toggle between `0` and the [`max`](Light::max) brightness value of the device
    ///
    /// This method is mainly intended for toggling LEDs on/off.
    ///
    /// ## Errors
    /// - All possible errors that can occur when calling [`Light::write_value`]
    fn toggle(&mut self) -> Result<()>
    where
        Self: Toggleable,
    {
        let value = if self.current() == self.max() {
            Self::Value::default()
        } else {
            self.max()
        };
        self.write_value(value)
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
/// * [``ErrorKind::WriteValue``]
pub fn change_bl(
    step_size: u32,
    ch: Change,
    dir: Direction,
    device_name: Option<Cow<str>>,
) -> crate::Result<()> {
    let mut device = Device::new(device_name)?;

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
/// ```no_run
/// # fn main() -> blight::Result<()> {
/// blight::set_bl(15, None)?;
/// # Ok(())
/// # }
/// ```
/// ```no_run
/// # fn main() -> blight::Result<()> {
/// blight::set_bl(50, Some("nvidia_0".into()))?;
/// # Ok(())
/// # }
/// ```
/// # Errors
/// Possible errors that can result from this function include:
/// * All errors that can result from [``Device::new``]
/// * All errors that can result from [`Light::write_value`]
pub fn set_bl(val: u32, device_name: Option<Cow<str>>) -> Result<()> {
    let mut device = Device::new(device_name)?;
    if val != device.current {
        device.write_value(val)?;
    }
    Ok(())
}

mod utils {
    use super::{Error, ErrorKind, Result};
    use std::{
        fs::File,
        io::{Read, Seek},
        path::PathBuf,
    };

    use crate::{CURRENT_FILE, MAX_FILE};

    pub(crate) struct Info {
        pub(crate) current: u32,
        pub(crate) max: u32,
        pub(crate) brightness: File,
        pub(crate) path: PathBuf,
    }

    /// Read all the necessary info from the backlight/led interface directory
    pub(crate) fn read_info(dir: &str, interface: &str) -> Result<Info> {
        let mut path = construct_path(dir, interface);
        if !path.is_dir() {
            return Err(ErrorKind::NotFound.into());
        }
        let map_err = |kind| |err| Error::from(kind).with_source(err);
        // Read max brightness value
        let max = {
            let err = map_err(ErrorKind::ReadMax);
            path.push(MAX_FILE);
            let mut max_file = File::open(&path).map_err(err.clone())?;
            read_ascii_u32(&mut max_file).map_err(err)?
        };
        // Read current brightness value
        let (current, brightness) = {
            let err = map_err(ErrorKind::ReadCurrent);
            path.set_file_name(CURRENT_FILE);
            let mut current_file = File::options()
                .read(true)
                .write(true)
                .open(&path)
                .map_err(err.clone())?;
            let current = read_ascii_u32(&mut current_file).map_err(err)?;
            (current, current_file)
        };
        // Remove file name so that path points to the parent dir again
        path.pop();
        Ok(Info {
            current,
            max,
            brightness,
            path,
        })
    }

    /// Try to read the ASCII contents from the source and convert them into a u32
    ///
    /// Note: this function resets the cursor of the source to the start after reading from it.
    ///
    /// ## Important
    /// The implementation assumes that the source contains valid ASCII bytes which may or may not contain
    /// a newline character at the end and that when converted to an integer, will result in a value that is `0` <= `value` <= `u32::MAX`
    pub(crate) fn read_ascii_u32<S: Read + Seek>(mut source: S) -> std::io::Result<u32> {
        let mut buf = [0; 10]; // large enough to hold ASCII string of u32::MAX
        let read = source.read(&mut buf)?;
        source.rewind()?;
        if read == 0 || read > buf.len() {
            return Err(std::io::Error::other(format!(
                "read too few or too many bytes: {read} bytes into buf of len {}",
                buf.len()
            )));
        }
        let mut readi = read - 1;
        if buf[readi] as char == '\n' {
            readi -= 1;
        }
        #[allow(clippy::cast_possible_truncation)]
        let (mut value, mut place) = (0, 10u32.pow(readi as _));
        #[allow(clippy::char_lit_as_u8)]
        for v in &buf[..=readi] {
            value += u32::from(v - '0' as u8) * place;
            place /= 10;
        }
        Ok(value)
    }

    pub(crate) fn construct_path(dir: &str, device_name: &str) -> PathBuf {
        let mut buf = PathBuf::with_capacity(dir.len() + device_name.len() + 1);
        buf.push(dir);
        buf.push(device_name);
        buf
    }
}

// NOTE: tests that read from and write to the disk should not be run in parallel
// run with `cargo test -- --test-threads 1`
#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) const BLDIR: &str = "testbldir";

    struct MockInterface(utils::Info);

    impl MockInterface {
        /// Reads from disk, for testing reads and writes
        fn new(name: &str) -> Self {
            Self(utils::read_info(BLDIR, name).expect("failed to initialize mock interface"))
        }
        /// Dummy instance with specified values that points to an empty temp file
        ///
        /// For testing non-IO operations
        fn dummy(current: u32, max: u32) -> Self {
            Self(utils::Info {
                current,
                max,
                brightness: File::create("/tmp/dummy.blight").expect("failed to open file"),
                path: PathBuf::new(),
            })
        }
    }

    impl private::Sealed for MockInterface {}
    impl Toggleable for MockInterface {}
    impl Dimmable for MockInterface {}
    impl Light for MockInterface {
        type Value = u32;

        fn name(&self) -> &str {
            self.0.path.file_name().unwrap().to_str().unwrap()
        }

        fn device_path(&self) -> &Path {
            &self.0.path
        }

        fn current(&self) -> Self::Value {
            self.0.current
        }

        fn max(&self) -> Self::Value {
            self.0.max
        }

        fn set_current(&mut self, _: private::Internal, current: Self::Value) {
            self.0.current = current;
        }

        fn brightness_file(&mut self, _: private::Internal) -> &mut File {
            &mut self.0.brightness
        }
    }

    #[test]
    fn reading_info() {
        let name = "generic";
        let test = || {
            let utils::Info {
                current, max, path, ..
            } = utils::read_info(BLDIR, name).expect("failed to read info");

            assert_eq!(current, 50, "incorrect current value");
            assert_eq!(max, 100, "incorrect max value");
            assert_eq!(
                &path,
                <str as AsRef<Path>>::as_ref(&format!("{BLDIR}/{name}")),
                "incorrect interface dir path"
            );
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn ascii_conversion() {
        let cases: [(&[u8], u32); 4] = [
            (b"123\n".as_slice(), 123),
            (b"999\n".as_slice(), 999),
            (b"0".as_slice(), 0),
            (b"4294967295".as_slice(), u32::MAX),
        ];
        for (i, (case, expected)) in cases.into_iter().enumerate() {
            let value = utils::read_ascii_u32(std::io::Cursor::new(case))
                .expect("failed to convert ASCII bytes to u32");
            assert_eq!(value, expected, "case {i} failed");
        }
    }

    #[test]
    fn path_construction() {
        assert_eq!(
            utils::construct_path(BLDIR, "generic"),
            PathBuf::from(&format!("{BLDIR}/generic"))
        );
    }

    #[test]
    fn detecting_device_nvidia() {
        let interfaces = ["nvidia_0", "generic"];
        let test = || {
            let name = Device::detect_device(BLDIR);
            assert!(name.is_ok());
            assert_eq!(name.unwrap(), "nvidia_0");
        };
        with_test_env(&interfaces, test);
    }

    #[test]
    fn detecting_device_amd() {
        let interfaces = ["nvidia_0", "generic", "amdgpu_x"];
        let test = || {
            let name = Device::detect_device(BLDIR);
            assert!(name.is_ok());
            assert_eq!(name.unwrap(), "amdgpu_x");
        };
        with_test_env(&interfaces, test);
    }

    #[test]
    fn detecting_device_acpi() {
        let interfaces = ["acpi_video0", "generic"];
        let test = || {
            let name = Device::detect_device(BLDIR);
            assert!(name.is_ok());
            assert_eq!(name.unwrap(), "acpi_video0");
        };
        with_test_env(&interfaces, test);
    }

    #[test]
    fn detecting_device_fallback() {
        let expected = "generic";
        let test = || {
            let name = Device::detect_device(BLDIR);
            assert!(name.is_ok());
            assert_eq!(name.unwrap(), expected);
        };
        with_test_env(&[expected], test);
    }

    #[test]
    fn toggle() {
        let name = "generic";
        let test = || {
            let mut d = MockInterface::new(name);
            d.write_value(0).expect("failed to write value");
            d.reload();
            assert_ne!(d.current(), d.max());
            d.toggle().expect("failed to toggle on/off");
            d.reload();
            assert_eq!(d.current(), d.max());
            d.toggle().expect("failed to toggle on/off");
            d.reload();
            assert_eq!(d.current(), <MockInterface as Light>::Value::default());
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn reload() {
        let name = "generic";
        let test = || {
            let mut d = MockInterface::new(name);
            let test_value = 12345;
            assert_ne!(d.current(), test_value);

            write!(&mut d.0.brightness, "{test_value}")
                .expect("failed to write value to brightness file");
            d.0.brightness
                .rewind()
                .expect("failed to reset brightness file cursor");

            d.reload();
            assert_eq!(d.current(), test_value);
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn write_value() {
        let name = "generic";
        let test = || {
            let mut d = MockInterface::new(name);
            d.write_value(100).unwrap();
            let res = fs::read_to_string(format!("{BLDIR}/generic/brightness"))
                .expect("failed to read test backlight value");
            assert_eq!(res.trim(), "100", "Result was {res}");
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn read_value() {
        let name = "generic";
        let test = || {
            let (_, mut file) = open_current_file(name);
            assert_eq!(50, utils::read_ascii_u32(&mut file).unwrap());
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn current_percent() {
        let percent = MockInterface::dummy(5, 255).current_percent().round();
        assert_eq!(percent, 2.0);
    }

    #[test]
    fn inc_calculation() {
        let d = MockInterface::dummy(10, 100);
        let ch = d.calculate_change(10, Direction::Inc);
        assert_eq!(ch, 20);
    }

    #[test]
    fn dec_calculation() {
        let d = MockInterface::dummy(30, 100);
        let ch = d.calculate_change(10, Direction::Dec);
        assert_eq!(ch, 20);
    }

    #[test]
    fn inc_calculation_max() {
        let d = MockInterface::dummy(90, 100);
        let ch = d.calculate_change(20, Direction::Inc);
        assert_eq!(ch, 100);
    }

    #[test]
    fn dec_calculation_max() {
        let d = MockInterface::dummy(10, 100);
        let ch = d.calculate_change(20, Direction::Dec);
        assert_eq!(ch, 0);
    }

    #[test]
    fn sweeping() {
        let name = "generic";
        let test = || {
            let mut d = MockInterface::new(name);
            d.sweep_write(100, Delay::default()).unwrap();
            d.reload();
            assert_eq!(d.current(), 100);
            d.sweep_write(0, Delay::default()).unwrap();
            d.reload();
            assert_eq!(d.current(), 0);
        };
        with_test_env(&[name], test);
    }

    #[test]
    fn sweep_bounds() {
        let name = "generic";
        let test = || {
            let mut d = MockInterface::new(name);
            d.write_value(0).unwrap();
            d.sweep_write(u32::MAX, Delay::default()).unwrap();
            d.reload();
            assert_eq!(d.current(), 0);
        };
        with_test_env(&[name], test);
    }

    pub(crate) fn with_test_env(dirs: &[&str], test: impl FnOnce()) {
        clean_up();
        setup_test_env(dirs, 50, 100);
        test();
        clean_up();
    }

    pub(crate) fn setup_test_env(dirs: &[&str], current: u32, max: u32) {
        let inner = || -> std::io::Result<()> {
            fs::create_dir(BLDIR)?;
            for dir in dirs {
                fs::create_dir(format!("{BLDIR}/{dir}"))?;
                fs::write(format!("{BLDIR}/{dir}/brightness"), current.to_string())?;
                fs::write(format!("{BLDIR}/{dir}/max_brightness"), max.to_string())?;
            }
            Ok(())
        };
        inner().expect("failed to set up test env");
    }

    fn open_current_file(name: &str) -> (PathBuf, File) {
        let mut path = utils::construct_path(BLDIR, name);
        path.push(CURRENT_FILE);
        let file = File::open(&path).expect("failed to open test current brightness file");
        (path, file)
    }

    pub(crate) fn clean_up() {
        if <str as AsRef<Path>>::as_ref(BLDIR).is_dir() {
            fs::remove_dir_all(BLDIR).expect("Failed to clean up testing backlight directory.");
        }
    }
}
