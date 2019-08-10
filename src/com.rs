use std::env;
use util;

pub struct Switches {
}

pub enum Command {
    VERSION { code: i32 },
    HELP { code: i32 },
    DO_STDIN { switches: Switches },
    DO_FILE { switches: Switches, file: String },
}


impl Command {
    pub fn new() -> Command {
        let mut args = std::env::args();
        let jann_bin = args.next();
        match args.next() {
            Some(ref arg) => {
                match arg.as_ref() {
                    "--version" => Command::VERSION { code: 0 },
                    "--help" => Command::HELP { code: 64 },
                    "--" => Command::DO_STDIN { switches: Switches {} },
                    _ => Command::DO_FILE { switches: Switches {}, file: arg.clone() }
                }
            },
            None => Command::HELP { code: 64 },
        }
    }
}
