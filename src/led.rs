//! Abstractions and functions for controlling LEDs using the `/sys/class/leds` Linux interface
//!
//! Once an instance of the [`Led`] type has been initialized, the interface for controlling it
//! is identical to a backlight device. However, LEDs that only support `0` and `1` as valid
//! brightness values are considered as `non-dimmable` and hence any functionality related to
//! dimming is not available on those types. This is statically enforced by the type system.
//!
//! # Usage
//! ```no_run
//! use blight::{Delay, Light, led};
//!
//! fn main() -> blight::Result<()> {
//!     led::set_led_state("target::led::name", true)?; // Turn an LED on
//!
//!     let leds = led::led_names()?; // Read all LED names from `/sys/class/leds`
//!     // Print all the info determined just by parsing the names of the LEDs
//!     for led in &leds {
//!         println!(
//!             "Full name: {}, parsed name: {:?}, color: {:?}, function: {:?}",
//!             led.raw_name(),
//!             led.parsed_name(),
//!             led.color(),
//!             led.function()
//!         );
//!     }
//!     // Find Capslock LED and alter its state
//!     if let Some(caps) = leds
//!         .into_iter()
//!         .find(|n| n.function() == led::Function::Capslock)
//!     {
//!         // This is the same as `led::Led::from_name(caps)?`
//!         match caps.initialize()? {
//!             // Dimmable LEDs offer more functionality (same as a backlight device)
//!             // Note: Capslock is almost always non-dimmable, this line of code is only
//!             // to illustrate the general usage of the interface.
//!             led::LedType::Dimmable(mut led) => led.sweep_write(0, Delay::default()),
//!             // Non-dimmable LEDs support only 0 and 1 as their brightness values,
//!             // and can only be turned on/off (using `toggle` or `write_value` methods)
//!             led::LedType::NonDimmable(mut led) => led.toggle(),
//!         }?
//!     }
//!     // Initialize a known LED by its name
//!     match led::Led::new("platform::kbd_backlight".into())? {
//!         led::LedType::Dimmable(mut led) => led.write_value(led.max()), // set it to max brightness
//!         led::LedType::NonDimmable(mut led) => led.write_value(0),      // turn off the LED
//!     }?;
//!     Ok(())
//! }
//! ```
use std::{
    borrow::Cow,
    fs::File,
    marker::PhantomData,
    path::{Path, PathBuf},
    str::FromStr,
};

use crate::{
    err::{Error, ErrorKind},
    private, utils, Light,
};

/// Linux LED interface directory
#[cfg(not(test))]
pub const LEDDIR: &str = "/sys/class/leds";
#[cfg(test)]
pub const LEDDIR: &str = "testbldir";

/// Distinguish between a dimmable and a non-dimmable LED
///
/// See [module][self] level docs for usage examples.
#[derive(Debug)]
pub enum LedType {
    /// LED that supports multiple brightness values
    Dimmable(Led<Dimmable>),
    /// LED that supports only 1 (on) and 0 (off) brightness values
    NonDimmable(Led<NonDimmable>),
}

/// Marker type used with [`Led`] to enable dimmable LED specific functionality
#[derive(Debug)]
pub struct Dimmable;

/// Marker type used with [`Led`] to restrict LED functionality to simple on/off toggle
#[derive(Debug)]
pub struct NonDimmable;

/// Abstraction of an LED device from `/sys/class/leds`
///
/// An LED can either be dimmable or non-dimmable. Non-dimmable LED instances only provide the [`Light::toggle`]
/// and [`Light::write_value`] method to change the state of the device. Dimmable LEDs, on the other hand, enable
/// access to all the methods provided by the [`Light`] trait. These constraints are statically enforced by the type system.
///
/// For usage examples, see [module][self] level docs.
#[derive(Debug)]
pub struct Led<Type> {
    name: LedName<'static>,
    max: u8,
    current: u8,
    path: PathBuf,
    brightness: File,
    marker: PhantomData<Type>,
}

