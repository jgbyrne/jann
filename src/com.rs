use std::env;
use util;
use std::collections::HashMap;
use std::process;

#[derive(Debug, Clone)]
pub enum Reference {
    TAG(String),
    STAGE(String),
    PL_TAG(String, String),
    PL_STAGE(String, String),
}

pub type Switches = Vec<(String, Vec<Reference>)>; 

pub enum Command {
    VERSION { code: i32 },
    HELP { code: i32 },
    DO_STDIN { switches: Switches },
    DO_FILE { switches: Switches, file: String },
}


fn parse_switches(args : env::Args) -> Switches {
    let mut switches = Switches::new();
    let mut cur_verb = None;
    let mut cur_args = vec![];
    for mut arg in args {
        if arg.starts_with("--") {
            if let Some(verb) = cur_verb {
                switches.push((verb, cur_args));
                cur_args = vec![];
            }
            cur_verb = Some(arg.split_off(2).to_string());
        }
        else {
            if cur_verb.is_none() {
                println!("Expected a verb (such as --enable) in the position of the argument {}", arg);
                process::exit(1);
            }

            if arg.starts_with("%") {
                cur_args.push(Reference::TAG(arg.split_off(1).to_string()));
            }
            else {
                let parts = arg.split(".").collect::<Vec<&str>>();
                if parts.len() == 1 {
                    cur_args.push(Reference::STAGE(arg));
                }
                else {
                    if parts[1].starts_with("%") {
                        cur_args.push(Reference::PL_TAG(parts[0].to_string(),
                                                        parts[1].to_string()
                                                                .split_off(1)
                                                                .to_string()));
                    }
                    else {
                        cur_args.push(Reference::PL_STAGE(parts[0].to_string(),
                                                          parts[1].to_string()));
                    }
                }
            }
        }
    }

    if let Some(verb) = cur_verb {
        switches.push((verb, cur_args));
    }

    switches
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
                    "--" => Command::DO_STDIN { switches: parse_switches(args) },
                    _ => Command::DO_FILE { switches: parse_switches(args), file: arg.clone() }
                }
            },
            None => Command::HELP { code: 64 },
        }
    }
}
