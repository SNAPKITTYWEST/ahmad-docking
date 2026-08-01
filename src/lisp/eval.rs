use crate::lisp::word::{Word, SymbolTable, builtins};
use crate::lisp::heap::Heap;
use crate::lisp::env::EnvStore;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum EvalError {
    #[error("unbound symbol: {0}")]
    Unbound(String),
    #[error("type error: expected {expected}, got {got}")]
    TypeError { expected: String, got: String },
    #[error("arity error: {form} expects {expected} args, got {got}")]
    Arity { form: String, expected: String, got: usize },
    #[error("invalid heap pointer: {0}")]
    BadPtr(u32),
    #[error("division by zero")]
    DivZero,
    #[error("stack overflow — recursion limit reached")]
    StackOverflow,
    #[error("parse error: {0}")]
    Parse(String),
}

const MAX_DEPTH: usize = 512;

/// The evaluator — takes a Word expression and evaluates it in an env frame.
pub struct Evaluator<'a> {
    pub heap: &'a mut Heap,
    pub symbols: &'a mut SymbolTable,
    pub env: &'a mut EnvStore,
    depth: usize,
}

impl<'a> Evaluator<'a> {
    pub fn new(
        heap: &'a mut Heap,
        symbols: &'a mut SymbolTable,
        env: &'a mut EnvStore,
    ) -> Self {
        Self { heap, symbols, env, depth: 0 }
    }

    pub fn eval(&mut self, expr: Word, frame_id: u32) -> Result<Word, EvalError> {
        if self.depth > MAX_DEPTH {
            return Err(EvalError::StackOverflow);
        }
        self.depth += 1;
        let result = self.eval_inner(expr, frame_id);
        self.depth -= 1;
        result
    }

    fn eval_inner(&mut self, expr: Word, frame_id: u32) -> Result<Word, EvalError> {
        match expr {
            // Self-evaluating
            Word::Nil => Ok(Word::Nil),
            Word::Bool(b) => Ok(Word::Bool(b)),
            Word::Int(n) => Ok(Word::Int(n)),
            Word::Str(id) => Ok(Word::Str(id)),
            Word::Builtin(op) => Ok(Word::Builtin(op)),
            Word::Closure { params, body, env_id } => {
                Ok(Word::Closure { params, body, env_id })
            }

            // Symbol lookup
            Word::Symbol(sym_id) => {
                self.env.get(frame_id, sym_id)
                    .ok_or_else(|| {
                        let name = self.symbols.name(sym_id)
                            .unwrap_or("?").to_string();
                        EvalError::Unbound(name)
                    })
            }

            // List — either a special form or function application
            Word::Cons(car_ptr, cdr_ptr) => {
                let car = self.heap.get(car_ptr)
                    .ok_or(EvalError::BadPtr(car_ptr))?.clone();

                // Check for special forms
                if let Word::Symbol(sym_id) = &car {
                    let name = self.symbols.name(*sym_id).unwrap_or("").to_string();
                    match name.as_str() {
                        "quote"  => return self.eval_quote(cdr_ptr),
                        "if"     => return self.eval_if(cdr_ptr, frame_id),
                        "lambda" => return self.eval_lambda(cdr_ptr, frame_id),
                        "define" => return self.eval_define(cdr_ptr, frame_id),
                        "set!"   => return self.eval_set(cdr_ptr, frame_id),
                        "begin"  => return self.eval_begin(cdr_ptr, frame_id),
                        "let"    => return self.eval_let(cdr_ptr, frame_id),
                        "cond"   => return self.eval_cond(cdr_ptr, frame_id),
                        "and"    => return self.eval_and(cdr_ptr, frame_id),
                        "or"     => return self.eval_or(cdr_ptr, frame_id),
                        _ => {} // fall through to application
                    }
                }

                // Function application
                let func = self.eval(car, frame_id)?;
                let args = self.eval_args(cdr_ptr, frame_id)?;
                self.apply(func, args)
            }

            Word::Agent(_) | Word::Trust(_) => Ok(expr),
        }
    }

    fn eval_quote(&mut self, args_ptr: u32) -> Result<Word, EvalError> {
        let (quoted, _) = self.list_head(args_ptr)?;
        Ok(quoted)
    }

