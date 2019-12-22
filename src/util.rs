extern crate regex;

use parse::Token;
use std::process;

pub struct Log<'src> {
    job  : String,
    lines: &'src Vec<String>,
    err_count: usize,
}

impl<'src> Log<'src> {
    pub fn new(job: String, lines: &'src Vec<String>) -> Log<'src> {
        Log {
            job: job,
            lines: lines,
            err_count: 0,
        }
    }
    
    fn message(&self, lvl: &str, msg: &str, hint: &str, tok: &Token) {
        println!("{}: {}", lvl, msg);
        if tok.lno != 1 {
            let preln = self.lines.get(tok.lno - 2).unwrap();
            if !preln.is_empty() {
                println!("{:>4} | {}", tok.lno - 1, preln);
            }
        }
        println!("{:>4} | {}", tok.lno, self.lines.get(tok.lno - 1).unwrap());
        println!("     |{}{}", &" ".repeat(1 + tok.val.lptr), &"^".repeat((tok.val.rptr - tok.val.lptr) + 1));
        println!("hint: {}\n", hint);
    }

    pub fn has_err(&self) -> bool {
        self.err_count > 0
    }

    pub fn conclude(&self) -> ! {
        if self.err_count == 0 {
            println!("\n[{}] success", self.job);
            process::exit(0);
        }
        else {
            println!("\n[{}] failed", self.job);
            process::exit(1);
        }
    }

    pub fn error(&mut self, msg: &str, hint: &str, tok: &Token) {
        self.message("error", msg, hint, tok);
        self.err_count += 1;
    }

    pub fn terminal(&mut self, msg: &str, hint: &str, tok: &Token) -> ! {
        self.message("error", msg, hint, tok);
        self.err_count += 1;
        self.conclude();
    }

    pub fn sys_terminal(&mut self, msg: &str) -> ! {
        println!("error: {}", msg);
        self.err_count += 1;
        self.conclude();
    }
}

pub fn check_name(name: &str) -> bool {
    let re = regex::Regex::new(r"^[a-zA-Z0-9_]*$").unwrap();
    re.is_match(name)
}
