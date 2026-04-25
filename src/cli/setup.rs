//! This module helps set up necessary udev rules for blight or the current user to gain write permission
//! to the brightness file in /sys/class/backlight/<device>/brightness \n
//! The write permission and ownership of the brightness file is assigned to the video group through the udev rules.
//! The user is then added to the video group if they're not in the group already.

use colored::*;
use std::{
    error::Error,
    fs,
    io::{self, ErrorKind},
    path::PathBuf,
    process,
};

const RULES: &str = r#"# Backlight controls
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chgrp video /sys/class/backlight/%k/brightness"
ACTION=="add", SUBSYSTEM=="backlight", RUN+="/bin/chmod g+w /sys/class/backlight/%k/brightness"

# LED controls
ACTION=="add", SUBSYSTEM=="leds", RUN+="/bin/chgrp video /sys/class/leds/%k/brightness"
ACTION=="add", SUBSYSTEM=="leds", RUN+="/bin/chmod g+w /sys/class/leds/%k/brightness""#;

const UDEVFILE: &str = "/lib/udev/rules.d/90-blight.rules";

/// The function runs the setup. The udev file 90-blight.rules is placed in /lib/udev/.udev.rules.d/.
/// The user is added to the 'video' group if they're not already in it.
pub fn run() {
    println!("{}", "Running Setup".bold());
    print!("UDEV Rules: ");
    match setup_rules() {
        Ok(RulesStatus::WriteOk) => println!("{}", "Ok".green()),
        Ok(RulesStatus::AlreadyExists) => println!("{}", "Ok (already in place)".green()),
        Err(err) => {
            if err.kind() == ErrorKind::PermissionDenied {
                println!("{}", "Failed. Run `blight setup` with sudo.".red())
            } else {
                println!("{} {}", "Error:".red(), err);
            }
        }
    }
    print!("Video Group: ");
    match setup_group() {
        GroupResult::Exists => println!("{}", "Ok (already in group)".green()),
        GroupResult::Err(err) => println!("{} {}", "Error:".red(), err),
        GroupResult::UnknownErr => println!("{}", "Failed. Run `blight setup` with sudo.".red(),),
        GroupResult::Ok => println!("{}", "Ok".green()),
    }

    println!(
        "{}\n{}",
        "Recommended: Reboot your system once the setup completes successfully.".yellow(),
        "You can run `blight status` to check if you have gained write permissions.".yellow()
    );
}

enum RulesStatus {
    WriteOk,
    AlreadyExists,
}

fn setup_rules() -> std::io::Result<RulesStatus> {
    let path = PathBuf::from(UDEVFILE);
    if path.exists() && fs::read_to_string(&path)? == RULES {
        return Ok(RulesStatus::AlreadyExists);
    }
    fs::write(UDEVFILE, RULES)?;
    Ok(RulesStatus::WriteOk)
}

enum GroupResult {
    Ok,
    Exists,
    Err(Box<dyn Error>),
    UnknownErr,
}

fn setup_group() -> GroupResult {
    let user =
        String::from_utf8(process::Command::new("logname").output().unwrap().stdout).unwrap();

    if in_group(&user) {
        return GroupResult::Exists;
    }

    if fs::read_to_string("/etc/group").unwrap().contains("video") {
        if let Err(err) = add_to_group(&user) {
            return GroupResult::Err(Box::new(err));
        }
    } else {
        if let Err(err) = process::Command::new("groupadd")
            .arg("video")
            .stderr(process::Stdio::null())
            .output()
        {
            return GroupResult::Err(Box::new(err));
        }
        if let Err(err) = add_to_group(&user) {
            return GroupResult::Err(Box::new(err));
        }
    }

    if in_group(&user) {
        GroupResult::Ok
    } else {
        GroupResult::UnknownErr
    }
}

fn in_group(user: &str) -> bool {
    String::from_utf8(
        process::Command::new("groups")
            .arg(user.trim())
            .output()
            .expect("Failed to run groups command")
            .stdout,
    )
    .unwrap()
    .contains("video")
}

fn add_to_group(user: &str) -> Result<(), io::Error> {
    process::Command::new("usermod")
        .args(["-aG", "video", user.trim()])
        .stderr(process::Stdio::null())
        .output()?;
    Ok(())
}
