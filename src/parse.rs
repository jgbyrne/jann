use std::str;
use std::fmt;
use util;

#[derive(Copy, Clone)]
pub struct Span<'src> {
    pub src: &'src str,
    pub lptr: usize,
    pub rptr: usize,
}

impl<'src> Span<'src> {
    fn single(src: &'src str, ptr: usize) -> Span {
        Span {src, lptr: ptr, rptr: ptr}
    }

    fn begin(src: &'src str, lptr: usize) -> Span {
        Span {src, lptr, rptr: 2_000_000_000}
    }

    fn conclude(&mut self, rptr: usize) {
        self.rptr = rptr;
    }

    fn conclude_prev(&mut self, rptr_plus: usize) {
        self.rptr = rptr_plus - 1;
    }

    fn shrink(&mut self, n: usize) {
        self.lptr += n;
        self.rptr -= n;
    }

    pub fn slice(&self) -> &'src str {
       str::from_utf8(&self.src.as_bytes()[self.lptr..(self.rptr + 1)]).unwrap()
    }
}

impl<'src> fmt::Debug for Span<'src> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.slice())
    }
}

#[derive(Copy, Clone, Debug)]
pub enum TokenType {
    STRING ,  // raw or "quoted"
    COMMAND,  // git clone
    
    LBRACE,   // {
    RBRACE,   // }
    LBRACK,   // [
    RBRACK,   // ]
    ARROW ,   // ->
    AT    ,   // @
    EQUALS,   // =
    DARROW,   // =>
    AARROW,   // >>
    COMMA ,   // ,
    PIPE  ,   // |
    COLON ,   // :
    HASH  ,   // #
    ERR   ,
}

#[derive(Copy, Clone)]
pub struct Token<'src> {
    id: usize,
    pub lno: usize,
    pub tt: TokenType,
    pub val: Span<'src>,
}

impl<'src> fmt::Debug for Token<'src> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({} | {:?} | {:?})", self.id, self.tt, self.val)
    }
}

enum Within {
    NONE    ,
    QSTRING ,
    BSTRING ,
    COMSTART,
    COMMAND ,
    ARROW   ,
    DARROW  ,
    AARROW  ,
}

fn breaking(c: char) -> bool {
     if c.is_alphanumeric() {
         return false;
     }
 
     if c.is_whitespace() {
         return true;
     }

     ['{','}','[',']','$','@','-','>','=',',','!','|','#'].iter().find(|b| **b == c).is_some()
}

