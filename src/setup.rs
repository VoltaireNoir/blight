use colored::*;
use std::{
    io::ErrorKind,
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
                         "Permission denied. Run `blight setup` with sudo.".red())
            } else {
                println!("{} {}","Error:".red(),err);
            }
        }
    }
    print!("Video Group: ");
    match setup_group() {
        GroupResult::Exists => println!("{}","Ok (already in group)".green()),
        GroupResult::Err => println!("{}","Failed to add user to video group".red()),
        GroupResult::Ok => println!("{}","Ok".green())
    }
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
    Err,
}

fn setup_group() -> GroupResult {
    let user = String::from_utf8(
        process::Command::new("logname")
            .output()
            .unwrap()
            .stdout)
        .unwrap();

    String::from_utf8(
        process::Command::new("groups")
            .arg(user.trim())
            .output()
            .unwrap()
            .stdout)
        .unwrap()
        .contains("video")
        .then_some(GroupResult::Exists)
        .unwrap_or(GroupResult::Err)
}
