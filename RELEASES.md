# Version 0.7.1

### Summary
A minor bug fix release that changes the behavior of how errors are handled while acquiring a lock.
Note: This release only contains changes to the CLI.

### Fixed
- Handle errors while acquiring a lock instead of panicking (#9)

# Version 0.7.0

### Summary
This release contains some breaking changes for the library users, like the type change from `u16 `to `u32` as part of a bug fix. Other than that, a `current_percent` method has been added to `Device` for convenience, and there's a minor CLI related change. The rest of the changes are all memory usage improvements and code refactoring for improved maintainability.

### Added
- `Device::current_percent` method that returns brightness percentage (in contrast with `current` which returns the raw current value)

### Changed
- All functions and methods that took and returned `u16` now use `u32` (breaking change)
- `device_path` method now returns a `PathBuf `instead of `&Path` due to internal code changes (breaking change)
- `blight status` now prints brightness percentage along with the raw value

### Improved
- Significant reduction of heap allocations
- Reduced code duplication and code refactored for maintainability

### Fixed
- blight failing to work with devices that may use values larger than `u16::MAX` - #6 (Thanks pdamianik)

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

