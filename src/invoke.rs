use parse::{ParseTree, ParseTreeNode, PTNodeType, Token, TokenType};
use inter;
use exec;
use deploy;
use util;

use std::fs;
use std::env;
use std::path::PathBuf;
use std::collections::HashMap;

#[derive(Debug)]
enum RunState {
    NOTRUN,
    DONE  ,
}

#[derive(Debug)]
struct PipelineStage {
    name: String,
    enabled: bool,
    state: RunState,
    pl_ptr: Option<usize>
}

#[derive(Debug)]
struct Pipeline {
    name  : String,
    stages: Vec<PipelineStage>,
}

impl Pipeline {
    fn execute<'inv, 'src: 'inv>(flow: &mut Workflow,
                                 pl_self: usize,
                                 inv: &Invocation<'src>,
                                 symbols: &mut inter::Symbols<'src>,
                                 log: &mut util::Log<'src>,
                                 ) {
        println!("Executing pipeline '{}'", &flow.lines[pl_self].name);
        for st_index in 0..flow.lines[pl_self].stages.len() {
            if !flow.lines[pl_self].stages[st_index].enabled {
                println!("Ignoring disabled stage '{}'...", flow.lines[pl_self].stages[st_index].name);
                continue;
            }

            if let Some(ptr) = flow.lines[pl_self].stages[st_index].pl_ptr {
                Pipeline::execute(flow, ptr, inv, symbols, log);
            }
            else {
                let name = &flow.lines[pl_self].stages[st_index].name;
                match flow.lines[pl_self].stages[st_index].state {
                    RunState::NOTRUN => {
                        println!("Executing pipeline stage '{}'...", name);
                        let block_id = *symbols.blocks.get(name.as_str()).unwrap();
                        let mut node: inter::LinkNode = inv.art.node(block_id);
                        exec::execute_block(inv, symbols, log, &node);
                    },
                    RunState::DONE   => {
                        println!("Already done stage '{}', skipping...", name);
                    },
                }
            }
            flow.lines[pl_self].stages[st_index].state = RunState::DONE;
        }
    }
}

struct Workflow {
    lines: Vec<Pipeline>,
    index : HashMap<String, usize>,
    // filepath?
}

impl Workflow {
    fn new() -> Workflow {
        Workflow { lines: vec![], index: HashMap::new() }
    }

    fn execute<'inv, 'src: 'inv>(&mut self, inv: &Invocation<'src>, symbols: &mut inter::Symbols<'src>, log: &mut util::Log<'src>) {
        let mut main_line = self.index.get(&inv.pl_name).unwrap();
        Pipeline::execute(self, *main_line, inv, symbols, log);
    }
}

pub struct Invocation<'src> {
    pub root : PathBuf,
    pub edir : PathBuf,
    pub opts : deploy::DepOpt,
    pub pl_name : String, 
    pub art  : inter::Artifact<'src>,
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

        // Recursively produce list of execution stages
        let root = self.art.root();
        let mut flow = Workflow::new();

        for child in root.children() {
            let tag = &child.children()[0];
            if tag.is_type(&PTNodeType::NAME) {
                symbols.blocks.insert(tag.token_value(), child.ptn.id);
            }

            // Find Pipeline requested by invocation 
            if child.is_type(&PTNodeType::PIPELINE) {
                let pl_children = child.children();

//              if *pl_children[0].token_value() == self.pl_name {
                let pl_name = &pl_children[0].token_value();
                let pl_list = &pl_children[1];
                let mut stages = vec![];

                for stage in pl_list.children() {
                    stage.expect_type(&PTNodeType::NAME);
                    let name = stage.token_value();
                    //check_name(name);

                    let enabled = stage.children().len() > 0;

                    stages.push(PipelineStage {
                        name: name.to_string(),
                        enabled: enabled,
                        state: RunState::NOTRUN,
                        pl_ptr: None,
                    });
                }
                flow.index.insert(pl_name.to_string(), flow.lines.len());
                flow.lines.push(Pipeline { name: pl_name.to_string(), stages });
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
