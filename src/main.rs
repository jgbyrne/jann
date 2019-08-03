use std::io;
use std::io::BufRead;
use std::path::PathBuf;
use std::env;

mod parse;
mod util;
mod invoke;
mod exec;
mod inter;
mod deploy;

fn main() {
    let stdin = io::stdin();
    let lines: Vec<String> = stdin.lock().lines().map(|l| l.unwrap()).collect();
    let mut toks = vec![];
    let mut log = util::Log::new("stdin", &lines);

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

    let tree = parse::parse(&mut log, &toks);
    if log.has_err() {
        log.conclude();
    }
    //tree.print_tree();
    
    let art = inter::Artifact::new(&toks, &tree);

    let cwd = env::current_dir().expect("Could not get cwd"); 
    let edir = cwd.join("pipeline-main");
    let inv = invoke::Invocation {
        root: cwd,
        edir,
        opts: deploy::DepOpt { OW_FF: true, OW_DD: true, OW_FD: false, OW_DF: true, INTER: true },
        pl_name: String::from("main"),
        art: art,
    };
    inv.invoke(&mut log);
    log.conclude();
}

