use parse::{ParseTree, ParseTreeNode, PTNodeType, Token, TokenType};
use com;
use inter;
use exec;
use deploy;
use util;

use std::fs;
use std::env;
use std::path::PathBuf;
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug)]
enum RunState {
    NOTRUN,
    DONE  ,
}

#[derive(Debug)]
struct PipelineStage<'src> {
    name: &'src str,
    tags: Vec<&'src str>,
    enabled: bool,
    state: RunState,
    pl_ptr: Option<usize>
}

#[derive(Debug)]
struct Pipeline<'src> {
    name  : &'src str,
    stages: Vec<PipelineStage<'src>>,
}

impl<'inv, 'src: 'inv> Pipeline<'src> {
    fn execute(flow: &mut Workflow,
               pl_self: usize,
               inv: &Invocation<'src>,
               symbols: &mut inter::Symbols<'src>,
               log: &mut util::Log<'src>,
               tab: usize,
               ) {
        let tabs = "\t".repeat(tab);
        println!("[Execute] {}{}", tabs, &flow.lines[pl_self].name);
        for st_index in 0..flow.lines[pl_self].stages.len() {
            if !flow.lines[pl_self].stages[st_index].enabled {
                println!("[ Ignore] {} : {}", tabs, flow.lines[pl_self].stages[st_index].name);
                continue;
            }

            if let Some(ptr) = flow.lines[pl_self].stages[st_index].pl_ptr {
                println!("[Running] {} | {}", tabs, flow.lines[pl_self].stages[st_index].name);
                Pipeline::execute(flow, ptr, inv, symbols, log, tab + 1);
            }
            else {
                let name = &flow.lines[pl_self].stages[st_index].name;
                match flow.lines[pl_self].stages[st_index].state {
                    RunState::NOTRUN => {
                        println!("[Execute] {} | {}", tabs, name);
                        if let Some(block_id) = symbols.blocks.get(name) {
                            let mut node: inter::LinkNode = inv.art.node(*block_id);
                            exec::execute_block(inv, symbols, log, &node);
                        }
                        else if let Some((file, entry, sudo)) = symbols.includes.get(*name) {
                            let file = inv.root.join(file).into_os_string().into_string().unwrap_or_else( |_| {
                                log.sys_terminal(&format!("Unable to handle file path {}", file));
                            });
                            let binary = env::current_exe().unwrap_or_else( |_| {
                                log.sys_terminal(&format!("Unable to get jann binary path"));
                            }).into_os_string().into_string().unwrap_or_else( |_| {
                                log.sys_terminal(&format!("Unable to handle binary path"));
                            });
                            let incl_msg = format!("--- Include: {}::{} ---", &file, &entry);
                            println!("{}", incl_msg);
                            let mut proc = if *sudo {
                                Command::new("sudo")
                                    .current_dir(&inv.root)
                                    .arg(binary)
                                    .arg(file)
                                    .arg("--execute")
                                    .arg(entry)
                                    .spawn()
                                    .expect("Failed to run included Jannfile")
                            }
                            else {
                                Command::new(binary)
                                    .current_dir(&inv.root)
                                    .arg(file)
                                    .arg("--execute")
                                    .arg(entry)
                                    .spawn()
                                    .expect("Failed to run included Jannfile")
                            };
                            if !proc.wait().expect("Failed to wait on Jann").success() {
                                println!("{}", "-".repeat(incl_msg.len()));
                                log.die();
                            };
                            println!("{}", "-".repeat(incl_msg.len()));
                        }
                        else {
                            log.sys_terminal(&format!("No such block {}", name));
                        }
                    },
                    RunState::DONE   => {
                        println!("[   Done] {} * {}", tabs, name);
                    },
                }
            }
            flow.lines[pl_self].stages[st_index].state = RunState::DONE;
        }
    }
}

struct Workflow<'src> {
    lines: Vec<Pipeline<'src>>,
    index : HashMap<&'src str, usize>,
    // filepath?
}

impl<'inv, 'src: 'inv> Workflow<'src> {
    fn new() -> Workflow<'src> {
        Workflow { lines: vec![], index: HashMap::new() }
    }

    fn execute(&mut self, inv: &Invocation<'src>, symbols: &mut inter::Symbols<'src>, log: &mut util::Log<'src>) {
        let mut main_line = self.index.get(inv.pl_name.as_str()).unwrap_or_else( | | {
            log.sys_terminal("No such pipeline exists.");
        });
        Pipeline::execute(self, *main_line, inv, symbols, log, 0);
    }
}

pub struct Invocation<'src> {
    pub root : PathBuf,
    pub edir : PathBuf,
    pub opts : deploy::DepOpt,
    pub pl_name : String, 
    pub art  : inter::Artifact<'src>,
    pub switches: com::Switches,
}


