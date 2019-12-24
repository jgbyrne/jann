use std::collections::HashMap;
use parse::{ParseTree, ParseTreeNode, PTNodeType, Token, TokenType};
use util;

#[derive(Clone, Debug)]
pub enum Value<'src> {
    List(Vec<Value<'src>>),
    Str(String),
    Name(&'src str),
    JName(&'src str),
}


#[derive(Debug)]
pub struct Symbols<'src> {
    pub names: HashMap<&'src str, Value<'src>>,
    pub jnames: HashMap<&'src str, Value<'src>>,
    pub blocks: HashMap<&'src str, usize>,
    pub includes: HashMap<String, (String, String, bool)>,
}

impl<'src> Symbols<'src> {
    pub fn new() -> Symbols<'src> {
        Symbols {
            names : HashMap::new(),
            jnames: HashMap::new(),
            blocks: HashMap::new(),
            includes: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct Artifact<'src> {
    pub toks:  &'src Vec<Token<'src>>,
    pub tree:  &'src ParseTree,
}

#[derive(Debug)]
pub struct LinkNode<'int, 'src: 'int> {
    pub int: &'int Artifact<'src>,
    pub tok: &'src Token<'src>,
    pub ptn: &'src ParseTreeNode,
}

impl<'int, 'src: 'int> LinkNode<'int, 'src> {
    pub fn children(&self) -> Vec<LinkNode<'int, 'src>> {
        let mut in_children = vec![];
        for cid in &self.ptn.children {
            let child = &self.int.tree.get_node(*cid);
            let tok   = &self.int.toks[child.tok_id - 1];
            in_children.push(LinkNode { int: &self.int, tok, ptn: child });
        }
        in_children
    }

    pub fn is_type(&self, nt: &PTNodeType) -> bool {
       self.ptn.nt == *nt 
    }

    pub fn expect_type(&self, nt: &PTNodeType) {
        if !self.is_type(nt) {
            panic!("Expected {:?} type!", nt);
        }
    }

    pub fn token_value(&self) -> &'src str {
        self.tok.val.slice()
    }
}

//opts: deploy::DepOpt { OW_FF: true, OW_DD: true, OW_FD: false, OW_DF: true, INTER: true }

impl<'int, 'src: 'int> Artifact<'src> {
    pub fn new(toks: &'src Vec<Token<'src>>, tree: &'src ParseTree) -> Artifact<'src> {
        Artifact { toks, tree }
    }

    pub fn root(&'int self) -> LinkNode<'int, 'src> {
        if self.tree.is_empty() {
            panic!("Parse Tree is empty");
        }
        let ptn = &self.tree.get_node(0);
        /* Token value should never be read, so just point to arbritrary token */
        LinkNode { int: &self, tok: &(self.toks[0]), ptn: ptn }
    }

    pub fn node(&'int self, n: usize) -> LinkNode<'int, 'src> {
        if n == 0 {
            return self.root();
        }
        let ptn = &self.tree.get_node(n);
        LinkNode { int: &self, tok: &(self.toks[ptn.tok_id - 1]), ptn: ptn }
    }
}

pub fn check_name(name: &str) -> bool {
    let re = regex::Regex::new(r"^[a-zA-Z0-9_]*$").unwrap();
    re.is_match(name)
}

// Substitute variable names from the symbol table
// Used for shell command statements and also other value strings

pub fn interpolate<'inv, 'src: 'inv>(log: &mut util::Log<'src>,
                                     symbols: &Symbols<'src>,
                                     base: &'inv str,
                                     node: &LinkNode<'inv, 'src>) -> String {
    // A mini enumeration of parsing states
    let NONE = 0;
    let LBRACE = 1;
    let RBRACE = 2;
    let WITHIN = 3;
    // Expecting Escape
    let mut esc = false;
    // Expected State
    let mut ex = NONE;

    // The final string is built into outstr
    let mut outstr: String = "".to_string();

    // Name stores interpolation variables as they are parsed
    let mut name: String = "".to_string();

    // We parse on a char-by-char basis
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
                if let Value::Str(ref v) = val  {
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

pub fn load_value<'old, 'src: 'old>(symbols: &Symbols<'src>,
                                    node : &LinkNode<'old, 'src> ) -> Value<'src> {
    match node.ptn.nt {
        PTNodeType::NAME => {
            let name = node.tok.val.slice();
            if let Some(val) = symbols.names.get(name) {
                (*val).clone()
            }
            else {
                Value::Str(name.to_string())
            }
        },
        PTNodeType::JNAME => {
            let jname = node.tok.val.slice();
            if let Some(val) = symbols.jnames.get(jname) {
                (*val).clone()
            }
            else {
                panic!("Undefined JNAME");
            }
        },
        PTNodeType::LIST => {
            let mut vals = vec![];
            for elem in node.children() {
                vals.push(load_value(symbols, &elem));
            }
            Value::List(vals)
        },
        _ => { panic!("Bad Value"); }
    }
}

