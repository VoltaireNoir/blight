use blight::{
    err::{BlibError, Tip},
    Change, Device,
    Direction::{self, Dec, Inc},
    BLDIR,
};
use colored::Colorize;
use fs4::FileExt;
use std::{
    borrow::Cow,
    env,
    env::Args,
    error::Error,
    fs::{self, File, OpenOptions},
    iter::Skip,
    path::PathBuf,
};

mod setup;

const SAVEDIR: &str = "/.local/share/blight";
const LOCKFILE: &str = "/tmp/blight.lock";

type DynError = Box<dyn Error + 'static>;

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
    Adjust { dir: Direction, value: u32 },
    Set(u32),
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

pub fn parse<'a>(mut args: Skip<Args>) -> Result<Config<'a>, DynError> {
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
                let val: u32 = args
                    .next()
                    .ok_or(MissingValue)?
                    .parse()
                    .or(Err(InvalidValue))?;

                (Set(val), option_parser(args))
            }

            ch @ ("inc" | "dec") => {
                let value: u32 = args
                    .next()
                    .ok_or(MissingValue)?
                    .parse()
                    .map_err(|_| InvalidValue)?;

                let dir = if ch == "inc" { Inc } else { Dec };

                (Adjust { dir, value }, option_parser(args))
            }
            _ => Err(UnrecognisedCommand)?,
        }
    } else {
        no_op(Command::ShortHelp)
    };

    Ok(Config { command, options })
}

type SuccessMessage = &'static str;

pub fn execute(conf: Config) -> Result<SuccessMessage, DynError> {
    use Command::*;

    match conf.command {
        Help => print_help(),
        ShortHelp => print_shelp(),
        List => print_devices(),
        Setup => setup::run(),
        Status => print_status(conf.options.device)?,
        Save => save(conf.options.device)?,
        Restore => restore()?,
        Set(v) => {
            let _lock = acquire_lock();
            blight::set_bl(v, conf.options.device)?
        }
        Adjust { dir, value } => {
            let _lock = acquire_lock();
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
    CreateSaveDir(PathBuf),
    WriteToSaveFile(PathBuf),
    ReadFromSave(std::io::Error),
    NoSaveFound,
    SaveParseErr,
}

impl Tip for BlightError {
    fn tip(&self) -> Option<Cow<'static, str>> {
        use BlightError::*;
        match self {
            UnrecognisedCommand => Some("try 'blight help' to see all commands".into()),
            InvalidValue => Some("make sure the value is a valid positive integer".into()),
            NoSaveFound => Some("try using 'blight save' first".into()),
            MissingValue => {
                Some("try 'blight help' to see all commands and their supported args".into())
            }
            ReadFromSave(_) => Some("make sure you have read permission for the save file".into()),
            SaveParseErr => Some("delete the save file and try save-restore again".into()),
            _ => None,
        }
    }
}

impl std::fmt::Display for BlightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use BlightError::*;
        match self {
            UnrecognisedCommand => write!(f, "unrecognised command entered"),
            MissingValue => write!(f, "required argument was not provided for the command"),
            InvalidValue => write!(f, "invalid value provided"),
            CreateSaveDir(loc) => write!(f, "failed to create save directory at {}", loc.display()),
            WriteToSaveFile(loc) => write!(f, "failed to write to save file at {}", loc.display()),
            ReadFromSave(err) => write!(f, "failed to read from save file\n{err}"),
            NoSaveFound => write!(f, "no save file found"),
            SaveParseErr => write!(f, "failed to parse saved brightness value"),
        }
    }
}

impl Error for BlightError {}

pub fn print_err(e: DynError) {
    eprintln!("{} {e}", "Error".red().bold());
    if let Some(tip) = e
        .downcast_ref::<BlibError>()
        .and_then(|e| e.tip())
        .or(e.downcast_ref::<BlightError>().and_then(|e| e.tip()))
    {
        eprintln!("{} {tip}", "Tip".yellow().bold())
    }
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
        "{}\nDetected device: {}\nWrite permission: {}\nCurrent brightness: {}\nMax brightness: {}",
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
    Sweep flag lets you increase brightness gradually, resulting in a smooth change.
    Dev (short for device) flag lets you specify a backlight device target other than the default one.";
    let commands: String = [
        ("inc [val] [flags: dev, sweep]", "-> increase brightness"),
        ("dec [val] [flags: dev, sweep]", "-> decrease brightness"),
        ("set [val] [flags: dev]", "-> set custom brightness value"),
        (
            "save [flags: dev]",
            "-> save current brightness value to restore later",
        ),
        ("restore", "-> restore saved brightness value\n"),
        (
            "setup",
            "-> installs udev rules and adds user to video group (run with sudo)",
        ),
        ("status [flags: dev]", "-> backlight device status"),
        ("list", "-> list all backlight devices"),
        ("help", "-> display help"),
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

pub fn save(device_name: Option<Cow<str>>) -> Result<(), DynError> {
    let device = Device::new(device_name)?;
    let mut savedir = PathBuf::from(env::var("HOME").unwrap() + SAVEDIR);

    if !savedir.exists() && fs::create_dir_all(&savedir).is_err() {
        return Err(BlightError::CreateSaveDir(savedir).into());
    }

    savedir.push("blight.save");

    fs::write(&savedir, format!("{} {}", device.name(), device.current()))
        .map_err(|_| BlightError::WriteToSaveFile(savedir))?;

    Ok(())
}

pub fn restore() -> Result<(), DynError> {
    let save = PathBuf::from((env::var("HOME").unwrap() + SAVEDIR) + "/blight.save");

    let restore = if save.is_file() {
        fs::read_to_string(save).map_err(BlightError::ReadFromSave)?
    } else {
        Err(BlightError::NoSaveFound)?
    };

    let (device_name, val) = restore.split_once(' ').unwrap();
    let device = Device::new(Some(device_name.into()))?;

    let value: u32 = val.parse().map_err(|_| BlightError::SaveParseErr)?;
    device.write_value(value)?;
    Ok(())
}

pub struct PanicReporter;

impl PanicReporter {
    pub fn init() {
        if !cfg!(debug_assertions) {
            std::panic::set_hook(Box::new(Self::report));
        }
    }
    fn report(info: &std::panic::PanicInfo) {
        let tip = "This is unexpected behavior. Please report this issue at https://github.com/VoltaireNoir/blight/issues";
        let payload = info.payload();
        let cause = if let Some(pay) = payload.downcast_ref::<&str>() {
            pay.to_string()
        } else if let Some(pay) = payload.downcast_ref::<String>() {
            pay.to_string()
        } else {
            "Unknown".to_owned()
        };
        eprintln!("{} A panic occured", "Error".red().bold());
        eprintln!("{} {cause}", "Reason".magenta().bold());
        if let Some(loc) = info.location() {
            eprintln!("{} {}", "Location".blue().bold(), loc);
        }
        eprintln!("{} {tip}", "Tip".yellow().bold());
    }
}

fn acquire_lock() -> File {
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(LOCKFILE)
        .expect("failed to open lock file");
    if file.try_lock_exclusive().is_ok() {
        return file;
    }
    println!(
        "{} {}",
        "Status".magenta().bold(),
        "Waiting for another instance to finish"
    );
    file.lock_exclusive().expect("failed to acquire lock");
    file
}
