use parse::{ParseTree, ParseTreeNode, PTNodeType, Token, TokenType};
use inter;
use exec;
use deploy;

use std::fs;
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
enum RunState {
    NOTRUN,
    DONE  ,
}

#[derive(Debug)]
struct PipelineStage {
    name: String,
    state: RunState,
}

#[derive(Debug)]
struct Pipeline {
    name  : String,
    stages: Vec<PipelineStage>,
}

impl Pipeline {
    fn load(path: PathBuf) -> Pipeline {
        let pl_data = fs::read_to_string(path).expect("Unable to load pipeline");
        let mut lines = pl_data.lines();

        let name = {
            if let Some(ln_one) = lines.next() {
                ln_one.to_string()
            }
            else {
                panic!("Blank pipeline file");
            }
        };

        let mut stages = vec![];

        for line in lines {
            let parts: Vec<&str> = line.split(' ').collect();
            
            let stage = {
                if let Some(part_one) = parts.get(0) {
                    part_one.to_string()
                }
                else {
                    panic!("Missing stage in pipeline file");
                }
            };

            let state = {
                if let Some(part_two) = parts.get(1) {
                    match *part_two {
                        "NOTRUN" => RunState::NOTRUN,
                        "DONE"   => RunState::DONE  ,
                        _        => { panic!("Bad state in pipeline file"); }
                    }
                }
                else {
                    panic!("Missing state in pipeline file");
                }
            };

            stages.push(PipelineStage { name: stage, state: state } );
        }

        Pipeline { name, stages }
    }

    fn dump(&self, path: PathBuf) {
        let mut out = self.name.clone();
        
        for PipelineStage { ref name, ref state } in &self.stages {
            out.push_str("\n");
            out.push_str(name);
            out.push_str(" ");
            out.push_str( {
                match *state {
                    RunState::NOTRUN => "NOTRUN",
                    RunState::DONE   => "DONE"  ,
                }
            });
        }

        fs::write(path, out).expect("Unable to write pipeline file");
    }
}

pub struct Invocation<'src> {
    pub root : PathBuf,
    pub edir : PathBuf,
    pub opts : deploy::DepOpt,
    pub pl_name : String, 
    pub art  : inter::Artifact<'src>,
}


impl<'src> Invocation<'src> {
    pub fn invoke(self) {
        let cwd = env::current_dir().expect("Could not get cwd.");

        if !self.edir.exists() {
            fs::create_dir_all(&self.edir).expect("Unable to create execution dir");
        }

        if env::set_current_dir(&self.edir).is_err() {
            panic!("Could not change CWD!");
        }

        let mut symbols = inter::Symbols::new();

        let pl_path = self.edir.join(&self.pl_name);
        let mut pipe = {
            if pl_path.exists() {
                Pipeline::load(pl_path)
            }
            else {
                // Recursively produce list of execution stages
                let root = self.art.root();
                let mut stages = vec![];

                for child in root.children() {
                    // Find Pipeline requested by invocation 
                    if child.is_type(&PTNodeType::PIPELINE) {
                        let pl_children = child.children();
                        if *pl_children[0].token_value() == self.pl_name {
                            let pl_list = &pl_children[1];
                            for stage in pl_list.children() {
                                stage.expect_type(&PTNodeType::NAME);
                                let name = stage.tok.val.slice();
                                //check_name(name);
                                stages.push(PipelineStage {
                                    name: name.to_string(),
                                    state: RunState::NOTRUN,
                                });
                            }
                            break
                        }
                    }

                    // Store Blocks in Symbol Table while we're at it
                    else if child.is_type(&PTNodeType::BLOCK) {
                       let tag = &child.children()[0];
                       if tag.is_type(&PTNodeType::NAME) {
                           symbols.blocks.insert(tag.token_value(), child.ptn.id);
                       }
                    }
                }
                Pipeline { name: self.pl_name.clone(), stages }
            }
        };
        
        for PipelineStage { name: ref st_name, state: ref mut st_state } in &mut pipe.stages {
            println!("{} {:?}", st_name, st_state);
            match *st_state {
                RunState::NOTRUN => {
                    let block_id = *symbols.blocks.get(st_name.as_str()).unwrap();
                    let mut node: inter::LinkNode = self.art.node(block_id);
                    exec::execute_block(&self, &mut symbols, &node);
                    /*
                    if let Some(es) = end_stage {
                        if *es == *st_name {
                            return;
                        }
                    }
                    */
                
                },
                RunState::DONE   => {
                    println!("Already done stage '{}', skipping...", st_name);
                    /*
                    if let Some(es) = end_stage {
                        if *es == *st_name {
                            println!("Nothing to be done");
                            return;
                        }
                    }
                    */
                },
            }
            *st_state = RunState::DONE;
        }
        
        pipe.dump(self.edir.join(self.pl_name));
        
        if env::set_current_dir(&cwd).is_err() {
            panic!("Could not change CWD!");
        }
    }
}
