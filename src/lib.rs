use std::fs;
use futures::executor::block_on;
const BLDIR: &str = "/sys/class/backlight";

pub enum Direction {
    Inc,
    Dec
}

pub enum Change {
    Sweep,
    Regular,
}

pub struct Device {
    pub name: String,
    pub current: u16,
    pub max: u16,
}

impl Device {

    pub fn new() -> Device {
        block_on(Device::load())
    }

    async fn load() -> Device {
        let name = Self::detect_device().expect("No known backlight devices found on system");

        Device { current: Self::get_current(&name).await,
                 max: Self::get_max(&name).await,
                 name,
        }
    }

    fn detect_device() -> Option<String> {

        let dirs = fs::read_dir(BLDIR)
            .expect("Failed to read backglight dir");
        let mut nv: bool = false;
        for d in dirs {
            let p: String = d.unwrap().file_name().to_string_lossy().to_string();

            if !nv { if p.contains("nvidia") { nv = true }; };

            if ["amdgpu_bl0","amdgpu_bl1","acpi_video0","intel_backlight"].contains(&p.as_str()) {
                return Some(p);
            };
        }

        if nv { Some(String::from("nvidia_0")) } else { None }
    }

    async fn get_max(device: &str) -> u16 {
        let max: u16 = fs::read_to_string(format!("{BLDIR}/{device}/max_brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        max
    }

    async fn get_current(device: &str) -> u16 {
        let current: u16 = fs::read_to_string(format!("{BLDIR}/{device}/brightness"))
            .expect("Failed to read max value")
            .trim()
            .parse()
            .expect("Failed to parse max value");
        current
    }

    pub fn write_value(&self, value: u16) {
        fs::write(format!("{BLDIR}/{}/brightness",self.name), format!("{value}"))
            .expect("Couldn't write to brightness file");
    }

}
