use std::env;
use std::fs;
const BLDIR: &str = "/sys/class/backlight";

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match &args[1..] {
            [dir,step_size] if dir == "inc" => change_bl(step_size, dir),
            [dir,step_size] if dir == "dec" => change_bl(step_size, dir),
            [dir] if dir == "inc" => change_bl("2", dir),
            [dir] if dir == "dec" => change_bl("2",dir),
            [case] if case == "help" => print_help(),
            _ => ()
        }
    }
}

fn get_device() -> String {
    let dirs = fs::read_dir(BLDIR)
        .expect("Failed to read backglight dir");

    for d in dirs {
        let p: String = d.unwrap().file_name().to_string_lossy().to_string();

        if ["amdgpu_bl0","amdgpu_bl1","acpi_video0"].contains(&p.as_str()) {
            return p;
        }
    }

    String::from("nvidia_0")
}

fn get_max(device: &str) -> u16 {
    let max: u16 = fs::read_to_string(format!("{BLDIR}/{device}/max_brightness"))
        .expect("Failed to read max value")
        .trim()
        .parse()
        .expect("Failed to parse max value");
    max
}

fn get_current(device: &str) -> u16 {
    let current: u16 = fs::read_to_string(format!("{BLDIR}/{device}/brightness"))
        .expect("Failed to read max value")
        .trim()
        .parse()
        .expect("Failed to parse max value");
    current
}

fn calculate_change(current: u16, max: u16, step_size: u16, dir: &str) -> u16 {
    let step: u16 = (max as f32 * (step_size as f32 / 100.0 )) as u16;
    println!("calc step {step}");

    let change: u16 = if dir == "dec" {current.saturating_sub(step)} else {current.saturating_add(step)};
    println!("pref change {change}");

    if change > max {
        max
    } else {
        change
    }

}

fn change_bl(step_size: &str, dir: &str) {
    let step_size: u16 = step_size.parse().expect("Invalid step size");
    let device = get_device();
    let current = get_current(&device);
    let max = get_max(&device);
    let change = calculate_change(current, max, step_size, dir);
    println!("change {change}");
    if change != current {
        fs::write(format!("{BLDIR}/{device}/brightness"), format!("{change}")).expect("Couldn't write to brightness file");
    }
}

fn print_help() {
    let help_str = "
    blight uses brightnessctl to find current device, and updates brightness accordingly.\n
    blight dec [optional step size] - decrease brightness
    blight inc [optional step size] - increase brightness\n
    Example: blight inc 10 (increases brightness by 10%)
    ";
    println!("{help_str}")
}
