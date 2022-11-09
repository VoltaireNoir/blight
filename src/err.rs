use std::path::PathBuf;

use colored::Colorize;

#[derive(Debug)]
pub enum BlibError {
    NoDeviceFound,
    WriteNewVal { err: std::io::Error, dev: String },
    ReadMax,
    ReadCurrent,
    CreateSaveDir(PathBuf),
    WriteToSaveFile(PathBuf),
    ReadFromSave(std::io::Error),
    NoSaveFound,
    SaveParseErr,
}

impl std::fmt::Display for BlibError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BlibError::*;
        match self {
            NoDeviceFound => write!(f, "No known backlight device detected"),

            WriteNewVal { err, dev } => {
                let tip_msg = format!(
                    "{main} '{dir}/{dev}/brightness'\n{extra}",
                    main = "Make sure you have write permission to the file",
                    dir = super::BLDIR,
                    extra = "
Run `sudo blight setup` to install necessarry udev rules and add user to video group.
Or visit https://wiki.archlinux.org/title/Backlight#Hardware_interfaces
if you'd like to do it manually.",
                );
                write!(
                    f,
                    "Failed to write to the brightness file ({err})\n{} {tip_msg}",
                    "Tip".yellow().bold()
                )
            }

            ReadCurrent => write!(f, "Failed to read current brightness value"),

            ReadMax => write!(f, "Failed to read max brightness value"),

            CreateSaveDir(loc) => write!(f, "Failed to create save directory at {}", loc.display()),

            WriteToSaveFile(loc) => write!(f, "Failed to write to save file at {}", loc.display()),

            ReadFromSave(err) => write!(f, "Failed to read from save file\n{err}"),

            NoSaveFound => write!(
                f,
                "No save file found\n{} Try using 'blight save' first",
                "Tip:".yellow().bold()
            ),

            SaveParseErr => write!(f, "Failed to parse saved brightness value"),
        }
    }
}

impl std::error::Error for BlibError {}