impl Led<()> {
    /// Create a new instance of an Led
    ///
    /// An instance will be created only if an LED of the provided name exists and the max and current brightness values are successfully read.
    ///
    /// Note: The initialized Led will only contain additional `function` and `color` information if the name of the LED
    /// is in accordance with the device naming convention described in <https://www.kernel.org/doc/html/latest/leds/leds-class.html#led-device-naming>.
    ///
    /// # Errors
    /// - `NotFound` - an LED dir of the given name is not found
    /// - `ReadMax` - failure to read the max brightness value
    /// - `ReadCurrent` - failure to read the current brightness value
    pub fn new(name: Cow<str>) -> crate::Result<LedType> {
        Self::new_inner(LedName::parse(name))
    }

    fn new_inner(name: LedName) -> crate::Result<LedType> {
        let utils::Info {
            current,
            max,
            brightness,
            path,
        } = utils::read_info(LEDDIR, &name.raw)?;
        #[allow(clippy::cast_possible_truncation)]
        let (max, current) = (max as _, current as _);
        let name = name.into_owned();
        let led = if max == 1 {
            LedType::NonDimmable(Led {
                name,
                max,
                current,
                path,
                brightness,
                marker: PhantomData,
            })
        } else {
            LedType::Dimmable(Led {
                name,
                max,
                current,
                path,
                brightness,
                marker: PhantomData,
            })
        };

        Ok(led)
    }

    /// Create an instance of an [`Led`] from an existing [`LedName`]
    ///
    /// An instance will be created only if an LED of the provided name exists and the max and current brightness values are successfully read.
    ///
    /// Note: The initialized Led will only contain additional `function` and `color` information if the name of the LED
    /// is in accordance with the device naming convention described in <https://www.kernel.org/doc/html/latest/leds/leds-class.html#led-device-naming>.
    ///
    /// # Errors
    /// - `NotFound` - an LED dir of the given name is not found
    /// - `ReadMax` - failure to read the max brightness value
    /// - `ReadCurrent` - failure to read the current brightness value
    pub fn from_name(name: LedName) -> crate::Result<LedType> {
        Self::new_inner(name)
    }
}

impl<Type> Led<Type> {
    /// Supported color of the LED
    ///
    /// See type level docs of [`LedName`] for additional details on parsing
    pub fn color(&self) -> Color {
        self.name.color
    }

    /// Function of the LED, such as Capslock and Numlock
    ///
    /// See type level docs of [`LedName`] for additional details on parsing
    pub fn function(&self) -> Function {
        self.name.function
    }

    /// Name of the LED that was parsed from the full device name using the standard Linux LED naming convention
    ///
    /// See type level docs of [`LedName`] for additional details on parsing
    pub fn parsed_name(&self) -> Option<&str> {
        self.name.parsed_name()
    }
}

impl<Type> private::Sealed for Led<Type> {}

impl super::Dimmable for Led<Dimmable> {}

impl super::Toggleable for Led<Dimmable> {}

impl super::Toggleable for Led<NonDimmable> {}

impl<Type> Light for Led<Type> {
    type Value = u8;

    /// Full name of the LED device
    ///
    /// Use [`Led::parsed_name`] to get the parsed name (if available)
    fn name(&self) -> &str {
        self.name.raw_name()
    }

    fn current(&self) -> Self::Value {
        self.current
    }

    fn max(&self) -> Self::Value {
        self.max
    }

    #[doc(hidden)]
    fn set_current(&mut self, _: crate::private::Internal, current: Self::Value) {
        self.current = current;
    }

    #[doc(hidden)]
    fn brightness_file(&mut self, _: crate::private::Internal) -> &mut File {
        &mut self.brightness
    }

    /// Returns absolute path that points to the device directory in `/sys/class/leds`
    fn device_path(&self) -> &Path {
        &self.path
    }
}

/// Abstraction that represents the name of an LED device
///
/// If [`LedName`] was initialized with a an LED name formatted according to the
/// device naming convention described in <https://www.kernel.org/doc/html/latest/leds/leds-class.html#led-device-naming>/,
/// the resulting instance can be inspected to get additional details of an LED device, such as [`Self::color`], [`Self::function`], and [`Self::parsed_name`].
///
/// Note: the aforementioned methods are also directly available on the [`Led`] type.
#[derive(Debug, Clone, PartialEq)]
pub struct LedName<'a> {
    raw: Cow<'a, str>,
    len: usize,
    color: Color,
    function: Function,
}