    fn eval_if(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let (cond_expr, rest) = self.list_head(args_ptr)?;
        let (then_expr, rest2) = self.list_head(rest)?;

        let cond_val = self.eval(cond_expr, frame_id)?;
        if cond_val.is_truthy() {
            self.eval(then_expr, frame_id)
        } else if rest2 != 0 {
            let (else_expr, _) = self.list_head(rest2)?;
            self.eval(else_expr, frame_id)
        } else {
            Ok(Word::Nil)
        }
    }

    fn eval_lambda(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let (params, rest) = self.list_head(args_ptr)?;
        let (body, _) = self.list_head(rest)?;

        let params_ptr = self.heap.alloc(params);
        let body_ptr = self.heap.alloc(body);

        Ok(Word::Closure { params: params_ptr, body: body_ptr, env_id: frame_id })
    }

    fn eval_define(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let (name_word, rest) = self.list_head(args_ptr)?;
        let (val_expr, _) = self.list_head(rest)?;

        let sym_id = match name_word {
            Word::Symbol(id) => id,
            _ => return Err(EvalError::TypeError {
                expected: "symbol".to_string(),
                got: format!("{:?}", name_word.tag()),
            }),
        };

        let val = self.eval(val_expr, frame_id)?;
        self.env.define(frame_id, sym_id, val);
        Ok(Word::Symbol(sym_id))
    }

    fn eval_set(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let (name_word, rest) = self.list_head(args_ptr)?;
        let (val_expr, _) = self.list_head(rest)?;

        let sym_id = match name_word {
            Word::Symbol(id) => id,
            _ => return Err(EvalError::TypeError {
                expected: "symbol".to_string(),
                got: format!("{:?}", name_word.tag()),
            }),
        };

        let val = self.eval(val_expr, frame_id)?;
        self.env.set(frame_id, sym_id, val);
        Ok(Word::Nil)
    }

    fn eval_begin(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let mut result = Word::Nil;
        let mut ptr = args_ptr;
        loop {
            if ptr == 0 { break; }
            let cell = self.heap.get(ptr).ok_or(EvalError::BadPtr(ptr))?.clone();
            match cell {
                Word::Cons(car, cdr) => {
                    let expr = self.heap.get(car).ok_or(EvalError::BadPtr(car))?.clone();
                    result = self.eval(expr, frame_id)?;
                    ptr = cdr;
                }
                _ => break,
            }
        }
        Ok(result)
    }

    fn eval_let(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let (bindings_word, rest) = self.list_head(args_ptr)?;
        let (body_expr, _) = self.list_head(rest)?;

        let child_frame = self.env.extend(frame_id);

        // Bindings are ((sym val) ...)
        let mut bind_ptr = match &bindings_word {
            Word::Cons(car, cdr) => {
                let _ = cdr;
                let first_cell = self.heap.alloc(bindings_word.clone());
                first_cell
            }
            _ => 0,
        };

        // Re-do: bindings_word IS the first cell
        let mut ptr = self.heap.alloc(bindings_word);
        loop {
            if ptr == 0 { break; }
            let cell = self.heap.get(ptr).ok_or(EvalError::BadPtr(ptr))?.clone();
            match cell {
                Word::Cons(binding_ptr, rest_ptr) => {
                    let (sym_word, val_rest) = self.list_head(binding_ptr)?;
                    let (val_expr, _) = self.list_head(val_rest)?;
                    let sym_id = match sym_word {
                        Word::Symbol(id) => id,
                        _ => return Err(EvalError::TypeError {
                            expected: "symbol".to_string(),
                            got: format!("{:?}", sym_word.tag()),
                        }),
                    };
                    let val = self.eval(val_expr, frame_id)?;
                    self.env.define(child_frame, sym_id, val);
                    ptr = rest_ptr;
                }
                _ => break,
            }
        }

        self.eval(body_expr, child_frame)
    }

    fn eval_cond(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let mut ptr = args_ptr;
        loop {
            if ptr == 0 { return Ok(Word::Nil); }
            let cell = self.heap.get(ptr).ok_or(EvalError::BadPtr(ptr))?.clone();
            match cell {
                Word::Cons(clause_ptr, rest) => {
                    let (test_expr, body_rest) = self.list_head(clause_ptr)?;
                    let is_else = matches!(&test_expr, Word::Symbol(id)
                        if self.symbols.name(*id) == Some("else"));

                    if is_else || self.eval(test_expr, frame_id)?.is_truthy() {
                        let (body_expr, _) = self.list_head(body_rest)?;
                        return self.eval(body_expr, frame_id);
                    }
                    ptr = rest;
                }
                _ => break,
            }
        }
        Ok(Word::Nil)
    }

