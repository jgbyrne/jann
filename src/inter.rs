use std::collections::HashMap;
use parse::{ParseTree, ParseTreeNode, PTNodeType, Token, TokenType};

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
}

impl<'src> Symbols<'src> {
    pub fn new() -> Symbols<'src> {
        Symbols {
            names : HashMap::new(),
            jnames: HashMap::new(),
            blocks: HashMap::new(),
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
    true /* lol */
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

