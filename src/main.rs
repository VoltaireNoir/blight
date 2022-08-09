use blight::{
    Change::{Regular,Sweep},
    Direction::{Dec,Inc},
};
use std::env::{self,Args};

fn main() {
    if blight::is_running() {
        return;
    }
    argument_parser(env::args());
}

fn argument_parser(mut args: Args) {
    if let Some(arg) = args.next().and_then(|_| args.next()) {
        match &arg[..] {
            "status" => blight::print_status(),
            "list" => blight::print_devices(),
            "set" => {
                if let Some(v) = args.next() { blight::set_bl(&v) }
            },
            "inc" => {
                if let Some(v) = args.next() {
                    blight::change_bl(&v, Regular, Inc)
                } else {
                    blight::change_bl("2", Regular, Inc)
                }
            },
            "dec" => {
                if let Some(v) = args.next() {
                    blight::change_bl(&v, Regular, Dec)
                } else {
                    blight::change_bl("2", Regular, Dec)
                }
            },
            "sweep-up" => {
                if let Some(v) = args.next() {
                    blight::change_bl(&v, Sweep, Inc)
                } else {
                    blight::change_bl("10", Sweep, Inc)
                }
            },
            "sweep-down" => {
                if let Some(v) = args.next() {
                    blight::change_bl(&v, Sweep, Dec)
                } else {
                    blight::change_bl("10", Sweep, Dec)
                }
            },
            _ => blight::print_help(),
        }
    }
}
