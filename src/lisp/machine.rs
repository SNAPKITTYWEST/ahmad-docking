use crate::lisp::word::{Word, SymbolTable, builtins};
use crate::lisp::heap::Heap;
use crate::lisp::env::EnvStore;
use crate::lisp::eval::{Evaluator, EvalError};
use crate::lisp::world::{WorldDump, WorldVault};
use crate::lisp::parser::parse_and_alloc;
use crate::chain::ChainError;

/// The LISP Machine.
/// Owns the heap, symbol table, environment store, and world vault.
/// This is the soul container — everything lives here, serializes here, restores here.
pub struct LispMachine {
    pub heap: Heap,
    pub symbols: SymbolTable,
    pub env: EnvStore,
    pub vault: WorldVault,
    pub tick: u64,
    pub agent_id: String,
}

impl LispMachine {
    pub fn new() -> Self {
        let mut machine = Self {
            heap: Heap::new(),
            symbols: SymbolTable::new(),
            env: EnvStore::new(),
            vault: WorldVault::new(),
            tick: 0,
            agent_id: "METATRON".to_string(),
        };
        machine.install_builtins();
        machine
    }

    pub fn with_agent(agent_id: &str) -> Self {
        let mut m = Self::new();
        m.agent_id = agent_id.to_string();
        m
    }

    /// Restore a machine from a world dump.
    pub fn restore(dump: WorldDump) -> Self {
        let tick = dump.tick + 1;
        let agent_id = dump.agent_id.clone();
        Self {
            heap: dump.heap,
            symbols: dump.symbols,
            env: dump.env,
            vault: WorldVault::new(), // new vault — the chain will be separate
            tick,
            agent_id,
        }
    }

    /// Parse and evaluate a source string.
    pub fn eval_str(&mut self, input: &str) -> Result<Word, EvalError> {
        let expr = parse_and_alloc(input, &mut self.symbols, &mut self.heap)?;
        self.eval_expr(expr)
    }

    /// Evaluate an already-parsed Word expression.
    pub fn eval_expr(&mut self, expr: Word) -> Result<Word, EvalError> {
        let global = self.env.global_id();
        let mut evaluator = Evaluator::new(&mut self.heap, &mut self.symbols, &mut self.env);
        evaluator.eval(expr, global)
    }

    /// Checkpoint: snapshot world state and seal to chain.
    pub fn checkpoint(&mut self, reason: &str) -> Result<String, ChainError> {
        let prev_seal = self.vault.latest().map(|d| d.seal.clone());
        let dump = WorldDump::new(
            self.tick,
            self.heap.clone(),
            self.symbols.clone(),
            self.env.clone(),
            prev_seal,
            format!("{} ({})", self.agent_id, reason),
        );
        let seal = self.vault.seal_dump(dump)?;
        self.tick += 1;
        Ok(seal)
    }

    /// Run GC — mark all env roots, sweep unreachable cells.
    pub fn gc(&mut self) -> usize {
        self.env.mark_roots(&mut self.heap);
        self.heap.sweep()
    }

    /// Display a Word as a human-readable string.
    pub fn display(&self, word: &Word) -> String {
        self.display_depth(word, 0)
    }

    fn display_depth(&self, word: &Word, depth: usize) -> String {
        if depth > 32 { return "...".to_string(); }
        match word {
            Word::Nil => "nil".to_string(),
            Word::Bool(true) => "#t".to_string(),
            Word::Bool(false) => "#f".to_string(),
            Word::Int(n) => n.to_string(),
            Word::Symbol(id) => self.symbols.name(*id).unwrap_or("?").to_string(),
            Word::Str(id) => format!("\"{}\"", self.symbols.str_val(*id).unwrap_or("")),
            Word::Builtin(op) => format!("#<builtin:{}>", op),
            Word::Agent(id) => format!("#<agent:{}>", id),
            Word::Trust(id) => format!("#<trust:{}>", id),
            Word::Closure { .. } => "#<closure>".to_string(),
            Word::Cons(car, cdr) => {
                let car_str = match self.heap.get(*car) {
                    Some(w) => self.display_depth(w, depth + 1),
                    None => "?".to_string(),
                };
                let cdr_word = self.heap.get(*cdr).cloned().unwrap_or(Word::Nil);
                match &cdr_word {
                    Word::Nil => format!("({})", car_str),
                    Word::Cons(_, _) => {
                        let rest = self.display_list_tail(&cdr_word, depth + 1);
                        format!("({} {})", car_str, rest)
                    }
                    other => format!("({} . {})", car_str, self.display_depth(other, depth + 1)),
                }
            }
        }
    }

