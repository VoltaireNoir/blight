use blight::{
    led::{self, Color, Function},
    Change, Device,
    Direction::{self, Dec, Inc},
    Light, BLDIR,
};
use colored::Colorize;
use std::{borrow::Cow, env, env::Args, fs, iter::Skip, path::PathBuf};

mod setup;

const SAVEDIR: &str = "/.local/share/blight";
const LOCKFILE: &str = "/tmp/blight.lock";

type DynError = Box<dyn std::error::Error + 'static>;

#[derive(Debug)]
pub struct Config<'a> {
    command: Command,
    options: Options<'a>,
}

#[derive(Debug)]
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
    Led(LedCommand),
}

#[derive(Debug)]
enum LedCommand {
    Toggle { led: String, kind: ToggleKind },
    Set { led: String, value: u8 },
    Info(String),
    List { raw: bool, filter: Option<LedListFilter> },
    Help,
    ShortHelp,
}

#[derive(Debug)]
enum LedListFilter {
    Index(usize),
    Function(String),
    Color(String), 
    FunctionColor(String, String)
}

#[derive(Debug)]
enum ToggleKind {
    Toggle,
    On,
    Off,
}

#[derive(Default, Debug)]
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

    let no_opt = |cm: Command| (cm, Options::default());

    let (command, options) = if let Some(arg) = args.next() {
        match arg.as_str() {
            "setup" => no_opt(Setup),
            "help" => no_opt(Help),
            "restore" => no_opt(Restore),
            "list" => no_opt(List),
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

            "led" => 'led: {
                let Some(cmd) = args.next() else {
                    break 'led no_opt(Command::Led(LedCommand::ShortHelp));
                };
                let led_err = BlightError::Led;
                let parsed = match (cmd.as_str(), args.next()) {
                    ("info", Some(led)) => LedCommand::Info(led),
                    ("set", Some(led)) => LedCommand::Set {
                        led,
                        value: args
                            .next()
                            .ok_or(led_err(LedError::MissingValue))?
                            .parse()
                            .map_err(|_| led_err(LedError::InvalidValue))?,
                    },
                    ("toggle", Some(led)) => LedCommand::Toggle {
                        led,
                        kind: args
                            .next()
                            .map(|kind| match kind.as_str() {
                                "--on" => Ok(ToggleKind::On),
                                "--off" => Ok(ToggleKind::Off),
                                _ => Err(UnrecognisedCommand),
                            })
                            .unwrap_or(Ok(ToggleKind::Toggle))?,
                    },
                    ("toggle" | "set", None) => Err(led_err(LedError::MissingName))?,
                    ("list", None) => LedCommand::List { raw: false, filter: None },
                    ("list", Some(arg)) => {
                        let (raw, filter) = parse_led_list_options(std::iter::once(arg).chain(args)).map_err(led_err)?;
                        LedCommand::List { raw, filter }
                    },
                    ("help", _) => LedCommand::Help,
                    _ => Err(UnrecognisedCommand)?,
                };
                no_opt(Command::Led(parsed))
            }

            _ => Err(UnrecognisedCommand)?,
        }
    } else {
        no_opt(Command::ShortHelp)
    };

    Ok(Config { command, options })
}

type SuccessMessage = &'static str;

pub fn execute(mut conf: Config) -> Result<SuccessMessage, DynError> {
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
            // Same impl as blight::set_bl but with file locking
            let mut device = new_locked(conf.options.device)?;
            if v != device.current() {
                device.write_value(v)?;
            }
        }
        Adjust { dir, value } => {
            // Same impl as blight::change_bl but with file locking
            let mut device = new_locked(conf.options.device)?;
            let change = device.calculate_change(value, dir);
            if change != device.current() {
                match conf.options.sweep {
                    Change::Sweep => device.sweep_write(change, blight::Delay::default())?,
                    Change::Regular => device.write_value(change)?,
                }
            }
        }
        Led(ref mut cmd) => {
            use blight::led;
            match cmd {
                LedCommand::Toggle { led, kind } => {
                    let state = match kind {
                        ToggleKind::Toggle => !(led::get_led_state(led)?),
                        ToggleKind::On => true,
                        ToggleKind::Off => false,
                    };
                    led::set_led_state(led, state)?;
                }
                LedCommand::Set { led, value } => {
                    led::set_led_value(led, *value)?;
                }
                LedCommand::Info(led) => print_led_info(led)?,
                LedCommand::List { raw, filter } => print_led_list(*raw, filter.take())?,
                LedCommand::Help => print_led_help(),
                LedCommand::ShortHelp => print_led_shelp(),
            }
        }
    };

    Ok(gen_success_msg(&conf.command))
}

