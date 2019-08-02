extern crate walkdir;

use parse::PTNodeType;
use std::process::Command;
use std::path::{Path, PathBuf};

use deploy;
use invoke;
use inter;


fn path_buf(s: &str) -> PathBuf {
    Path::new(s).to_path_buf()
}

fn command<'old, 'src: 'old>(inv: &invoke::Invocation<'src>,
                             symbols: &mut inter::Symbols<'src>,
                             node: &inter::LinkNode<'old, 'src>){
    let shell = {
        if let Some(inter::Value::Str(s)) = symbols.jnames.get("shell") {
            s.to_owned()
        }
        else {
            "/bin/sh".to_owned()
        }
    };

    let com = node.token_value();

    let NONE = 0;
    let LBRACE = 1;
    let RBRACE = 2;
    let WITHIN = 3;
    let mut esc = false;
    let mut ex = NONE;

    let mut outcom: String = "".to_string();
    let mut name: String = "".to_string();

    for c in com.chars() {
        if ex == RBRACE {
            if c != '}' {
                panic!("Malformed command");
            }
            ex = NONE;
            continue;
        }

        if ex == WITHIN {
            if c == '}' {
                let val = symbols.names.get(name.trim()).expect("No such variable");
                if let inter::Value::Str(ref v) = val  {
                    outcom.push_str(v);
                }
                else {
                    panic!("Only strings can be interpolated into commansd");
                }
                name = "".to_string();
                ex = RBRACE;
                continue;
            }
            name.push(c);
            continue;
        }

        if ex == LBRACE {
            if c == '{' {
                ex = WITHIN;
                continue;
            }
            else {
                ex = NONE;
                outcom.push_str("{");
            }
        
        }

        if c == '\\' && !esc {
            esc = true;
            continue;
        }
        if c == '{' && !esc {
           ex = LBRACE;
           continue;
        }

        if esc { esc = false; }

        outcom.push(c);
    }

    println!("\n>>> {}", outcom);

    
    let mut proc = Command::new(&shell)
        .arg("-c")
        .arg(outcom)
        .spawn()
        .expect("failed to execute process");

    if !proc.wait().expect("failed to wait on process").success() { println!("Command ended with non-zero status") }
}

fn execute_stmts<'old, 'src: 'old>(inv: &invoke::Invocation<'src>,
                                   symbols: &mut inter::Symbols<'src>,
                                   stmts: Vec<&inter::LinkNode<'old, 'src>>) {
    for node in stmts {
        match node.ptn.nt {
            PTNodeType::ASSIGN => {
                let rval = inter::load_value(symbols, &node.children()[1]);
                let lval = &node.children()[0];
                if inter::check_name(lval.token_value()) {
                    if lval.is_type(&PTNodeType::NAME) {
                        symbols.names.insert(lval.token_value(), rval);
                    }
                    else if lval.is_type(&PTNodeType::JNAME) {
                        symbols.jnames.insert(lval.token_value(), rval);
                    }
                }
                else {
                    panic!("Bad LVAL"); 
                }
            },
            PTNodeType::COMMAND => {
                command(inv, symbols, node);
            },
            PTNodeType::COPY => {
                let cpy_children = &node.children();
                let src_buf = path_buf(cpy_children[0].token_value());
                let dst_buf = path_buf(cpy_children[1].token_value());
                if src_buf.is_file() {
                    deploy::deploy(src_buf, deploy::Entity::FILE, dst_buf, inv.opts);
                }
                else {
                    deploy::deploy(src_buf, deploy::Entity::DIR, dst_buf, inv.opts);
                }
            },
            PTNodeType::INSERT => {
                let ins_children = &node.children();
                //insert(dir, path_buf(ins_children[0].tok.val.slice()), path_buf(ins_children[1].tok.val.slice()));
            },
            PTNodeType::BLOCK   => { execute_block(inv, symbols, node); },
            _ => { continue; },
        }
    }
}

pub fn execute_block<'old, 'src: 'old>(inv: &invoke::Invocation<'src>,
                                       symbols: &mut inter::Symbols<'src>,
                                       node: &inter::LinkNode<'old, 'src>) {
    let mut block_children = node.children();
    let tag = &block_children[0];

    match tag.ptn.nt {
        PTNodeType::NAME => {
            if inter::check_name(tag.token_value()) {
                execute_stmts(inv, symbols, block_children.iter().skip(1).collect());
            }
            else {
                panic!("Bad blocktag");
            }
        },
        PTNodeType::MAP  => {
            let map = &block_children[0];
            let map_children = map.children();
            if let inter::Value::List(vlist) = inter::load_value(symbols, &map_children[0]) {
                let name = map_children[1].token_value();
                if inter::check_name(name) {
                    for elem in vlist {
                        symbols.names.insert(name, elem);
                        execute_stmts(inv, symbols, block_children.iter().skip(1).collect());
                    }
                    symbols.names.remove(name);
                }
                else {
                    panic!("Bad Name");
                }
            }
            else {
                panic!("Left side must be list");
            }
            let name  = &node.children()[1];
            
        },
        _ => { panic!{"Invalid block tag"} },
    }
}
