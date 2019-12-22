use std::io;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::env;
use std::process;
use std::fs::File;

mod com;
mod parse;
mod util;
mod invoke;
mod exec;
mod inter;
mod deploy;

fn main() {
    /* Parse command line arguments */
    
    let command = com::Command::new();

    let (lines, switches, job) = match command {
        com::Command::HELP { code } => {
            println!("jann - Configuration deployment utility for *nix");
            process::exit(code);
        },
        com::Command::VERSION { code } => {
            println!("jann v0.1.0");
            process::exit(code);
        },
        com::Command::DO_STDIN { switches } => {
            let stdin = io::stdin();
            let lines: Vec<String> = stdin.lock().lines().map(|l| l.unwrap()).collect();
            (lines, switches, String::from("stdin"))
        },
        com::Command::DO_FILE { switches, file: path } => {
            let file = File::open(&path);
            let file = file.unwrap_or_else( |_| {
                println!("error: no such file {}", path);
                process::exit(66);
            });
            let reader = BufReader::new(file);
            let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();
            (lines, switches, path)
        },
    };

    // println!("Switches: {:?}", switches);
    

    let mut log = util::Log::new(job, &lines);

    /* Tokenise input data */

    let mut toks = vec![];

    let mut id: usize = 1;
    let mut lno: usize = 1;
    for index in 0..(lines.len()) {
        toks.extend(parse::tokenise(&mut log, lno, &mut id, &lines[index]));
        lno += 1;
    }
    if log.has_err() {
        log.conclude();
    }
    
    //println!("{:#?}", &toks);
    
    /* Create parse tree for input data */

    let tree = parse::parse(&mut log, &toks);
    if log.has_err() {
        log.conclude();
    }
    
    //tree.print_tree();
    
    /* Get entry-point */
    
    let mut pl_name = String::from("main");
    for (com, refs) in &switches {
        if com == "execute" {
            if let Some(com::Reference::PIPELINE(pl)) = refs.get(0) {
                pl_name = pl.to_string();
            }
        }
    }

    /* Execute parsed Jannfile */

    let art = inter::Artifact::new(&toks, &tree);
    let cwd = env::current_dir().expect("Could not get cwd"); 
    // use ./deploy as execution directory for now
    let edir = cwd.join("deploy"); 
    let inv = invoke::Invocation {
        root: cwd,
        edir,
        opts: deploy::DepOpt { OW_FF: true, OW_DD: true, OW_FD: false, OW_DF: true, INTER: true },
        pl_name,
        art: art,
        switches: switches,
    };
    inv.invoke(&mut log);

    log.conclude();
}

