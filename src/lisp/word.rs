use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tag identifies what kind of value a Word holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Tag {
    Nil      = 0,
    Bool     = 1,
    Integer  = 2,
    Symbol   = 3,
    Cons     = 4,
    Closure  = 5,
    Agent    = 6,
    Trust    = 7,
    Builtin  = 8,
    Str      = 9,
}

/// The fundamental data type of the LISP machine.
/// Every value — numbers, lists, closures, agents — is a Word.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Word {
    Nil,
    Bool(bool),
    Int(i64),
    Symbol(u32),            // interned symbol id
    Cons(u32, u32),         // (car_ptr, cdr_ptr) into heap
    Closure {
        params: u32,        // heap ptr to param list
        body: u32,          // heap ptr to body expr
        env_id: u32,        // id of captured env frame
    },
    Agent(u32),             // agent record id
    Trust(u32),             // trust object id
    Builtin(u8),            // built-in opcode
    Str(u32),               // interned string id
}

impl Word {
    pub fn tag(&self) -> Tag {
        match self {
            Word::Nil => Tag::Nil,
            Word::Bool(_) => Tag::Bool,
            Word::Int(_) => Tag::Integer,
            Word::Symbol(_) => Tag::Symbol,
            Word::Cons(_, _) => Tag::Cons,
            Word::Closure { .. } => Tag::Closure,
            Word::Agent(_) => Tag::Agent,
            Word::Trust(_) => Tag::Trust,
            Word::Builtin(_) => Tag::Builtin,
            Word::Str(_) => Tag::Str,
        }
    }

    pub fn is_nil(&self) -> bool { matches!(self, Word::Nil) }
    pub fn is_truthy(&self) -> bool { !matches!(self, Word::Nil | Word::Bool(false)) }
    pub fn is_atom(&self) -> bool { !matches!(self, Word::Cons(_, _)) }
}

impl std::fmt::Display for Word {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Word::Nil => write!(f, "nil"),
            Word::Bool(b) => write!(f, "{}", if *b { "#t" } else { "#f" }),
            Word::Int(n) => write!(f, "{}", n),
            Word::Symbol(id) => write!(f, "sym#{}", id),
            Word::Cons(car, cdr) => write!(f, "({} . {})", car, cdr),
            Word::Closure { .. } => write!(f, "#<closure>"),
            Word::Agent(id) => write!(f, "#<agent:{}>", id),
            Word::Trust(id) => write!(f, "#<trust:{}>", id),
            Word::Builtin(op) => write!(f, "#<builtin:{}>", op),
            Word::Str(id) => write!(f, "str#{}", id),
        }
    }
}

/// Bidirectional symbol intern table.
/// Once a symbol string is interned, it gets a stable u32 id forever.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SymbolTable {
    by_name: HashMap<String, u32>,
    by_id: Vec<String>,
    str_by_name: HashMap<String, u32>,
    str_by_id: Vec<String>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.by_name.get(name) {
            return id;
        }
        let id = self.by_id.len() as u32;
        self.by_id.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    pub fn name(&self, id: u32) -> Option<&str> {
        self.by_id.get(id as usize).map(|s| s.as_str())
    }

    pub fn intern_str(&mut self, s: &str) -> u32 {
        if let Some(&id) = self.str_by_name.get(s) {
            return id;
        }
        let id = self.str_by_id.len() as u32;
        self.str_by_id.push(s.to_string());
        self.str_by_name.insert(s.to_string(), id);
        id
    }

    pub fn str_val(&self, id: u32) -> Option<&str> {
        self.str_by_id.get(id as usize).map(|s| s.as_str())
    }
}

/// Built-in opcodes
pub mod builtins {
    pub const CONS:   u8 = 0;
    pub const CAR:    u8 = 1;
    pub const CDR:    u8 = 2;
    pub const ADD:    u8 = 3;
    pub const SUB:    u8 = 4;
    pub const MUL:    u8 = 5;
    pub const DIV:    u8 = 6;
    pub const EQ:     u8 = 7;
    pub const NULL_P: u8 = 8;
    pub const ATOM_P: u8 = 9;
    pub const LIST:   u8 = 10;
    pub const NOT:    u8 = 11;
    pub const PRINT:  u8 = 12;
    pub const LT:     u8 = 13;
    pub const GT:     u8 = 14;
}