pub fn tokenise<'src>(log: &mut util::Log, lno: usize, init_id: &mut usize, input: &'src str) -> Vec<Token<'src>> {

    if input.trim_start().starts_with("//") {
        return vec![];
    }

    let mut id = *init_id;
    let mut within: Within = Within::NONE;
    let mut esc: bool = false;
    let mut span: Span = Span::single(input, 0);
    let mut toks = vec![];

    let ci : Vec<(usize, char)> = input.char_indices().collect();
    let inlen = ci.len();
    let mut idx: usize = 0;

    'tok: while let Some((i, c)) = ci.get(idx) {
        let i = *i; let c = *c;
        match within {
            Within::NONE    => {
                let stt = match c {
                    '{' => Some(TokenType::LBRACE),
                    '}' => Some(TokenType::RBRACE),
                    '[' => Some(TokenType::LBRACK),
                    ']' => Some(TokenType::RBRACK),
                    '@' => Some(TokenType::AT    ),
                    ',' => Some(TokenType::COMMA ),
                    '|' => Some(TokenType::PIPE  ),
                    ':' => Some(TokenType::COLON ),
                    '#' => Some(TokenType::HASH  ),
                    _   => None, 
                };

                if let Some(stt) = stt {
                    toks.push(Token { id, lno, tt: stt, val: Span::single(input, i) } );
                    id += 1
                }
                else {  
                    span = Span::begin(input, i);
                    within = match c {
                        '-' => Within::ARROW,
                        '=' => Within::DARROW,
                        '>' => Within::AARROW,
                        '"' => Within::QSTRING,
                        '$' => Within::COMSTART,
                        c if !breaking(c) => Within::BSTRING,
                        _   => Within::NONE,
                    }
                }
            },

            Within::QSTRING => {
                if c == '"' && !esc {
                    span.conclude(i);
                    span.shrink(1);
                    toks.push(Token { id, lno, tt: TokenType::STRING, val: span });
                    id += 1; span = Span::single(input, 0);
                    within = Within::NONE;
                }
            },

            Within::BSTRING => {
                if breaking(c) && !esc {
                    span.conclude_prev(i);
                    toks.push(Token { id, lno, tt: TokenType::STRING, val: span } );
                    id += 1; span = Span::single(input, 0);
                    within = Within::NONE;
                    continue 'tok;
                }
                if c == '\\' && !esc {
                    esc = true;
                }
                else if esc {
                    esc = false;
                }

            },

            Within::COMSTART => {
                if !c.is_whitespace() {
                    span = Span::begin(input, i);
                    within = Within::COMMAND;
                    continue 'tok;
                }
            },

            Within::COMMAND => {
                // Within a command
            },

           arr @ Within::ARROW | arr @ Within::AARROW  => {
                if c == '>' {
                    span.conclude(i);
                    toks.push(Token { id, lno, tt: match arr { Within::ARROW => TokenType::ARROW, Within::AARROW => TokenType::AARROW, _ => unreachable!() }, val: span } );
                    id += 1; span = Span::single(input, 0); 
                    within = Within::NONE;
                }
                else {
                    span.conclude(i - 1);
                    toks.push(Token { id, lno, tt: TokenType::ERR, val: span } );
                    log.error("Headless Arrow", "Add a '>' character", &toks.last().unwrap());
                    *init_id = id + 1;
                    return toks;
                }
            },

            Within::DARROW => {
                if c == '>' {
                    span.conclude(i);
                    toks.push(Token { id, lno, tt: TokenType::DARROW, val: span } );
                    id += 1; span = Span::single(input, 0);
                    within = Within::NONE;
                }
                else {
                    span.conclude(i - 1);
                    toks.push(Token { id, lno, tt: TokenType::EQUALS, val: span } );
                    id += 1; span = Span::single(input, 0);
                    within = Within::NONE;
                    continue 'tok;
                }
            },
        }
        idx += 1;

        if idx >= inlen {
            match within {
                Within::NONE => {},
                Within::BSTRING => {
                    span.conclude(i);
                    toks.push(Token { id, lno, tt: TokenType::STRING, val: span } );
                    id += 1;
                },
                Within::COMMAND => {
                    span.conclude(i);
                    toks.push(Token { id, lno, tt: TokenType::COMMAND, val: span } );
                    id += 1;
                },
                _ => {
                    span.conclude(i);
                    toks.push(Token { id, lno, tt: TokenType::ERR, val: span } );
                    log.error("Unexpected EOF", "Close this construct", &toks.last().unwrap());
                    *init_id = id + 1;
                    return toks;
                }
            }
            break;
        }
    }
    *init_id = id;
    toks
}

#[derive(PartialEq, Debug)]
pub enum PTNodeType {
    ROOT   ,
    BLOCK  ,
    MAP    ,
    ASSIGN ,
    COMMAND,
    DIRECTIVE,
    JNAME  ,
    NAME   ,
    LIST   ,
    INSERT ,
    COPY   ,
    PIPELINE,
    FLAG    ,
}

#[derive(Debug)]
pub struct ParseTreeNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub nt: PTNodeType,
    pub tok_id: usize,
}

