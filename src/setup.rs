use colored::*;
use std::{
    error::Error,
    io::{self, ErrorKind},
    path::PathBuf,
    process,
    fs,
};

const RULES: &str = r#"ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video /sys/class/backlight/%k/brightness"
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chmod g+w /sys/class/backlight/%k/brightness""#;
const UDEVFILE: &str = "/lib/udev/rules.d/90-blight.rules";

pub fn run() {
    println!("{}","Running Setup".bold());
    print!("UDEV Rules: ");
    match setup_rules() {
        RulesResult::Ok => println!("{}","Ok".green()),
        RulesResult::Exists => println!("{}","Ok (already in place)".green()),
        RulesResult::Err(err) => {
            if err.kind() == ErrorKind::PermissionDenied {
                println!("{}",
                         "Failed. Run `blight setup` with sudo.".red())
            } else {
                println!("{} {}","Error:".red(),err);
            }
        }
    }
    print!("Video Group: ");
    match setup_group() {
        GroupResult::Exists => println!("{}","Ok (already in group)".green()),
        GroupResult::Err(err) => println!("{} {}","Error:".red(),err),
        GroupResult::UnknownErr => println!("{}", "Failed. Run `blight setup` with sudo.".red(),),
        GroupResult::Ok => println!("{}","Ok".green())
    }

    println!("{}\n{}",
             "Recommended: Reboot your system once the setup completes successfully.".yellow(),
             "You can run `blight status` to check if you have gained write permissions.".yellow());
}

enum RulesResult {
    Ok,
    Exists,
    Err(std::io::Error)
}

fn setup_rules() -> RulesResult {
    let path = PathBuf::from(UDEVFILE);
    if path.exists() && fs::read_to_string(&path).unwrap().contains(RULES) {
        return RulesResult::Exists
    }
    if let Err(err) = fs::write(UDEVFILE, RULES) {
        return RulesResult::Err(err)
    }
    RulesResult::Ok
}

enum GroupResult {
    Ok,
    Exists,
    Err(Box<dyn Error>),
    UnknownErr,
}

fn setup_group() -> GroupResult {
    let user = String::from_utf8(
        process::Command::new("logname")
            .output()
            .unwrap()
            .stdout)
        .unwrap();

    if in_group(&user) {
        return GroupResult::Exists
    }

    if fs::read_to_string("/etc/group")
        .unwrap()
        .contains("video") {
            if let Err(err) = add_to_group(&user) {
                    return GroupResult::Err(Box::new(err))
                }
        } else {
            if let Err(err) = process::Command::new("groupadd")
                .arg("video")
                .stderr(process::Stdio::null())
                .output() {
                    return GroupResult::Err(Box::new(err))
                }
            if let Err(err) = add_to_group(&user) {
                    return GroupResult::Err(Box::new(err))
                }
        }

    in_group(&user)
        .then_some(GroupResult::Ok)
        .unwrap_or(GroupResult::UnknownErr)
}

fn in_group(user: &str) -> bool {
    String::from_utf8(
        process::Command::new("groups")
            .arg(user.trim())
            .output()
            .expect("Failed to run groups command")
            .stdout)
        .unwrap()
        .contains("video")
}

fn add_to_group(user: &str) -> Result<(), io::Error> {
    process::Command::new("usermod")
        .args(["-aG","video",user.trim()])
        .stderr(process::Stdio::null())
        .output()?;
    Ok(())
}
