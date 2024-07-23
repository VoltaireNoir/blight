use std::{
    ffi::OsString,
    io::{self, Read},
    path::Path,
    str::FromStr,
};

pub const LEDDIR: &str = "/sys/class/leds";

#[derive(Debug, Clone)]
pub struct Led {
    name: Name,
    max: u8,
    current: u8,
}

#[derive(Debug, Clone, Copy)]
enum ValType {
    Max,
    Current,
}

impl AsRef<str> for ValType {
    fn as_ref(&self) -> &str {
        match self {
            ValType::Max => super::MAX_FILE,
            ValType::Current => super::CURRENT_FILE,
        }
    }
}

impl Led {
    /// Create a new instance of an Led
    ///
    /// An instance will be created only if the name is parsed correctly, and the max and current brightness values are read.
    /// Note: This function will reject any Leds with names that do not conform to the
    /// device naming standard described in <https://www.kernel.org/doc/html/latest/leds/leds-class.html#led-device-naming/>.
    ///
    /// If the parsing strategy is too strict, use [`Led::new_lenient`] instead.
    pub fn new(name: &str) -> Option<Self> {
        Self::new_inner(name.parse().ok()?)
    }

    /// Same as [`Led::new`] except the parsing strategy is lenient for parsing the name
    pub fn new_lenient(name: &str) -> Option<Self> {
        Self::new_inner(name.parse().unwrap_or_else(|()| {
            let len = name.find(':').and_then(|i| (i > 0).then_some(i));
            Name {
                raw: name.into(),
                name_len: len,
                color: None,
                function: None,
            }
        }))
    }

    fn new_inner(name: Name) -> Option<Self> {
        let mut uninit = Self {
            name,
            max: 0,
            current: 0,
        };
        let max = uninit.read_value(ValType::Max, LEDDIR)?;
        let cur = uninit.read_value(ValType::Current, LEDDIR)?;
        uninit.max = max;
        uninit.current = cur;
        Some(uninit)
    }

    fn read_value(&self, vtype: ValType, dir: &str) -> Option<u8> {
        let mut buf: [u8; 3] = [0; 3];
        #[allow(clippy::unused_io_amount)]
        std::fs::File::open(format!("{dir}/{}/{}", self.name.raw, vtype.as_ref()))
            .ok()?
            .read(&mut buf)
            .ok()?;
        let pat: &[_] = &['\0', '\n', ' '];
        std::str::from_utf8(&buf)
            .ok()?
            .trim_matches(pat)
            .parse::<u8>()
            .ok()
    }

    fn color(&self) -> Option<Color> {
        self.name.color
    }

    fn function(&self) -> Option<Function> {
        self.name.function
    }

    fn name(&self) -> Option<&str> {
        let pos = self.name.name_len?;
        std::str::from_utf8(&self.name.raw.as_bytes()[..pos]).ok()
    }

    fn raw_name(&self) -> &str {
        &self.name.raw
    }
}

#[derive(Debug, Clone)]
struct Name {
    raw: String,
    name_len: Option<usize>,
    color: Option<Color>,
    function: Option<Function>,
}

impl FromStr for Name {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut iter = s.rsplit(':');
        let Some(fun) = iter.next() else {
            return Err(());
        };
        let fun: Function = fun.parse()?;
        let clr: Option<Color> = match iter.next() {
            Some(c) => c.parse().ok(),
            // If no string slice was encountered here
            // it means the name didn't contain `:` making it invalid
            None => return Err(()),
        };
        let name: Option<usize> = iter.next().map(str::len);
        Ok(Self {
            raw: s.to_owned(),
            name_len: name,
            color: clr,
            function: Some(fun),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl FromStr for Color {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
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
            _ => return Err(()),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

impl FromStr for Function {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[allow(clippy::enum_glob_use)]
        use Function::*;
        Ok(match s {
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
            _ => return Err(()),
        })
    }
}

fn led_names<P: AsRef<Path>>(path: P) -> Result<Vec<OsString>, io::Error> {
    Ok(std::fs::read_dir(path)?
        .filter_map(|d| {
            d.ok().and_then(|inr| {
                if inr.path().is_dir() {
                    Some(inr.file_name())
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>())
}

fn leds<P: AsRef<Path>>(path: P) -> Result<Vec<Led>, io::Error> {
    led_names(path).map(|names| {
        names
            .into_iter()
            .filter_map(|n| n.to_str().and_then(Led::new_lenient))
            .collect()
    })
}

fn leds_from_names(names: &[&str]) -> Vec<Led> {
    names.iter().filter_map(|n| Led::new_lenient(n)).collect()
}

#[cfg(test)]
mod tests {
    use super::{led_names, leds, LEDDIR};

    #[test]
    fn all_leds() {
        let names = led_names(LEDDIR).unwrap();
        let leds = leds(LEDDIR).unwrap();
        assert_eq!(names.len(), leds.len());
    }

    #[test]
    fn names() {
        let leds = leds(LEDDIR).unwrap();
        dbg!(leds.iter().filter_map(|l| l.name()).collect::<Vec<_>>());
    }
}