impl ParseTreeNode {
    fn rprint(&self, tree: &ParseTree, n: usize) {
        println!("{}{:?}: {:?} [{:?}]", "\t".repeat(n), self.id, self.nt, self.tok_id);
        for child in &self.children {
            tree.nodes[*child].rprint(&tree, n + 1);
        }
    }
}

#[derive(Debug)]
pub struct ParseTree {
    nodes: Vec<ParseTreeNode>,
}

impl ParseTree {
    fn new() -> ParseTree {
        let root = ParseTreeNode {
            id: 0,
            parent: None,
            children: vec![],
            nt: PTNodeType::ROOT,
            tok_id: 0
        };
        ParseTree { nodes: vec![root] }
    }

    fn add_node(&mut self, mut node: ParseTreeNode) -> usize {
        node.id = self.nodes.len();
        let nid = node.id;
        if let Some(parent) = node.parent {
            self.nodes[parent].children.push(nid);
        }
        self.nodes.push(node);
        nid
    }

    fn bind_child(&mut self, parent: usize, child: usize) {
        self.nodes[parent].children.push(child);
        self.nodes[child].parent = Some(parent);
    }

    pub fn print_tree(&self) {
        self.nodes[0].rprint(&self, 0);
    }

    pub fn get_node(&self, id: usize) -> &ParseTreeNode {
        self.nodes.get(id).unwrap()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.len() == 1
    }
}


struct Parser<'log, 'src: 'log> {
    toks   : &'src Vec<Token<'src>>,
    backptr: usize,
    foreptr: usize,
    tree   : ParseTree,
    log    : &'log mut util::Log<'src>,
}

impl<'log, 'src> Parser<'log, 'src> {
    fn new(log: &'log mut util::Log<'src>, toks: &'src Vec<Token>) -> Parser<'log, 'src> {
        Parser { toks, backptr: 0, foreptr: 0, tree: ParseTree::new(), log }
    }

    fn tok(&self) -> &Token<'src> {
        self.toks.get(self.backptr).unwrap()
    }

    fn tok_id(&self) -> usize {
        self.toks.get(self.backptr).unwrap().id
    }

    fn has_cur(&self) -> bool {
        self.backptr < self.toks.len()
    }

    fn has_next(&self) -> bool {
        (self.backptr + 1) < self.toks.len()
    }

    fn retreat(&mut self) -> usize {
        self.backptr -= 1;
        self.backptr
    }

    fn step(&mut self) -> usize {
        self.backptr += 1;
        self.backptr
    }

    fn step_or_err(&mut self, msg: &str, hint: &str) -> Option<usize> {
        if !self.has_next() {
            self.terminal(msg, hint);
            None
        }
        else {
            Some(self.step())
        }
    }

    fn peek(&mut self, n: usize) -> &Token<'src> {
        self.foreptr = self.backptr + n;
        self.toks.get(self.foreptr).unwrap()
    }

    fn orphan(&mut self, nt: PTNodeType, tok_id: usize) -> usize {
        let node = ParseTreeNode {
            id: 0, /* assigned by ParseTree */
            parent: None,
            children: vec![],
            nt: nt,
            tok_id: tok_id,
        };

        self.tree.add_node(node)
    }

    fn node(&mut self, parent: usize, nt: PTNodeType, tok_id: usize) -> usize {
        let node = ParseTreeNode {
            id: 0, /* assigned by ParseTree */
            parent: Some(parent),
            children: vec![],
            nt: nt,
            tok_id: tok_id,
        };

        self.tree.add_node(node)
    }

    fn error(&mut self, msg: &str, hint: &str) {
        let cur_tok = &self.tok().clone();
        self.log.error(msg, hint, cur_tok);
    }

    fn terminal(&mut self, msg: &str, hint: &str) {
        let cur_tok = &self.tok().clone();
        self.log.terminal(msg, hint, cur_tok);
    }

}