    fn display_list_tail(&self, word: &Word, depth: usize) -> String {
        if depth > 32 { return "...".to_string(); }
        match word {
            Word::Nil => String::new(),
            Word::Cons(car, cdr) => {
                let car_str = match self.heap.get(*car) {
                    Some(w) => self.display_depth(w, depth + 1),
                    None => "?".to_string(),
                };
                let cdr_word = self.heap.get(*cdr).cloned().unwrap_or(Word::Nil);
                match &cdr_word {
                    Word::Nil => car_str,
                    Word::Cons(_, _) => {
                        format!("{} {}", car_str, self.display_list_tail(&cdr_word, depth + 1))
                    }
                    other => format!("{} . {}", car_str, self.display_depth(other, depth + 1)),
                }
            }
            other => self.display_depth(other, depth),
        }
    }

    /// Install built-ins into the global env under their standard names.
    fn install_builtins(&mut self) {
        let pairs = [
            ("cons",  builtins::CONS),
            ("car",   builtins::CAR),
            ("cdr",   builtins::CDR),
            ("+",     builtins::ADD),
            ("-",     builtins::SUB),
            ("*",     builtins::MUL),
            ("/",     builtins::DIV),
            ("eq?",   builtins::EQ),
            ("=",     builtins::EQ),
            ("null?", builtins::NULL_P),
            ("atom?", builtins::ATOM_P),
            ("list",  builtins::LIST),
            ("not",   builtins::NOT),
            ("print", builtins::PRINT),
            ("<",     builtins::LT),
            (">",     builtins::GT),
        ];
        let global = self.env.global_id();
        for (name, op) in &pairs {
            let sym_id = self.symbols.intern(name);
            self.env.define(global, sym_id, Word::Builtin(*op));
        }
    }
}

impl Default for LispMachine {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer_eval() {
        let mut m = LispMachine::new();
        let result = m.eval_str("42").unwrap();
        assert_eq!(result, Word::Int(42));
    }

    #[test]
    fn test_addition() {
        let mut m = LispMachine::new();
        let result = m.eval_str("(+ 1 2)").unwrap();
        assert_eq!(result, Word::Int(3));
    }

    #[test]
    fn test_define_and_lookup() {
        let mut m = LispMachine::new();
        m.eval_str("(define x 99)").unwrap();
        let result = m.eval_str("x").unwrap();
        assert_eq!(result, Word::Int(99));
    }

    #[test]
    fn test_if_true() {
        let mut m = LispMachine::new();
        let result = m.eval_str("(if #t 1 2)").unwrap();
        assert_eq!(result, Word::Int(1));
    }

    #[test]
    fn test_if_false() {
        let mut m = LispMachine::new();
        let result = m.eval_str("(if #f 1 2)").unwrap();
        assert_eq!(result, Word::Int(2));
    }

    #[test]
    fn test_lambda() {
        let mut m = LispMachine::new();
        m.eval_str("(define square (lambda (x) (* x x)))").unwrap();
        let result = m.eval_str("(square 7)").unwrap();
        assert_eq!(result, Word::Int(49));
    }

    #[test]
    fn test_checkpoint_and_chain() {
        let mut m = LispMachine::new();
        m.eval_str("(define tick-test 1)").unwrap();
        let seal = m.checkpoint("test").unwrap();
        assert!(!seal.is_empty());
        assert_eq!(m.vault.chain_length(), 1);
        assert_eq!(m.tick, 1);
    }

    #[test]
    fn test_nested_arithmetic() {
        let mut m = LispMachine::new();
        let result = m.eval_str("(* (+ 2 3) (- 10 4))").unwrap();
        assert_eq!(result, Word::Int(30));
    }

    #[test]
    fn test_cons_car_cdr() {
        let mut m = LispMachine::new();
        let pair = m.eval_str("(cons 1 2)").unwrap();
        let pair_ptr = m.heap.alloc(pair);
        m.env.define(0, m.symbols.intern("p"), Word::Cons(pair_ptr, m.heap.alloc(Word::Nil)));
        // Just test cons + car basic roundtrip
        let result = m.eval_str("(car (cons 10 20))").unwrap();
        assert_eq!(result, Word::Int(10));
    }

    #[test]
    fn test_display() {
        let mut m = LispMachine::new();
        let r = m.eval_str("(list 1 2 3)").unwrap();
        let s = m.display(&r);
        assert!(s.contains("1"));
    }
}
