# blight
**A simple commandline utility to adjust backlight on Linux systems with hybrid GPU set-up.**

> **Note**
> For this program to run without root privilages, the user needs to be in the video group and might need udev rules to allow write access to brightness files. Read more about it [here](https://wiki.archlinux.org/title/Backlight#ACPI).

### Compile from Source
- Clone repository
- `cd cloned-repo`
- `cargo build -r`

### Commands
- Increase brightness `blight inc` or `blight inc 5` (increases by 5%, instead of default 2%)
- Decrease brightness `blight dec` or `blight dec 10` (decrease by 10%)
- Set custom brightness value `blight set val` | `blight set 50`
- Display help `blight`
