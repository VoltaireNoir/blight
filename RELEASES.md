# Version 0.5.0

### Added
- `Device::device_path` method, returns `&Path` to the location of the device in `sys/class/backlight/`
- `Delay` type to customize the write frequency or delay between each write in `Device::sweep_write`
- Custom panic hook (CLI) to print more helpful messages when a panic occurs

### Improved
- `Device::reload` only reloads current value
- `Device::sweep_write` updates brightness changes more efficiently (#2)

### Changed
- Helper function `sweep` is now `Device::sweep_write` (#2)

### Fixed
- Integer overflow while using sweep change (#1)
- `Device::write_value` & `set_bl` silently ignoring or writing values larger than max supported (f30b3c5)
- Stdout and Stderr message formatting inconsistencies

# Version 0.6.0

### Summary
Fixed a major bug related to single instance check, which also changes the CLI behavior slightly. Improved error reporting in case of panics. Most changes in this release only affect the CLI side of things.

### Changed
- CLI no longer returns an error if another instance is running. Instead, it waits for it to finish (#5)
- blight no longer compiles on OSs other than Linux, as they are unsupported

### Improved
- Custom panic handler now properly prints panic related info to the user to help with better bug reports

### Fixed
- CLI falsely reporting that another instance is running (#4)
