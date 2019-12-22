use std::env;
use util;
use std::collections::HashMap;
use std::process;

#[derive(Debug, Clone)]
pub enum Reference {
    // --enable, --disable
    TAG(String),
    STAGE(String),
    PL_TAG(String, String),
    PL_STAGE(String, String),
    ALL,

    // --execute
    PIPELINE(String),

    // --allow, --forbid
    FLAG(String),
}

pub type Switches = Vec<(String, Vec<Reference>)>; 

pub enum Command {
    VERSION { code: i32 },
    HELP { code: i32 },
    DO_STDIN { switches: Switches },
    DO_FILE { switches: Switches, file: String },
}

fn is_verb(s: &str) -> bool {
    match s {
        "execute" | "allow" | "forbid" | "enable" | "disable" => true,
        _ => false,
    }
}

fn parse_switches(args : env::Args) -> Result<Switches, Command> {
    let mut switches = Switches::new();
    let mut cur_verb : Option<String> = None;
    let mut cur_args = vec![];
    for mut arg in args {
        if arg.starts_with("--") {
            if let Some(ref verb) = cur_verb {
                switches.push((verb.to_string(), cur_args));
                cur_args = vec![];
            }
            let cv = arg.split_off(2).to_string();
            if !is_verb(&cv) {
                return Err(Command::HELP { code: 64 });
            }
            cur_verb = Some(cv);
        }
        else {
            match cur_verb {
                None => {
                    println!("Expected a verb (such as --enable) in the position of the argument {}", arg);
                    process::exit(1);
                },
                Some(ref verb) => {
                    if verb == "execute" {
                        cur_args.push(Reference::PIPELINE(arg));
                    }
                    else if verb == "allow" || verb == "forbid" {
                        cur_args.push(Reference::FLAG(arg))
                    }
                    else if verb == "enable" || verb == "disable" {
                        if arg == "*" {
                            cur_args.push(Reference::ALL);
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
                    else {
                        return Err(Command::HELP { code: 64 });
                    }
                }
            }
        }
    }

    if let Some(verb) = cur_verb {
        switches.push((verb, cur_args));
    }

    Ok(switches)
}

impl Command {
    pub fn new() -> Command {
        let mut args = std::env::args();
        let jann_bin = args.next();
        match args.next() {
            Some(ref arg) => {
                match arg.as_ref() {
                    "--version" => { return Command::VERSION { code: 0 }; },
                    "--help" => { return Command::HELP { code: 64 }; },
                    _ => (),
                }

                match parse_switches(args) {
                    Ok(sw) => {
                        if arg == "--" {
                            return Command::DO_STDIN { switches: sw };
                        }
                        else {
                            return Command::DO_FILE { switches: sw, file: arg.clone() };
                        }
                    },
                    Err(com) => {
                        println!("Invalid command");
                        return com;
                    }
                }
            },
            None => Command::HELP { code: 64 },
        }
    }
}
