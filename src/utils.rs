use blight::{
    err::BlibError,
    Change, Device,
    Direction::{self, Dec, Inc},
    BLDIR,
};
use colored::Colorize;
use std::{borrow::Cow, env, env::Args, fs, iter::Skip, path::PathBuf, process};

mod setup;

const SAVEDIR: &str = "/.local/share/blight";

pub struct Config<'a> {
    command: Command,
    options: Options<'a>,
}

enum Command {
    Setup,
    Help,
    ShortHelp,
    Status,
    Save,
    Restore,
    List,
    Adjust { dir: Direction, value: u16 },
    Set(u16),
}

#[derive(Default)]
struct Options<'a> {
    device: Option<Cow<'a, str>>,
    sweep: Change,
}

impl Options<'_> {
    fn set(mut self, arg: String) -> Self {
        match arg.as_str() {
            "-d" | "--device" => self.device = Some("".into()),
            "-s" | "--sweep" => self.sweep = Change::Sweep,
            _ => {
                if let Some(d) = &mut self.device {
                    if d.is_empty() {
                        *d = Cow::from(arg);
                    }
                }
            }
        }
        self
    }
}

pub fn parse<'a>(mut args: Skip<Args>) -> Result<Config<'a>, BlightError> {
    use BlightError::*;
    use Command::*;

    let option_parser =
        |args: Skip<Args>| -> Options { args.fold(Options::default(), |op, arg| op.set(arg)) };

    let no_op = |cm: Command| (cm, Options::default());

    let (command, options) = if let Some(arg) = args.next() {
        match arg.as_str() {
            "setup" => no_op(Setup),
            "help" => no_op(Help),
            "restore" => no_op(Restore),
            "list" => no_op(List),
            "status" => (Status, option_parser(args)),
            "save" => (Save, option_parser(args)),

            "set" => {
                let val: u16 = args
                    .next()
                    .ok_or(MissingValue)?
                    .parse()
                    .or(Err(InvalidValue))?;

                (Set(val), option_parser(args))
            }

            ch @ ("inc" | "dec") => {
                let value: u16 = args
                    .next()
                    .ok_or(MissingValue)?
                    .parse()
                    .or(Err(InvalidValue))?;

                let dir = if ch == "inc" { Inc } else { Dec };

                (Adjust { dir, value }, option_parser(args))
            }
            _ => return Err(UnrecognisedCommand),
        }
    } else {
        no_op(Command::ShortHelp)
    };

    Ok(Config { command, options })
}

type SuccessMessage = &'static str;

pub fn execute(conf: Config) -> Result<SuccessMessage, blight::err::BlibError> {
    use Command::*;

    match conf.command {
        Help => print_help(),
        ShortHelp => print_shelp(),
        List => print_devices(),
        Setup => setup::run(),
        Status => print_status(conf.options.device)?,
        Save => save(conf.options.device)?,
        Restore => restore()?,
        Set(v) => blight::set_bl(v, conf.options.device)?,
        Adjust { dir, value } => {
            blight::change_bl(value, conf.options.sweep, dir, conf.options.device)?
        }
    };

    Ok(gen_success_msg(&conf.command))
}

#[derive(Debug)]
pub enum BlightError {
    UnrecognisedCommand,
    MissingValue,
    InvalidValue,
}

impl std::fmt::Display for BlightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BlightError::*;
        match self {
            UnrecognisedCommand => write!(
                f,
                "Unrecognised command entered\n{} Try 'blight help' to see all commands",
                "Tip".yellow().bold()
            ),
            MissingValue => write!(f, "Required argument was not provided for the command"),
            InvalidValue => write!(
                f,
                "Invalid value provided\n{} Make sure the value is a valid positive integer",
                "Tip".yellow().bold()
            ),
        }
    }
}

pub fn print_err(e: impl std::fmt::Display) {
    eprintln!("{} {e}", "Error".red().bold())
}

pub fn print_ok(msg: &str) {
    if !msg.is_empty() {
        println!("{} {msg}", "Success".green().bold())
    }
}

fn gen_success_msg(cm: &Command) -> SuccessMessage {
    use Command::*;
    match cm {
        Save => "Current backlight state saved",
        Restore => "Saved backlight state restored",
        Set(_) => "Backlight value set",
        Adjust { .. } => "Backlight changed",
        _ => "",
    }
}