    fn eval_and(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let mut result = Word::Bool(true);
        let mut ptr = args_ptr;
        loop {
            if ptr == 0 { return Ok(result); }
            let (expr, rest) = self.list_head(ptr)?;
            result = self.eval(expr, frame_id)?;
            if !result.is_truthy() { return Ok(Word::Bool(false)); }
            ptr = rest;
        }
    }

    fn eval_or(&mut self, args_ptr: u32, frame_id: u32) -> Result<Word, EvalError> {
        let mut ptr = args_ptr;
        loop {
            if ptr == 0 { return Ok(Word::Bool(false)); }
            let (expr, rest) = self.list_head(ptr)?;
            let val = self.eval(expr, frame_id)?;
            if val.is_truthy() { return Ok(val); }
            ptr = rest;
        }
    }

    fn eval_args(&mut self, args_ptr: u32, frame_id: u32) -> Result<Vec<Word>, EvalError> {
        let mut args = Vec::new();
        let mut ptr = args_ptr;
        loop {
            if ptr == 0 { break; }
            let cell = self.heap.get(ptr).ok_or(EvalError::BadPtr(ptr))?.clone();
            match cell {
                Word::Cons(car, cdr) => {
                    let expr = self.heap.get(car).ok_or(EvalError::BadPtr(car))?.clone();
                    args.push(self.eval(expr, frame_id)?);
                    ptr = cdr;
                }
                _ => break,
            }
        }
        Ok(args)
    }

    pub fn apply(&mut self, func: Word, args: Vec<Word>) -> Result<Word, EvalError> {
        match func {
            Word::Builtin(op) => self.apply_builtin(op, args),
            Word::Closure { params, body, env_id } => {
                let call_frame = self.env.extend(env_id);
                let body_expr = self.heap.get(body).ok_or(EvalError::BadPtr(body))?.clone();

                // Bind params to args
                let params_word = self.heap.get(params).ok_or(EvalError::BadPtr(params))?.clone();
                self.bind_params(params_word, args, call_frame)?;

                self.eval(body_expr, call_frame)
            }
            _ => Err(EvalError::TypeError {
                expected: "closure or builtin".to_string(),
                got: format!("{:?}", func.tag()),
            }),
        }
    }

