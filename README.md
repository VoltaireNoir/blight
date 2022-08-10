# blight
[![Rust](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml/badge.svg)](https://github.com/VoltaireNoir/blight/actions/workflows/rust.yml)
[![Crates.io](https://img.shields.io/crates/v/blight)](https://crates.io/crates/blight)
#### _A hassle-free CLI utility to manage backlight on Linux; one that plays well with hybrid GPU configuration and proprietary drivers._

> "And man said, 'let there b-light' and there was light." - Some Book 1:3

![Generated using Dall-E 2](blightm.png)
*(Credits: Sneha Sundar, for generating this lovely image for blight using DALL·E 2)*
## Screenshots
![](blight.png)
![](blight2.png)
## About
> **Warning**
> For this program to run without root privileges, the user needs to be in the video group and might need udev rules to allow write access to brightness files. Read more about it [here](https://wiki.archlinux.org/title/Backlight#ACPI). If you do not have write permissions, then you'd have to run the program with `sudo`.

A lot of Linux backlight utilities often fail to detect the right backlight device to control in laptops that ship with Intel or Amd iGPUs and an Nvidia dGPU with proprietary drivers. This utility aims to solve that problem by prioritizing integrated graphic devices, followed by dedicated Nvdia GPU and ACPI kernel module. This means that you do not have to manually specify which device is currently active whenever you switch between your iGPU and dGPU using the MUX switch. Other than that, *blight* also implements sweep-up and sweep-down option, which lets you change brightness in a smooth sweeping manner, rather than applying sudden jerky increments/decrements.

In principle, blight should work on any GNU/Linux distro, and even on systems without hybrid GPU configuration. However, it has only been tested on Arch and Debian so far. Any feedback and bug reports will be greatly appreciated.

## Usage
Set custom shortcuts using your distro settings or pair it with a hotkey daemon like [sxhkd](https://github.com/baskerville/sxhkd) and you'll be good to go. *blight* doesn't execute any code if another instance is already running, so do not worry about spamming the key that triggers it.

### Commands
- Display status `blight status` OR `blight status device_name`
- Display help `blight`
- List all backlight devices `blight list`
- Increase brightness `blight inc` OR `blight inc 5` (increases by 5%, instead of default 2%)
- Decrease brightness `blight dec` OR `blight dec 10` (decrease by 10%)
- Increase brightness smoothly `blight sweep-up` OR `blight sweep-up 20`
- Decrease brightness smoothly `blight sweep-down` OR `blight sweep-down 20`
- Set custom brightness value ``blight set val``
- Increase brightness for specific device `blight inc 2 nvidia_0`

Note: When specifying a device name, you need to specify optional values too if the command accepts them (see above example).

## Install
### Using Cargo
- `cargo install blight`
- Binary will be compiled to `$HOME:.cargo/bin`

### Compile from Source
- Clone repository
- `cd cloned-repo`
- `cargo build -r`

## Contribute
Coding, for me, is a hobby and I'm very much new to Rust and to programming as a whole. So if you notice anything in the code that can be improved, do open an issue to voice your opinion and pass on your suggestions. If you want to improve the code directly, please raise a pull-request, I'd be happy to collaborate and work to improve this together. Cheers!