trait Tip: std::error::Error + 'static {
    fn tip(&self) -> Option<Cow<'static, str>>;
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
    Led(LedError),
}

#[derive(Debug)]
pub enum LedError {
    MissingName,
    MissingValue,
    InvalidValue,
    BadListOptions(Option<&'static str>),
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
            Led(err) => match err {
                LedError::MissingName => write!(f, "no LED name provided"),
                LedError::MissingValue => {
                    write!(
                        f,
                        "no valid LED brightness value between 0-255 was provided"
                    )
                }
                LedError::InvalidValue => {
                    write!(
                        f,
                        "invalid LED brightness value. Value should be between 0-255"
                    )
                }
                LedError::BadListOptions(Some(desc)) => {
                    write!(f, "failed to parse options for 'led list' command: {desc}")
                }
                LedError::BadListOptions(None) => {
                    write!(f, "unknown options/filters provided for 'led list' command")
                }
            },
        }
    }
}

impl std::error::Error for BlightError {}

impl Tip for blight::Error {
    fn tip(&self) -> Option<Cow<'static, str>> {
        use blight::ErrorKind::{LockError, WriteValue};
        match self.kind() {
            WriteValue { device } => {
                let tip_msg = format!(
                    "{main} '{dir}/{device}/brightness'\n{extra}",
                    main = "make sure you have write permission to the file",
                    dir = blight::BLDIR,
                    extra = "
Run `sudo blight setup` to install necessarry udev rules and add user to video group.
or visit https://wiki.archlinux.org/title/Backlight#Hardware_interfaces
if you'd like to do it manually.",
                );
                Some(tip_msg.into())
            }
            LockError { .. } => {
                Some(format!("try manually removing the lock file: `{LOCKFILE}`").into())
            }
            _ => None,
        }
    }
}

pub fn print_err(e: DynError) {
    eprintln!("{} {e}", "Error".red().bold());
    if let Some(tip) = e
        .downcast_ref::<blight::Error>()
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
        Led(LedCommand::Toggle { kind, .. }) => match kind {
            ToggleKind::Toggle => "LED toggled",
            ToggleKind::On => "LED toggled on",
            ToggleKind::Off => "LED toggled off",
        },
        Led(LedCommand::Set { .. }) => "LED value set",
        _ => "",
    }
}

fn check_write_perm(device_name: &str, bldir: &str) -> Result<(), std::io::Error> {
    let path = format!("{bldir}/{device_name}/brightness");
    fs::read_to_string(&path)
        .and_then(|contents| fs::write(&path, contents))
        .and(Ok(()))
}



pub fn print_status(device_name: Option<Cow<str>>) -> blight::Result<()> {
    let device = Device::new(device_name)?;

    let write_perm = match check_write_perm(device.name(), BLDIR) {
        Ok(_) => "Ok".green(),
        Err(err) => format!("{err}").red(),
    };

    println!(
        "{}\nDetected device: {}\nWrite permission: {}\nCurrent brightness: {}, {}%\nMax brightness: {}",
        "Device status".bold(),
        device.name().green(),
        write_perm,
        device.current().to_string().green(),
        device.current_percent().round().to_string().green(),
        device.max().to_string().green()
    );
    Ok(())
}