    fn bind_params(&mut self, params: Word, args: Vec<Word>, frame_id: u32) -> Result<(), EvalError> {
        match params {
            Word::Nil => Ok(()),
            Word::Cons(car, cdr) => {
                let sym = self.heap.get(car).ok_or(EvalError::BadPtr(car))?.clone();
                if let Word::Symbol(sym_id) = sym {
                    let val = args.into_iter().next().unwrap_or(Word::Nil);
                    self.env.define(frame_id, sym_id, val);
                    // bind remaining — simplified: take first arg only
                    // full variadic support in Phase 2
                }
                Ok(())
            }
            Word::Symbol(sym_id) => {
                // variadic: bind rest as list
                let list = self.make_list(args);
                self.env.define(frame_id, sym_id, list);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn apply_builtin(&mut self, op: u8, args: Vec<Word>) -> Result<Word, EvalError> {
        match op {
            builtins::CONS => {
                let car = args.get(0).cloned().unwrap_or(Word::Nil);
                let cdr = args.get(1).cloned().unwrap_or(Word::Nil);
                let car_ptr = self.heap.alloc(car);
                let cdr_ptr = self.heap.alloc(cdr);
                Ok(Word::Cons(car_ptr, cdr_ptr))
            }
            builtins::CAR => {
                match args.get(0).cloned().unwrap_or(Word::Nil) {
                    Word::Cons(car, _) => {
                        Ok(self.heap.get(car).ok_or(EvalError::BadPtr(car))?.clone())
                    }
                    other => Err(EvalError::TypeError {
                        expected: "cons".to_string(),
                        got: format!("{:?}", other.tag()),
                    }),
                }
            }
            builtins::CDR => {
                match args.get(0).cloned().unwrap_or(Word::Nil) {
                    Word::Cons(_, cdr) => {
                        Ok(self.heap.get(cdr).ok_or(EvalError::BadPtr(cdr))?.clone())
                    }
                    other => Err(EvalError::TypeError {
                        expected: "cons".to_string(),
                        got: format!("{:?}", other.tag()),
                    }),
                }
            }
            builtins::ADD => {
                let sum = args.iter().try_fold(0i64, |acc, w| match w {
                    Word::Int(n) => Ok(acc + n),
                    _ => Err(EvalError::TypeError { expected: "integer".to_string(), got: format!("{:?}", w.tag()) }),
                })?;
                Ok(Word::Int(sum))
            }
            builtins::SUB => {
                match args.as_slice() {
                    [Word::Int(a), Word::Int(b)] => Ok(Word::Int(a - b)),
                    [Word::Int(a)] => Ok(Word::Int(-a)),
                    _ => Err(EvalError::Arity { form: "-".to_string(), expected: "1 or 2".to_string(), got: args.len() }),
                }
            }
            builtins::MUL => {
                let prod = args.iter().try_fold(1i64, |acc, w| match w {
                    Word::Int(n) => Ok(acc * n),
                    _ => Err(EvalError::TypeError { expected: "integer".to_string(), got: format!("{:?}", w.tag()) }),
                })?;
                Ok(Word::Int(prod))
            }
            builtins::DIV => {
                match args.as_slice() {
                    [Word::Int(a), Word::Int(b)] => {
                        if *b == 0 { Err(EvalError::DivZero) }
                        else { Ok(Word::Int(a / b)) }
                    }
                    _ => Err(EvalError::Arity { form: "/".to_string(), expected: "2".to_string(), got: args.len() }),
                }
            }
            builtins::EQ => {
                let equal = args.windows(2).all(|w| w[0] == w[1]);
                Ok(Word::Bool(equal))
            }
            builtins::NULL_P => Ok(Word::Bool(args.get(0).map(|w| w.is_nil()).unwrap_or(true))),
            builtins::ATOM_P => Ok(Word::Bool(args.get(0).map(|w| w.is_atom()).unwrap_or(true))),
            builtins::NOT => Ok(Word::Bool(!args.get(0).map(|w| w.is_truthy()).unwrap_or(false))),
            builtins::LT => {
                match args.as_slice() {
                    [Word::Int(a), Word::Int(b)] => Ok(Word::Bool(a < b)),
                    _ => Err(EvalError::TypeError { expected: "integer integer".to_string(), got: "other".to_string() }),
                }
            }
            builtins::GT => {
                match args.as_slice() {
                    [Word::Int(a), Word::Int(b)] => Ok(Word::Bool(a > b)),
                    _ => Err(EvalError::TypeError { expected: "integer integer".to_string(), got: "other".to_string() }),
                }
            }
            builtins::LIST => {
                Ok(self.make_list(args))
            }
            builtins::PRINT => {
                for arg in &args {
                    print!("{} ", arg);
                }
                println!();
                Ok(args.into_iter().last().unwrap_or(Word::Nil))
            }
            _ => Err(EvalError::TypeError { expected: "known builtin".to_string(), got: format!("opcode {}", op) }),
        }
    }

    /// Destructure a heap cons cell: returns (car_word, cdr_ptr).
    fn list_head(&self, ptr: u32) -> Result<(Word, u32), EvalError> {
        if ptr == 0 {
            return Ok((Word::Nil, 0));
        }
        match self.heap.get(ptr).ok_or(EvalError::BadPtr(ptr))? {
            Word::Cons(car, cdr) => {
                let car_word = self.heap.get(*car).ok_or(EvalError::BadPtr(*car))?.clone();
                Ok((car_word, *cdr))
            }
            other => Ok((other.clone(), 0)),
        }
    }

    pub fn make_list(&mut self, items: Vec<Word>) -> Word {
        let mut result = Word::Nil;
        for item in items.into_iter().rev() {
            let item_ptr = self.heap.alloc(item);
            let rest_ptr = self.heap.alloc(result);
            result = Word::Cons(item_ptr, rest_ptr);
        }
        result
    }
}