impl<'a> LedName<'a> {
    /// Parse a string containing an LED interface name
    ///
    /// This function is infallible, which means any string will be accepted.
    /// However, only names formatted according to the Linux LED naming convention will be parsed correctly
    /// to get color, name and function information. See [type][LedName] level docs for additional details.
    pub fn parse(name: Cow<'a, str>) -> Self {
        let mut name = Self {
            raw: name,
            len: 0,
            color: Color::default(),
            function: Function::default(),
        };
        let Some((rem, fun)) = name.raw.rsplit_once(':') else {
            return name;
        };
        let Ok(fun): Result<Function, _> = fun.parse();
        name.function = fun;
        // If no string slice was encountered here
        // it means the name didn't contain `:` making it invalid
        let Some((rem, clr)) = rem.rsplit_once(':') else {
            return name;
        };
        let Ok(clr): Result<Color, _> = clr.parse();
        name.color = clr;
        name.len = rem.len();
        name
    }

    /// Color of the LED which was parsed from the name
    ///
    /// See type level docs for details on LED naming convention.
    pub fn color(&self) -> Color {
        self.color
    }

    /// Function of the LED which was parsed from the name (Capslock, Scrollock, Numlock, etc)
    ///
    /// See type level docs for details on LED naming convention.
    pub fn function(&self) -> Function {
        self.function
    }

    /// The full unparsed name of the LED (same as the string used to initialize the `LedName`)
    ///
    /// See type level docs for details on LED naming convention.
    pub fn raw_name(&self) -> &str {
        &self.raw
    }

    /// Parsed name of the LED
    ///
    /// See type level docs for details on LED naming convention.
    pub fn parsed_name(&self) -> Option<&str> {
        (self.len != 0).then_some(&self.raw[..self.len])
    }

    /// Initialize an instance of [`Led`] using `self`
    ///
    /// This is identical to calling [`Led::from_name`] and passing self to it (which is exactly what this method does).
    ///
    /// # Errors
    /// - All possible errors returned by [`Led::from_name`] or [`Led::new`]
    pub fn initialize(self) -> crate::Result<LedType> {
        Led::from_name(self)
    }

    /// Convert an LED name containing borrowed data into an owned instance
    pub fn into_owned(self) -> LedName<'static> {
        LedName {
            raw: self.raw.into_owned().into(),
            ..self
        }
    }
}

/// Supported color of an LED
///
/// Use [`LedName`] to parse an LED name (string) to inspect its supported color. The same is also
/// done automatically when initializing an LED with [`Led::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Color {
    White = 0,
    Red = 1,
    Green = 2,
    Blue = 3,
    Amber = 4,
    Violet = 5,
    Yellow = 6,
    Ir = 7,
    Multi = 8,
    Rgb = 9,
    Purple = 10,
    Orange = 11,
    Pink = 12,
    Cyan = 13,
    Lime = 14,
    Max = 15,
    #[default]
    Unknown,
}

impl FromStr for Color {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clr = match s {
            "white" => Color::White,
            "red" => Color::Red,
            "green" => Color::Green,
            "blue" => Color::Blue,
            "amber" => Color::Amber,
            "violet" => Color::Violet,
            "yellow" => Color::Yellow,
            "ir" => Color::Ir,
            "multi" => Color::Multi,
            "rgb" => Color::Rgb,
            "purple" => Color::Purple,
            "orange" => Color::Orange,
            "pink" => Color::Pink,
            "cyan" => Color::Cyan,
            "lime" => Color::Lime,
            "max" => Color::Max,
            _ => Color::default(),
        };
        Ok(clr)
    }
}

