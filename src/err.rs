//! All blight library related errors in one place. See [BlibError]

use std::{borrow::Cow, error::Error};

pub type BlResult<T> = Result<T, BlibError>;
/// All blight library related errors in one place. Every time one of the functions or methods of the library return an error, it'll always be one of this enum's variants.
/// Some variants wrap additional error information and all of them have their separate Display trait implementations, containing a simple description of the error and possibly
/// a tip to help the user fix it.
#[derive(Debug)]
pub enum BlibError {
    ReadBlDir(std::io::Error),
    NoDeviceFound,
    WriteNewVal { err: std::io::Error, dev: String },
    ReadMax,
    ReadCurrent,
    SweepError(std::io::Error),
    ValueTooLarge { given: u32, supported: u32 },
}

#[doc(hidden)]
pub trait Tip: Error + 'static {
    fn tip(&self) -> Option<Cow<'static, str>>;
}

impl Tip for BlibError {
    fn tip(&self) -> Option<Cow<'static, str>> {
        use BlibError::WriteNewVal;
        match &self {
            WriteNewVal { dev, .. } => {
                let tip_msg = format!(
                    "{main} '{dir}/{dev}/brightness'\n{extra}",
                    main = "make sure you have write permission to the file",
                    dir = super::BLDIR,
                    extra = "
Run `sudo blight setup` to install necessarry udev rules and add user to video group.
or visit https://wiki.archlinux.org/title/Backlight#Hardware_interfaces
if you'd like to do it manually.",
                );
                Some(tip_msg.into())
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for BlibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BlibError::*;
        match self {
            ReadBlDir(e) => write!(f, "failed to read {} directory\n{e}", super::BLDIR),

            NoDeviceFound => write!(f, "no known backlight device detected"),

            WriteNewVal { err, .. } => {
                write!(f, "failed to write to the brightness file ({err})",)
            }

            ReadCurrent => write!(f, "failed to read current brightness value"),

            ReadMax => write!(f, "failed to read max brightness value"),

            SweepError(err) => write!(f, "failed to sweep write to brightness file ({err})"),

            ValueTooLarge { given, supported } => write!(
                f,
                "provided value ({given}) is larger than the max supported value of {supported}"
            ),
        }
    }
}

impl std::error::Error for BlibError {}