impl<'inv, 'src: 'inv> Invocation<'src> {
    pub fn invoke(self, log: &'inv mut util::Log<'src>) {
        let cwd = env::current_dir().unwrap_or_else( | _ | {
            log.sys_terminal("Could not get cwd.");
        });

        if !self.edir.exists() {
            fs::create_dir_all(&self.edir).unwrap_or_else( | _ | {
                log.sys_terminal("Unable to create execution dir");
            });
        }

        env::set_current_dir(&self.edir).unwrap_or_else( | _ | {
            log.sys_terminal(
                &format!("Could not change working directory to {:?}.", &self.edir)
            );
        });

        let mut symbols = inter::Symbols::new();

        let root = self.art.root();
        let mut flow = Workflow::new();

        // Populate the symbol table and build the workflow by walking
        // through the top level nodes of the parse tree

        fn parse_extern(node: &inter::LinkNode, log: &mut util::Log) -> Option<(String, String)> {
            let parts = node.token_value().split("::").collect::<Vec<&str>>();
            match parts.len() {
                1 => Some((parts[0].to_string(), "main".to_string())),
                2 => Some((parts[0].to_string(), parts[1].to_string())),
                _ => { log.error("Bad directive", "Too many '::'", node.tok); None },
            } 
        }

        for child in root.children() {
            if child.is_type(&PTNodeType::DIRECTIVE) {
                let verb = &child.children()[0];
                let data = &child.children()[1];
                if !verb.is_type(&PTNodeType::NAME) {
                    log.error("Invalid directive verb", "This needs to be a name", verb.tok);
                    continue;
                }
                match verb.token_value() {
                    v @ "include" | v @ "sudo_include" => {
                        let (file, entry, symbol) = match data.ptn.nt {
                            PTNodeType::NAME => {
                                let (file, entry) = match parse_extern(&data, log) { Some(t) => t, None => { continue; } };
                                (file, entry.clone(), entry)
                            },
                            PTNodeType::LIST => {
                                if data.children().len() != 2 {
                                    log.error("Bad list argument to include directive",
                                              "Should be two values here", data.tok);
                                }
                                if !data.children().iter().all(|c| c.is_type(&PTNodeType::NAME)) {
                                    log.error("Bad value in list argument for include directive",
                                              "These values need all be names", data.tok);
                                }
                                if let Some((file, entry)) = parse_extern(&data.children()[0], log) {
                                    (file, entry, data.children()[1].token_value().to_string())
                                }
                                else {
                                    continue;
                                }
                            },
                            _ => {
                                unimplemented!();
                            }
                        };
                        symbols.includes.insert(symbol, (file, entry, v == "sudo_include"));
                    },
                    _ => {},
                }
                continue;
            }

            let tag = &child.children()[0];
            if tag.is_type(&PTNodeType::NAME) {
                symbols.blocks.insert(tag.token_value(), child.ptn.id);
            }

            if child.is_type(&PTNodeType::PIPELINE) {
                let pl_children = child.children();

                let pl_name = &pl_children[0].token_value();

                if !util::check_name(pl_name) {
                    log.error("Invalid pipeline name", "Make this a valid pipeline name", &pl_children[0].tok);
                    continue;
                }

                let pl_list = &pl_children[1];
                let mut stages = vec![];

                for stage in pl_list.children() {
                    if !stage.is_type(&PTNodeType::NAME) {
                        log.error("Invalid stage name", "Make this a name", stage.tok);
                        continue;
                    }
                    if !util::check_name(stage.token_value()) {
                        log.error("Invalid stage name", "Make this a valid stage name", stage.tok);
                        continue;
                    }

                    let mut enabled = false;
                    let mut tags = vec![]; 
                    for child in stage.children() {
                        if child.is_type(&PTNodeType::FLAG) {
                            enabled = true;
                        }
                        else if child.is_type(&PTNodeType::LIST) {
                            for tag in child.children() {
                                match tag.ptn.nt {
                                    PTNodeType::NAME => { tags.push(tag.token_value()) },
                                    _ => { log.error("Invalid tag", "Make this a valid tag name", tag.tok) },
                                }
                            }
                        }
                    }

                    stages.push(PipelineStage {
                        name: stage.token_value(),
                        tags: tags,
                        enabled: enabled,
                        state: RunState::NOTRUN,
                        pl_ptr: None,
                    });
                }
                flow.index.insert(pl_name, flow.lines.len());
                flow.lines.push(Pipeline { name: pl_name, stages });
            }
        }

        let mut enable_set : Vec<(com::Reference, bool)> = vec![];

        for (com, refs) in &self.switches {
            match com.as_ref() {
                "enable" => refs.iter().for_each(|r| enable_set.push((r.clone(), true))),
                "disable" => refs.iter().for_each(|r| enable_set.push((r.clone(), false))),
                _ => (),
            }
        }

        for (r, val) in enable_set {
            match r {
                com::Reference::TAG(ref rtag) => {
                    for pl in &mut flow.lines {
                        for stage in &mut pl.stages {
                            if stage.tags.contains(&rtag.as_str()) {
                                stage.enabled = val;
                            }
                        }
                    }
                },
                com::Reference::STAGE(ref rstage) => {
                    for pl in &mut flow.lines {
                        for stage in &mut pl.stages {
                            if *rstage == stage.name {
                                stage.enabled = val;
                            }
                        }
                    }
                },
                com::Reference::PL_TAG(ref pl, ref rtag) => {
                    if let Some(pl_ind) = flow.index.get(pl.as_str()) {
                        for stage in &mut flow.lines[*pl_ind].stages {
                            if stage.tags.contains(&rtag.as_str()) {
                                stage.enabled = val;
                            }
                        }
                    }
                },
                com::Reference::PL_STAGE(ref pl, ref rstage) => {
                    if let Some(pl_ind) = flow.index.get(pl.as_str()) {
                        for stage in &mut flow.lines[*pl_ind].stages {
                            if stage.name == *rstage {
                                stage.enabled = val;
                            }
                        }
                    }
                },
                com::Reference::ALL => {
                    for pl in &mut flow.lines {
                        for stage in &mut pl.stages {
                            stage.enabled = val;
                        }
                    }
                },
                _ => unreachable!(),
            }
        }


        for pl in &mut flow.lines {
            for stage in &mut pl.stages {
                match flow.index.get(&stage.name) {
                    Some(nxt_pl) => { (*stage).pl_ptr = Some(*nxt_pl); },
                    None => (),
                }
            }
        }
        
        flow.execute(&self, &mut symbols, log);

        env::set_current_dir(&cwd).unwrap_or_else( | _ | { 
            log.sys_terminal("Could not change CWD!");
        });
    }
}