fn parse_val(parser: &mut Parser) -> Option<usize> {
    let cur_tt = parser.tok().tt;
    let tok_id = parser.tok_id();
    match cur_tt {
        TokenType::STRING => {
            parser.step();
            Some(parser.orphan(PTNodeType::NAME, tok_id))
        },
        TokenType::AT   => {
            parser.step_or_err("Bare '@'", "Cannot conclude here")?;
            let name_tt = parser.tok().tt;
            let name_id = parser.tok_id();
            match name_tt {
                TokenType::STRING => {
                    parser.step();
                    Some(parser.orphan(PTNodeType::JNAME, name_id))
                },
                _ => {
                    parser.error("Name must follow '@'", "Change this value to a name");
                    None
                },
            }
        },
        TokenType::LBRACK  => {
            let list = parser.orphan(PTNodeType::LIST, tok_id);
            parser.step_or_err("Bare Left Bracket", "Cannot conclude here")?;
            loop {
                match parser.tok().tt {
                    TokenType::RBRACK => { 
                        parser.step();
                        break Some(list);
                    },
                    _ => {
                        let elem = parse_val(parser)?;
                        parser.tree.bind_child(list, elem);
                    },
                }

                match parser.tok().tt {
                    TokenType::COMMA => {
                        parser.step_or_err("Bare Comma", "Cannot conclude here")?;
                    },
                    TokenType::RBRACK => {
                        parser.step();
                        break Some(list);
                    }
                    _ => { 
                        parser.error("Malformed List", "Add a comma or bracket before here");
                        break None;
                    },
                }
            }
        },
        _ => { parser.error("Expected value", "Add a value before here"); None },
    }
}

fn recover_block(parser: &mut Parser) {
    loop {
        if parser.step_or_err("Unclosed Brace", "Add a brace after here").is_none() {
            return;
        }

        match parser.tok().tt {
            TokenType::RBRACE => {
                parser.step();
                return;
            },
            _ => {},
        }
    }
}

fn parse_block(parser: &mut Parser, tag: usize) -> Option<usize> {
    let block_id = parser.tok_id();
    let block = parser.orphan(PTNodeType::BLOCK, block_id);
    parser.tree.bind_child(block, tag);
    parser.step_or_err("Unclosed Brace", "Add a brace after here")?;
    loop {
        if let Some(sub_stmt) = parse_stmt(parser) {
            if sub_stmt == 0 {
                break;
            }
            else {
                parser.tree.bind_child(block, sub_stmt);
            }
        }
        else {
            recover_block(parser);
            break;
        }
    }
    Some(block)
}