pub fn is_running() -> bool {
    let out = process::Command::new("pgrep")
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

pub fn print_status(device_name: Option<Cow<str>>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;

    let write_perm = match check_write_perm(device.name(), BLDIR) {
        Ok(_) => "Ok".green(),
        Err(err) => format!("{err}").red(),
    };

    println!(
        "{}\nDetected device: {}\nWrite permission: {}\nCurrent brightness: {}\nMax brightness {}",
        "Device status".bold(),
        device.name().green(),
        write_perm,
        device.current().to_string().green(),
        device.max().to_string().green()
    );
    Ok(())
}

pub fn print_devices() {
    println!("{}", "Detected Devices".bold());
    fs::read_dir(BLDIR)
        .expect("Failed to read Backlight Directory")
        .for_each(|d| println!("{}", d.unwrap().file_name().to_string_lossy().green()));
}

pub fn print_help() {
    let title = "blight: A backlight utility for Linux that plays well with hybrid GPUs";
    let quote = "\"And man said, \'let there b-light\' and there was light.\" - Some Book 1:3";
    let flags = "Flags: sweep [--sweep, -s], dev [--device <name>, -d <name>]
    Sweep flag lets you increases brightness gradually, resulting in a smooth change.
    Dev (short for device) flag lets you specify a backlight device target other than the default one.";
    let commands: String = [
        ("inc [val] [flags: dev, sweep]", "-> increase brightness"),
        ("dec [val] [flags: dev, sweep]", "-> decrease brightness"),
        ("set [val] [flags: dev]", "-> set custom brightness value"),
        (
            "save [flags: dev]",
            "-> save current brightness value to restore later",
        ),
        (
            "restore [flags: dev]",
            "-> restore saved brightness value\n",
        ),
        (
            "setup",
            "-> installs udev rules and adds user to video group (run with sudo)",
        ),
        ("status [flags: dev]", "-> backlight device status"),
        ("list", "-> list all backlight devices"),
        ("help", "-> displays help"),
    ]
    .into_iter()
    .map(|(c, e)| format!("{} {e}\n", c.green().bold()))
    .collect();

    let exampels = "\
Examples:
    sudo blight setup
    blight status (show backlight device status info)
    blight inc 5 --sweep (increase brightness smoothly by 5%)
    blight set 10 (sets the brightness value to 10)
    blight inc 2 -s -d nvidia_0 (increases nvidia_0's brightness smoothly by 2%)";

    println!(
        "{t}\n\n{quote}\n\n{f}\n\n{ct}\n{commands}\n{e}",
        t = title.blue().bold(),
        f = flags.magenta(),
        ct = "Commands".bold(),
        e = exampels.bright_yellow()
    );
}

pub fn print_shelp() {
    let cc: String = [
        ("inc [val]", "-> increase brightness by given value"),
        ("dec [val]", "-> decrease brightness by given value"),
        ("set [val]", "-> set custom brightness value"),
        ("status", "-> show backlight device info"),
        ("setup", "-> gain write permission to brightness file"),
    ]
    .into_iter()
    .map(|(c, e)| format!("{} {e}\n", c.green().bold()))
    .collect();

    println!(
        "{t}\n\n{ct}\n{cc}\n{h}",
        t = "blight: A backlight utility for Linux".blue().bold(),
        ct = "Common Commands".bold(),
        h = "Use `blight help' to display all commands and options".yellow()
    );
}

pub fn save(device_name: Option<Cow<str>>) -> Result<(), BlibError> {
    let device = Device::new(device_name)?;
    let mut savedir = PathBuf::from(env::var("HOME").unwrap() + SAVEDIR);

    if !savedir.exists() && fs::create_dir_all(&savedir).is_err() {
        return Err(BlibError::CreateSaveDir(savedir));
    }

    savedir.push("blight.save");

    if fs::write(&savedir, format!("{} {}", device.name(), device.current())).is_err() {
        return Err(BlibError::WriteToSaveFile(savedir));
    };

    Ok(())
}

pub fn restore() -> Result<(), BlibError> {
    let save = PathBuf::from((env::var("HOME").unwrap() + SAVEDIR) + "/blight.save");

    let restore = if save.is_file() {
        fs::read_to_string(save).map_err(BlibError::ReadFromSave)?
    } else {
        return Err(BlibError::NoSaveFound);
    };

    let (device_name, val) = restore.split_once(' ').unwrap();
    let device = Device::new(Some(device_name.into()))?;

    let value: u16 = val.parse().or(Err(BlibError::SaveParseErr))?;
    device.write_value(value)?;
    Ok(())
}
