# blight
[![Rust](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml/badge.svg)](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml)
#### _A hassle-free CLI utility to manage backlight on Linux laptops with hybrid GPU configuration._
![](blight.png)
> **Note**
> For this program to run without root privileges, the user needs to be in the video group and might need udev rules to allow write access to brightness files. Read more about it [here](https://wiki.archlinux.org/title/Backlight#ACPI).
## About
A lot of Linux backlight utilities often fail to detect the right backlight device to control in laptops that ship with Intel or Amd iGPUs and an Nvidia dGPU with proprietary drivers. This utility aims to solve that problem by prioritizing integrated graphic devices, followed by dedicated Nvdia GPU and ACPI kernel module. This means that you do not have to manually specify which device is currently active whenever you switch between your iGPU and dGPU using the MUX switch. Other than that, blight also implements sweep-up and sweep-down option, which lets you change brightness in a smooth sweeping manner, rather than in sudden jerky increments/decrements.

In principle, blight should work on any GNU/Linux distro, and even on systems without hybrid GPU configuration. However, it has only been tested on Arch and Fedora so far. Any feedback and bug reports will be greatly appreciated.

## Usage
Set custom shortcuts using your distro settings or pair it with a hotkey daemon like [sxhkd](https://github.com/baskerville/sxhkd) and you'll be good to go. blight doesn't execute any code if another instance is already running, so do not worry about spamming the key that triggers it.

### Commands
- Increase brightness `blight inc` | `blight inc 5` (increases by 5%, instead of default 2%)
- Decrease brightness `blight dec` | `blight dec 10` (decrease by 10%)
- Set custom brightness value `blight set val` | `blight set 50`
- Increase brightness smoothly `blight sweep-up` | `blight sweep-up 20`
- Decrease brightness smoothly `blight sweep-down` | `blight sweep-down 20`
- Display status `blight status`
- Display help `blight`

## Install
### Using Cargo
- `cargo install blight`
- Binary will be compiled to `.cargo/bin`

### Compile from Source
- Clone repository
- `cd cloned-repo`
- `cargo build -r`