fn parse_val_stmt(parser: &mut Parser) -> Option<usize> {
    let val = parse_val(parser)?;
    
    if !parser.has_cur() {
        parser.retreat();
        parser.error("Bare Value", "Cannot conclude here");
        return None;
    }

    let cur_tt = parser.tok().tt;
    let tok_id  = parser.tok_id();

    match cur_tt {
        TokenType::EQUALS => {
            let stmt = parser.orphan(PTNodeType::ASSIGN, tok_id);
            parser.tree.bind_child(stmt, val);
            parser.step_or_err("Bare Equals", "Cannot conclude here")?;
            let rval = parse_val(parser)?;
            parser.tree.bind_child(stmt, rval);
            Some(stmt)
        },
        TokenType::AARROW => {
            let stmt = parser.orphan(PTNodeType::COPY, tok_id);
            parser.tree.bind_child(stmt, val);
            parser.step_or_err("Bare Copy Arrow", "Cannot conclude here")?;
            let rval = parse_val(parser)?;
            parser.tree.bind_child(stmt, rval);
            Some(stmt)
        },
        TokenType::DARROW => {
            let stmt = parser.orphan(PTNodeType::INSERT, tok_id);
            parser.tree.bind_child(stmt, val);
            parser.step_or_err("Bare Insertion Arrow", "Cannot conclude here")?;
            let rval = parse_val(parser)?;
            parser.tree.bind_child(stmt, rval);
            Some(stmt)
        },
        TokenType::PIPE | TokenType::COLON => {
            let mut enabled = match cur_tt { TokenType::PIPE => true,
                                             TokenType::COLON => false,
                                             _ => unreachable!() };
            let mut bar_tok_id = tok_id;
            let stmt = parser.orphan(PTNodeType::PIPELINE, tok_id);
            parser.tree.bind_child(stmt, val);
            parser.step_or_err("Bare pipeline symbol", "Cannot conclude here")?;
            let stages = parser.orphan(PTNodeType::LIST, tok_id);
            loop {
                let stage = parse_val(parser)?;
                parser.tree.bind_child(stages, stage);
                if enabled {
                    let stage_enabled = parser.orphan(PTNodeType::FLAG, bar_tok_id);
                    parser.tree.bind_child(stage, stage_enabled);
                }
                if !parser.has_cur() { break; }
                match parser.tok().tt {
                    TokenType::LBRACK => {
                        let tags = parse_val(parser)?;
                        parser.tree.bind_child(stage, tags);
                    },
                    _ => (),
                }

                bar_tok_id = parser.tok_id();
                match parser.tok().tt {
                    TokenType::PIPE => {
                        enabled = true;
                        parser.step_or_err("Bare enabled pipe", "Cannot conclude here")?;
                    },
                    TokenType::COLON => {
                        enabled = false;
                        parser.step_or_err("Bare disabled pipe", "Cannot conclude here")?;
                    },
                    _ => { break; }
                }
            }
            parser.tree.bind_child(stmt, stages);
            Some(stmt)
        },
        TokenType::ARROW  => {
            let map = parser.orphan(PTNodeType::MAP, tok_id);
            parser.tree.bind_child(map, val);
            parser.step_or_err("Bare arrow", "Cannot conclude here")?;
            let rval = parse_val(parser)?;
            parser.tree.bind_child(map, rval);
            
            if !parser.has_cur() {
                parser.retreat();
                parser.error("Expected block", "Add a block after here");
                return None;
            }

            match parser.tok().tt {
                TokenType::LBRACE => {
                    Some(parse_block(parser, map)?)
                },
                _ => {
                    parser.error("Expected block", "Add a brace before here");
                    None
                },
            }
        },
        TokenType::LBRACE => {
            Some(parse_block(parser, val)?)
        },
        _ => { parser.error("Malformed statement", "This token is invalid in this position"); None },
    }
}

fn parse_stmt(parser: &mut Parser) -> Option<usize> {
    if !parser.has_cur() {
        parser.retreat();
        return None;
    }
    
    let cur_tt = parser.tok().tt;
    let tok_id = parser.tok_id();
    match cur_tt {
        TokenType::COMMAND => {
            let stmt = parser.orphan(PTNodeType::COMMAND, tok_id);
            parser.step();
            Some(stmt)
        },
        TokenType::HASH => {
            let stmt = parser.orphan(PTNodeType::DIRECTIVE, tok_id);
            parser.step();
            let verb = parse_val(parser)?;
            parser.tree.bind_child(stmt, verb);
            let data = parse_val(parser)?;
            parser.tree.bind_child(stmt, data);
            Some(stmt)
        }
        TokenType::RBRACE  => { parser.step(); Some(0) },
        _                  => { parse_val_stmt(parser) },
    }
}

fn parse_file(parser: &mut Parser) {
    if parser.has_cur() {
        loop {
            if let Some(stmt) = parse_stmt(parser) {
                parser.tree.bind_child(0, stmt);
                if !parser.has_next() {
                    break;
                }
            }
            else {
                break;
            }
        }
    }
}
pub fn parse<'log, 'src: 'log>(log: &'log mut util::Log<'src>, toks: &'src Vec<Token<'src>>) -> ParseTree {
    let mut parser = Parser::new(log, toks);
    parse_file(&mut parser);
    parser.tree
}
