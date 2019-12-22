extern crate walkdir;
extern crate dirs;

use parse::PTNodeType;
use std::process::Command;
use std::path::{Path, PathBuf, Component};
use std::env;
use std::fs;

use deploy;
use invoke;
use inter;
use util;
use parse;

fn interpolate<'inv, 'src: 'inv>(log: &mut util::Log<'src>,
                                 symbols: &mut inter::Symbols<'src>,
                                 node: &inter::LinkNode<'inv, 'src>) -> String {
    let base = node.token_value();
    let NONE = 0;
    let LBRACE = 1;
    let RBRACE = 2;
    let WITHIN = 3;
    let mut esc = false;
    let mut ex = NONE;

    let mut outstr: String = "".to_string();
    let mut name: String = "".to_string();

    for c in base.chars() {
        if ex == RBRACE {
            if c != '}' {
                log.terminal("Expected right brace", "Missing right brace", &node.tok);
            }
            ex = NONE;
            continue;
        }

        if ex == WITHIN {
            if c == '}' {
                let val = symbols.names.get(name.trim()).unwrap_or_else( || {
                    symbols.jnames.get(name.trim()).unwrap_or_else( || {
                        log.terminal(&format!("No such variable {}", name),
                                     "Ensure interpolation uses extant, in-scope variables", &node.tok);
                    })
                });
                if let inter::Value::Str(ref v) = val  {
                    outstr.push_str(v);
                }
                else {
                    log.terminal("Only strings can be interpolated into commands",
                                 &format!("Change the type of variable {}", name.trim()), &node.tok);
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
                outstr.push_str("{");
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

        outstr.push(c);
    }

    if ex != NONE {
        log.terminal("Bad interpolation syntax", "Make sure all braces are matched", &node.tok);
    }

    outstr
}


fn component_string(c: &Component) -> String {
    c.as_os_str().to_string_lossy().to_string()
}

fn command<'inv, 'src: 'inv>(inv: &invoke::Invocation<'src>,
                             symbols: &mut inter::Symbols<'src>,
                             log: &mut util::Log<'src>,
                             node: &inter::LinkNode<'inv, 'src>){
    let shell = {
        if let Some(inter::Value::Str(s)) = symbols.jnames.get("shell") {
            s.to_owned()
        }
        else {
            "/bin/sh".to_owned()
        }
    };

    let outcom = interpolate(log, symbols, node);
    println!(">>> {}", outcom);
    
    let mut proc = Command::new(&shell)
        .arg("-c")
        .arg(outcom)
        .spawn()
        .expect("failed to execute process");

    if !proc.wait().expect("failed to wait on process").success() { println!("Command ended with non-zero status") }
}

fn execute_stmts<'inv, 'src: 'inv>(inv: &invoke::Invocation<'src>,
                                   symbols: &mut inter::Symbols<'src>,
                                   log: &mut util::Log<'src>,
                                   stmts: Vec<&inter::LinkNode<'inv, 'src>>) {
    let mut scope_names : Vec<&'src str> = vec![];
    for node in stmts {
        match node.ptn.nt {
            PTNodeType::ASSIGN => {
                let rval = inter::load_value(symbols, &node.children()[1]);
                let lval = &node.children()[0];
                if util::check_name(lval.token_value()) {
                    if lval.is_type(&PTNodeType::NAME) {
                        scope_names.push(lval.token_value());
                        symbols.names.insert(lval.token_value(), rval);
                    }
                    else if lval.is_type(&PTNodeType::JNAME) {
                        symbols.jnames.insert(lval.token_value(), rval);
                    }
                }
                else {
                    log.terminal("Invalid variable name", "Make this a valid name", &lval.tok);
                }
            },
            PTNodeType::COMMAND => {
                command(inv, symbols, log, node);
            },
            PTNodeType::COPY | PTNodeType::INSERT => {
                let deploy_children = &node.children();
                let src_buf = PathBuf::from(interpolate(log, symbols, &deploy_children[0]));

                let comps: Vec<Component> = src_buf.components().collect();

                if comps.len() == 0 {
                    log.terminal("Source path is empty (this should not be allowed by the parser)",
                                 "Put a path here and then please file a bug report!",
                                 &deploy_children[0].tok);
                }

                if !comps.iter().all(|&c| match c { Component::Normal(_) => true, _ => false }) {
                    log.terminal("Invalid source path",
                                 "Remove any expansions and ensure path is relative to Jannfile",
                                 &deploy_children[0].tok);
                }
                
                let full_src = inv.root.join(&src_buf);

                if !full_src.exists() {
                    log.terminal(&format!("No entity at source path: {:?}", full_src),
                                 "Make this a valid path", &deploy_children[0].tok);
                }
                
                let mut dst_buf = PathBuf::from(interpolate(log, symbols, &deploy_children[1]));

                let dst_cpy = dst_buf.clone();
                let dst_comps: Vec<Component> = dst_cpy.components().collect();
             
                if dst_comps.len() == 0 {
                    log.terminal("Destination path is empty (this should not be allowed by the parser)",
                                 "Put a path here and then please file a bug report!",
                                 &deploy_children[1].tok);

                }

                dst_buf = if let Ok(dst_tail) = dst_buf.strip_prefix("~") {
                    dirs::home_dir().unwrap_or_else( || {
                        log.sys_terminal("Could not find home directory");
                    }).join(dst_tail)
                }
                else {
                    dst_buf
                };

                if !dst_buf.components().all(|c| match c {
                    Component::CurDir | Component::ParentDir => false,
                    _ => true,
                }) {
                    log.terminal(&format!("Invalid destination path {:?}", dst_buf),
                                 "Ensure path is absolute",
                                 &deploy_children[1].tok);
                }

                if node.is_type(&PTNodeType::INSERT) {
                    let entity = if let Some(parent) = src_buf.parent() {
                        src_buf.strip_prefix(parent).unwrap()
                    }
                    else {
                        &src_buf
                    };
                    dst_buf = PathBuf::from("/").join(dst_buf.join(entity));
                }

                if let Err(result) = {
                    if full_src.is_file() {
                        deploy::deploy(full_src, deploy::Entity::FILE, dst_buf, inv.opts)
                    }
                    else {
                        deploy::deploy(full_src, deploy::Entity::DIR, dst_buf, inv.opts)
                    }
                } {
                    log.terminal(&format!("Deployment error: [{}] {}", &result.source, &result.message),
                                          "Modify this line appropriately", &node.tok);
                }

            },
            PTNodeType::BLOCK   => { execute_block(inv, symbols, log, node); },
            _ => { continue; },
        }
    }
    for name in scope_names.iter() {
        symbols.names.remove(name);
    }
}

pub fn execute_block<'inv, 'src: 'inv>(inv: &invoke::Invocation<'src>,
                                       symbols: &mut inter::Symbols<'src>,
                                       log: &mut util::Log<'src>,
                                       node: &inter::LinkNode<'inv, 'src>) {
    let mut block_children = node.children();
    let tag = &block_children[0];

    match tag.ptn.nt {
        PTNodeType::NAME => {
            if util::check_name(tag.token_value()) {
                execute_stmts(inv, symbols, log, block_children.iter().skip(1).collect());
            }
            else {
                log.terminal("Invalid Block Name", "Choose a valid name for this block", &tag.tok);
            }
        },
        PTNodeType::MAP  => {
            let map = &block_children[0];
            let map_children = map.children();
            if let inter::Value::List(vlist) = inter::load_value(symbols, &map_children[0]) {
                let name = map_children[1].token_value();
                if util::check_name(name) {
                    for elem in vlist {
                        symbols.names.insert(name, elem);
                        execute_stmts(inv, symbols, log, block_children.iter().skip(1).collect());
                    }
                    symbols.names.remove(name);
                }
                else {
                    log.terminal("Invalid Map Variable Name", 
                                 "Choose a valid name for this variable", &map_children[1].tok);
                }
            }
            else {
                log.terminal("Left side of Map must be a list",
                             "Replace this value with a list", &map_children[0].tok);
            }
            let name  = &node.children()[1];
            
        },
        _ => { log.terminal("Invalid Block Tag", "Replace this with a name or a mapping", &tag.tok); },
    }
}