fn parse_led_list_options(args: impl IntoIterator<Item = String>) -> Result<(bool, Option<LedListFilter>), LedError> {
    type Filter = LedListFilter;
    let mut raw = false;
    let mut filter = None;
    let mut args = args.into_iter();
    let err = |msg| Err(LedError::BadListOptions(msg));
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--raw" | "-r" => raw = true,
            "--function" | "-f" if let Some(func) = args.next() && !func.starts_with("--") => match filter {
                None => filter = Some(Filter::Function(func.to_lowercase())),
                Some(Filter::Color(clr)) => filter = Some(Filter::FunctionColor(func.to_lowercase(), clr.to_lowercase())),
                Some(Filter::Function(_)) => return err(Some("function supplied twice")),
                _ => return err(None)
            },
            "--function" | "-f" => return err(Some("missing function")),
            "--color" | "-c" if let Some(color) = args.next() && !color.starts_with("--") => match filter {
                None => filter = Some(Filter::Color(color.to_lowercase())),
                Some(Filter::Function(func)) => filter = Some(Filter::FunctionColor(func.to_lowercase(), color.to_lowercase())),
                Some(Filter::Color(_)) => return err(Some("color supplied twice")),
                _ => return err(None),
            },
            "--color" | "-c" => return err(Some("missing color")),
            "--index" | "-i" if let Some(idx) = args.next().and_then(|idx| idx.parse::<usize>().ok()) => {
                if filter.is_some() {
                    return err(Some("indexing is incompatible with color or function filters"));
                }
                filter = Some(Filter::Index(idx));
            },
            "--index" | "-i" => return err(Some("missing/invalid index")),
            _ => return err(None),
        }
    }
    Ok((raw, filter))
}

fn print_led_list(raw: bool, filter: Option<LedListFilter>) -> blight::Result<()> {
    let mut names = led::led_names()?;
    let mut col_len = 0;
    names.sort_by(|a, b| {
        // Find max raw name length
        col_len = col_len.max(a.raw_name().len().max(b.raw_name().len()));
        // Sort by raw name
        a.raw_name().cmp(b.raw_name())
    });

    let pretty_print_led = |led: &led::LedName| {
        println!(
            "{raw:col_len$}\t[name: {parsed}, color: {color}, function: {fun}]",
            raw = led.raw_name().green(), fun = format!("{:?}", led.function()).yellow(), color = format!("{:?}", led.color()).magenta(), parsed = led.parsed_name().unwrap_or("Unknown").blue()
          );
    };
    if let Some(filter) = filter {
        // NOTE: the function and color filtering is done in a very inefficient way to avoid complexity and additional dependencies.
        // The current implementation is considered good enough as the performance in this context is unlikely to be an issue.
        match filter {
            LedListFilter::Function(function) => 
                names.retain(|n| format!("{:?}", n.function()).to_lowercase().starts_with(&function))
            ,
            LedListFilter::Color(color) => 
                names.retain(|n| format!("{:?}", n.color()).to_lowercase().starts_with(&color))
            ,
            LedListFilter::FunctionColor(function, color) => 
                names.retain(|n| {
                let dbg_str = format!("{:?},{:?}", n.function(), n.color());
                let (fun, clr) = dbg_str.split_once(',').expect("comma doesn't exist in the format string");
                fun.to_lowercase().starts_with(&function) && clr.to_lowercase().starts_with(&color)
            }),
            LedListFilter::Index(idx) => {
                // Replace list of names with a vec containing single element of the requested index or an empty vec if index is out of bounds
                names = idx.checked_sub(1).and_then(|idx| (idx < names.len()).then_some(vec![names.swap_remove(idx)])).unwrap_or_default();
            },
            
        }
    }

    if !raw {
        println!("{}", "Detected LED Devices".bold());
    }
    for (i, led) in names.iter().enumerate() {
        if raw {
            println!("{}", led.raw_name());            
        } else {
            print!("({n}) ", n = i + 1);
            pretty_print_led(led);
        }
    }
    Ok(())
}

pub fn print_led_info(name: &str) -> blight::Result<()> {
    fn print_info(
        led: &impl blight::Light,
        parsed_name: Option<&str>,
        color: Color,
        func: Function,
        dimmable: bool,
    ) {
        let state = if u32::try_from(led.current()).unwrap() == 0 { "off" } else { "on" }.green();
        let write = if let Err(err) = check_write_perm(led.name(), led::LEDDIR) {
            err.to_string().red()
        } else {
            "Ok".green()
        };
        println!(
            "{title}\nName: {name}\nWrite permission: {write}\nState: {state}\nCurrent brightness: {current}\nMax brightness: {max}\nDimmable: {dim}\nParsed name: {parsed}\nColor: {color}\nFunction: {func}",
            title = "LED Device Info".bold(),
            name = led.name().green(),
            current = led.current().to_string().green(),
            max =led.max().to_string().green(), 
            dim = dimmable.to_string().green(),
            parsed =parsed_name.unwrap_or("None").green(), 
            color =format!("{color:?}").green(), 
            func =format!("{func:?}").green() 
        );
    }
    match led::Led::new(name.into())? {
        led::LedType::Dimmable(led) => {
            print_info(&led, led.parsed_name(), led.color(), led.function(), true)
        }
        led::LedType::NonDimmable(led) => {
            print_info(&led, led.parsed_name(), led.color(), led.function(), false)
        }
    }
    Ok(())
}

