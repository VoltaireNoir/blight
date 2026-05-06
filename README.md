# blight
<div align="center">

![](blight.jpg)

[![Rust](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml/badge.svg)](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/blight)](https://crates.io/crates/blight)
[![Downloads](https://img.shields.io/crates/d/blight)](https://crates.io/crates/blight)
![License](https://img.shields.io/crates/l/blight)

</div>

> "And man said, 'let there b-light' and there was light." - Some Book 1:3

**blight** is primarily a CLI backlight utility for Linux focused on providing hassle-free backlight control. However, the parts which blight relies on to make backlight changes, are also exposed through the library aspect of this crate, which can be used like any other Rust library by using the command `cargo add blight --no-default-features` in your Rust project. The CLI utility, on the other hand, can be installed by running `cargo install blight`.

*The latest version (>=0.8.0) now supports controlling LEDs using the `/sys/class/leds` interface. Refer to [changelog](RELEASES.md) for a list of all changes.*

**Three features of blight that standout**:
1. Prioritizing backlight device detection in this order: iGPU>dGPU>ACPI>Fallback device
    - Useful for machines with a hybrid GPU setup
2. Smooth dimming by writing in increments/decrements of 1 with a few milliseconds of delay
    - `blight inc 5 --sweep`
3. Minimal reliance on external dependencies
    - The library has zero dependencies
    - The CLI only has a single direct external dependency

> **Note**
> This page contains documentation for the CLI. For library docs, visit [docs.rs](https://docs.rs/blight/). The latest version of the library now also supports LEDs.

> **Warning**
> For this program to run without root privileges, the user needs to be in the video group and might need udev rules to allow write access to brightness files. Read more about it [here](https://wiki.archlinux.org/title/Backlight#ACPI). You can gain required permissions by using the helper script that comes with blight by running `sudo blight setup` once or you could do it manually too. If not, you'd have to run the program with `sudo` every time.

## Demo
<p align="center">
  <img src="https://raw.githubusercontent.com/VoltaireNoir/blight/main/demo.gif" alt="CLI Demo">
</p>

## Usage
Set custom shortcuts using your distro settings or pair it with a hotkey daemon like [sxhkd](https://github.com/baskerville/sxhkd) and you'll be good to go. *blight* doesn't execute any code if another instance is already running, so do not worry about spamming the key that triggers it.

### Commands
- Display help `blight` (quick help) or `blight help`
- Display status `blight status` OR `blight status -d device_name`
- Run first time setup script (for write permissions) `sudo blight setup`
- List all backlight devices `blight list`
- Increase brightness `blight inc 5` (increase by 5%)
- Decrease brightness `blight dec 10` (decrease by 10%)
- Increase/decrease brightness smoothly `blight inc 10 -s` OR `blight dec 10 --sweep`
- Set custom brightness value `blight set 50`
- Increase brightness for specific device `blight inc 2 -d nvidia_0`
- Save brightness `blight save` OR `blight save -d amdgpu_bl0`
- Restore brightness `blight restore`
- Display LED help `blight led` (quick help) or `blight led help`

## Install
### Using Cargo
- `cargo install blight`
- Binary will be compiled to `$HOME:.cargo/bin`

### Compile from Source
- Clone repository
- `cd cloned-repo`
- `cargo build -r`

### Pre-built Binary
- Pre-built binaries are availabe for `x86-64` and `ARM64` Linux in the [releases section](https://github.com/VoltaireNoir/blight/releases).

## Contribute
All contributions are welcome and appreciated. Want to implement a new feature or have a feature request? Create an issue. If you've implemented it already and think it is a meaningful addition, it is perfectly fine to create a PR directly. Cheers 🍻