/// Function of an LED
///
/// Use [`LedName`] to parse an LED name (string) to inspect its function. The same is also
/// done automatically when initializing an LED with [`Led::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Function {
    Capslock,
    Scrolllock,
    Numlock,
    Fnlock,
    KbdBacklight,
    Power,
    Disk,
    Charging,
    Status,
    Micmute,
    Mute,
    Player1,
    Player2,
    Player3,
    Player4,
    Player5,
    Activity,
    Alarm,
    Backlight,
    Bluetooth,
    Boot,
    Cpu,
    Debug,
    DiskActivity,
    DiskErr,
    DiskRead,
    DiskWrite,
    Fault,
    Flash,
    Heartbeat,
    Indicator,
    Lan,
    Mail,
    Mobile,
    Mtd,
    Panic,
    Programming,
    Rx,
    Sd,
    SpeedLan,
    SpeedWan,
    Standby,
    Torch,
    Tx,
    Usb,
    Wan,
    WanOnline,
    Wlan,
    Wlan2ghz,
    Wlan5ghz,
    Wlan6ghz,
    Wps,
    #[default]
    Unknown,
}

impl FromStr for Function {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[allow(clippy::enum_glob_use)]
        use Function::*;
        let func = match s {
            "capslock" => Capslock,
            "scrolllock" => Scrolllock,
            "numlock" => Numlock,
            "fnlock" => Fnlock,
            "kbd_backlight" => KbdBacklight,
            "power" => Power,
            "disk" => Disk,
            "charging" => Charging,
            "status" => Status,
            "micmute" => Micmute,
            "mute" => Mute,
            "player-1" => Player1,
            "player-2" => Player2,
            "player-3" => Player3,
            "player-4" => Player4,
            "player-5" => Player5,
            "activity" => Activity,
            "alarm" => Alarm,
            "backlight" => Backlight,
            "bluetooth" => Bluetooth,
            "boot" => Boot,
            "cpu" => Cpu,
            "debug" => Debug,
            "disk-activity" => DiskActivity,
            "disk-err" => DiskErr,
            "disk-read" => DiskRead,
            "disk-write" => DiskWrite,
            "fault" => Fault,
            "flash" => Flash,
            "heartbeat" => Heartbeat,
            "indicator" => Indicator,
            "lan" => Lan,
            "mail" => Mail,
            "mobile" => Mobile,
            "mtd" => Mtd,
            "panic" => Panic,
            "programming" => Programming,
            "rx" => Rx,
            "sd" => Sd,
            "speed-lan" => SpeedLan,
            "speed-wan" => SpeedWan,
            "standby" => Standby,
            "torch" => Torch,
            "tx" => Tx,
            "usb" => Usb,
            "wan" => Wan,
            "wan-online" => WanOnline,
            "wlan" => Wlan,
            "wlan-2ghz" => Wlan2ghz,
            "wlan-5ghz" => Wlan5ghz,
            "wlan-6ghz" => Wlan6ghz,
            "wps" => Wps,
            _ => Unknown,
        };
        Ok(func)
    }
}

/// Helper function to read all the LED names available in `/sys/class/leds`
///
/// The name can be used to inspect the color and function of an LED (if parsed correctly),
/// without initializing an instance of [`Led`].
///
/// # Errors
/// - `ReadDir` - failure to read [`LEDDIR`] (usually due to missing permissions)
pub fn led_names() -> crate::Result<Vec<LedName<'static>>> {
    let read_dir_err = |err| Error::from(ErrorKind::ReadDir { dir: LEDDIR }).with_source(err);
    let mut names = vec![];
    for d in std::fs::read_dir(LEDDIR).map_err(read_dir_err)? {
        let is_dir = d.as_ref().is_ok_and(|inr| inr.path().is_dir());
        if is_dir {
            let entry = d.map_err(read_dir_err)?;
            names.push(LedName::parse(entry.file_name().to_string_lossy()).into_owned());
        }
    }
    Ok(names)
}

/// Helper function to initialize all the LED devices available in `/sys/class/leds`
///
/// This function will return an error if any single LED fails to initialize.
///
/// # Errors
/// - All possible errors returned by [`led_names`]
/// - All possible errors returned by [`Led::from_name`]
pub fn leds() -> crate::Result<Vec<LedType>> {
    led_names().and_then(|names| names.into_iter().map(Led::from_name).collect())
}