pub fn print_led_help() {
    let flags = "Flags: raw [--raw, -r], on [--on], off [--off]
Opts: function [--function <name>, -f <name>], color [--color <name>, -c <name>], index [--index <n>, -i <n>]
    Raw shows only the LED device sysfs name without any formatting.
    On and off specify if an LED should be either toggled on or off.
    Function filters LED list function (e.g. 'kbd', 'numlock', 'scrolllock').
    Color filters LED list by color (e.g. 'red', 'white').
    Index selects a specific LED by its position in the list.";
    let commands: String = [
        ("list [flags: raw] [opts: function, color, index]", "-> list all LED devices"),
        ("info <led>", "-> show LED device info"),
        ("set <led> <val>", "-> set LED brightness (0-255)"),
        ("toggle <led> [flags: on, off]", "-> toggle LED state"),
        ("help", "-> display help"),
    ]
    .into_iter()
    .map(|(c, e)| format!("{} {e}\n", c.green().bold()))
    .collect();

    let examples = "\
Examples:
    blight led list (list all LED devices)
    blight led list --function kbd (list keyboard LEDs)
    blight led list --raw (show raw sysfs names)
    blight led info input3::capslock (show LED info)
    blight led set input3::capslock 255 (set LED max brightness)
    blight led toggle input3::capslock --off (turn LED off)
    blight led toggle $(blight led list -i 1 -r) (toggle 1st LED from the list on/off)";

    println!(
        "{t}\n\n{f}\n\n{ct}\n{commands}\n{e}",
        t = "LED Commands".blue().bold(),
        f = flags.magenta(),
        ct = "Commands".bold(),
        e = examples.bright_yellow()
    );
}

pub fn print_led_shelp() {
    let cc: String = [
        ("list", "-> list LED devices"),
        ("info <led>", "-> show LED info"),
        ("set <led> <value>", "-> set LED brightness"),
        ("toggle <led>", "-> toggle LED on/off"),
    ]
    .into_iter()
    .map(|(c, e)| format!("{} {e}\n", c.green().bold()))
    .collect();

    println!(
        "{t}\n{cc}\n{h}",
        t = "LED Commands".bold(),
        h = "Use `blight led help' to display all commands and options".yellow()
    );
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
    let flags = "Flags: sweep [--sweep, -s] Opts: device [--device <name>, -d <name>]
    Sweep lets you increase brightness gradually, resulting in a smooth change.
    Device lets you specify a backlight device target other than the default one.";
    let commands: String = [
        ("inc <val> [flags: sweep] [opts: device]", "-> increase brightness"),
        ("dec <val> [flags: sweep] [opts: device]", "-> decrease brightness"),
        ("set <val> [opts: device]", "-> set custom brightness value"),
        (
            "save [opts: device]",
            "-> save current brightness value to restore later",
        ),
        ("restore", "-> restore saved brightness value\n"),
        (
            "setup",
            "-> installs udev rules and adds user to video group (run with sudo)",
        ),
        ("status [flags: dev]", "-> backlight device status"),
        ("list", "-> list all backlight devices"),
        ("led", "-> list led related commands"),
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
    blight inc 2 -s -d nvidia_0 (increases nvidia_0's brightness smoothly by 2%)
    blight led list --function kbd (list keyboard related LEDs)";

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
        ("inc <val>", "-> increase brightness by given value"),
        ("dec <val>", "-> decrease brightness by given value"),
        ("set <val>", "-> set custom brightness value"),
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
    let mut device = Device::new(Some(device_name.into()))?;

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
    fn report(info: &std::panic::PanicHookInfo) {
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

fn new_locked(name: Option<Cow<str>>) -> Result<Device, DynError> {
    let device = match Device::new_locked(name.clone(), false) {
        Err(err) if *err.kind() == blight::ErrorKind::LockError { blocked: true } => {
            println!(
                "{} Waiting for another instance to finish",
                "Status".magenta().bold(),
            );
            Device::new_locked(name, true)
        }
        other => other,
    }?;

    Ok(device)
}