/// Helper function to initialize all LEDs from an iterator over [`LedName`]s
///
/// This function will return an error if any single LED fails to initialize.
///
/// # Examples
/// ```no_run
/// # fn main() -> blight::Result<()> {
/// let leds = blight::led::leds_from_names(
///     blight::led::led_names()?
///         .into_iter()
///         .filter(|n| n.color() == blight::led::Color::Rgb),
/// )?;
/// for led in leds {
/// // do something here
/// }
/// #   Ok(())
/// # }
/// ```
///
/// # Errors
/// - All possible errors returned by [`Led::from_name`]
pub fn leds_from_names<'a>(
    names: impl IntoIterator<Item = LedName<'a>>,
) -> crate::Result<Vec<LedType>> {
    names.into_iter().map(Led::from_name).collect()
}

/// Helper function to turn an LED on/off
///
/// `State`: true = on, false = off
///
/// # Errors
/// - All possible errors returned by [`Led::new`] and [`Light::write_value`]
pub fn set_led_state(led_name: &str, state: bool) -> crate::Result<()> {
    let value = u8::from(state);
    match Led::new(led_name.into())? {
        LedType::Dimmable(mut led) => led.write_value(value),
        LedType::NonDimmable(mut led) => led.write_value(value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{clean_up, setup_test_env};

    #[test]
    fn parse_name() {
        let cases = [
            (
                "platform:white:kbd_backlight",
                LedName {
                    raw: "platform:white:kbd_backlight".into(),
                    len: 8,
                    color: Color::White,
                    function: Function::KbdBacklight,
                },
                Some("platform"),
            ),
            (
                "input13::capslock",
                LedName {
                    raw: "input13::capslock".into(),
                    len: 7,
                    color: Color::Unknown,
                    function: Function::Capslock,
                },
                Some("input13"),
            ),
            (
                "input7::numlock",
                LedName {
                    raw: "input7::numlock".into(),
                    len: 6,
                    color: Color::Unknown,
                    function: Function::Numlock,
                },
                Some("input7"),
            ),
            (
                "name::",
                LedName {
                    raw: "name::".into(),
                    len: 4,
                    color: Color::Unknown,
                    function: Function::Unknown,
                },
                Some("name"),
            ),
            (
                "unknown",
                LedName {
                    raw: "unknown".into(),
                    len: 0,
                    color: Color::Unknown,
                    function: Function::Unknown,
                },
                None,
            ),
        ];
        for (i, (string, expected, expected_name)) in cases.into_iter().enumerate() {
            let name: LedName = LedName::parse(string.into());
            assert_eq!(name, expected, "Case {i} failed");
            assert_eq!(name.parsed_name(), expected_name, "case {i} failed");
        }
    }

    #[test]
    fn initialize_dimmable() {
        clean_up();
        let name = "generic";
        setup_test_env(&[name], 10, 100);
        let led = Led::new(name.into()).expect("failed to initialize LED");
        assert!(
            matches!(led, LedType::Dimmable(_)),
            "Initialized LED is not of dimmable type"
        );
        clean_up();
    }

    #[test]
    fn initialize_non_dimmable() {
        clean_up();
        let name = "generic";
        setup_test_env(&[name], 1, 1);
        let led = Led::new(name.into()).expect("failed to initialize LED");
        assert!(
            matches!(led, LedType::NonDimmable(_)),
            "Initialized LED is not of dimmable type"
        );
        clean_up();
    }

    #[test]
    fn names() {
        clean_up();
        let names = ["led1", "led2", "led3"];
        setup_test_env(&names, 0, 1);
        let mut names_read: Vec<_> = led_names()
            .expect("failed to get LED names")
            .into_iter()
            .map(|n| n.raw.into_owned())
            .collect();
        names_read.sort_unstable();
        let names_read: Vec<&str> = names_read.iter().map(String::as_str).collect();
        assert_eq!(
            names.as_ref(),
            &names_read,
            "LED names read from the dir do not match"
        );
        clean_up();
    }

    #[test]
    fn set_state() {
        clean_up();
        let name = "generic";
        setup_test_env(&[name], 0, 1);
        // Turn LED on
        set_led_state(name, true).expect("failed to turn on LED");
        let LedType::NonDimmable(mut led) = Led::new(name.into()).unwrap() else {
            unreachable!()
        };
        assert_eq!(led.current(), 1, "LED is not turned on");
        // Turn LED off
        set_led_state(name, false).expect("failed to turn off LED");
        led.reload();
        assert_eq!(led.current(), 0, "LED is not turned off");
        clean_up();
    }
}
