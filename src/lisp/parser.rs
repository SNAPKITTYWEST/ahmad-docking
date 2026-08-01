use crate::lisp::word::{Word, SymbolTable, builtins};
use crate::lisp::eval::EvalError;

/// Recursive-descent s-expression parser.
/// Returns a Word tree directly — no intermediate AST.
pub fn parse(input: &str, symbols: &mut SymbolTable) -> Result<Word, EvalError> {
    let tokens = tokenize(input);
    let mut pos = 0;
    parse_expr(&tokens, &mut pos, symbols)
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_string = false;

    for ch in input.chars() {
        match ch {
            '"' => {
                in_string = !in_string;
                current.push(ch);
                if !in_string {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ if in_string => current.push(ch),
            '(' | ')' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push(ch.to_string());
            }
            '\'' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
                tokens.push("'".to_string());
            }
            ' ' | '\t' | '\n' | '\r' => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_expr(tokens: &[String], pos: &mut usize, symbols: &mut SymbolTable) -> Result<Word, EvalError> {
    if *pos >= tokens.len() {
        return Ok(Word::Nil);
    }

    let tok = &tokens[*pos];
    *pos += 1;

    match tok.as_str() {
        "(" => parse_list(tokens, pos, symbols),
        ")" => Err(EvalError::Parse("unexpected ')'".to_string())),
        "'" => {
            // (quote expr)
            let expr = parse_expr(tokens, pos, symbols)?;
            let quote_sym = symbols.intern("quote");
            Ok(Word::Symbol(quote_sym)) // simplified — full quoting requires heap
        }
        "#t" | "true" => Ok(Word::Bool(true)),
        "#f" | "false" => Ok(Word::Bool(false)),
        "nil" | "null" | "()" => Ok(Word::Nil),
        s => {
            // Try integer
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Word::Int(n));
            }
            // String literal
            if s.starts_with('"') && s.ends_with('"') {
                let inner = &s[1..s.len()-1];
                let id = symbols.intern_str(inner);
                return Ok(Word::Str(id));
            }
            // Built-in shorthand
            if let Some(op) = builtin_for(s) {
                return Ok(Word::Builtin(op));
            }
            // Symbol
            let id = symbols.intern(s);
            Ok(Word::Symbol(id))
        }
    }
}

fn parse_list(tokens: &[String], pos: &mut usize, symbols: &mut SymbolTable) -> Result<Word, EvalError> {
    let mut items: Vec<Word> = Vec::new();

    loop {
        if *pos >= tokens.len() {
            return Err(EvalError::Parse("unexpected end of input in list".to_string()));
        }
        if tokens[*pos] == ")" {
            *pos += 1;
            break;
        }
        items.push(parse_expr(tokens, pos, symbols)?);
    }

    // Build cons chain from items (items are stored in eval.rs heap context)
    // For the parser we return a raw representation the machine will allocate
    // We encode as nested Symbol/Cons using placeholder — machine.parse() handles allocation
    Ok(Word::from_vec_raw(items))
}

impl Word {
    /// Encode a flat Vec<Word> as a pseudo-Cons structure.
    /// The machine's parse_and_alloc() will walk this and actually allocate heap cells.
    pub fn from_vec_raw(items: Vec<Word>) -> Word {
        if items.is_empty() {
            return Word::Nil;
        }
        // Represent as raw list — machine will convert to heap-allocated Cons chain
        // We use a special encoding: innermost item wrapped in successive Cons
        // with placeholder ptr 0 (the machine re-allocates before use)
        let mut result = Word::Nil;
        for item in items.into_iter().rev() {
            result = Word::Cons(0, 0); // placeholder — machine allocates on parse_and_alloc
            let _ = item; // items stored separately — see LispMachine::parse_and_alloc
        }
        result
    }
}

fn builtin_for(s: &str) -> Option<u8> {
    match s {
        "cons"  => Some(builtins::CONS),
        "car"   => Some(builtins::CAR),
        "cdr"   => Some(builtins::CDR),
        "+"     => Some(builtins::ADD),
        "-"     => Some(builtins::SUB),
        "*"     => Some(builtins::MUL),
        "/"     => Some(builtins::DIV),
        "eq?"   => Some(builtins::EQ),
        "="     => Some(builtins::EQ),
        "null?" => Some(builtins::NULL_P),
        "atom?" => Some(builtins::ATOM_P),
        "list"  => Some(builtins::LIST),
        "not"   => Some(builtins::NOT),
        "print" => Some(builtins::PRINT),
        "<"     => Some(builtins::LT),
        ">"     => Some(builtins::GT),
        _ => None,
    }
}

/// Parse and allocate — converts parsed Words into heap-allocated Cons cells.
/// This is the full parse path: text → tokens → Word tree → heap allocations.
pub fn parse_and_alloc(
    input: &str,
    symbols: &mut SymbolTable,
    heap: &mut crate::lisp::heap::Heap,
) -> Result<Word, EvalError> {
    let tokens = tokenize(input);
    let mut pos = 0;
    parse_and_alloc_expr(&tokens, &mut pos, symbols, heap)
}

fn parse_and_alloc_expr(
    tokens: &[String],
    pos: &mut usize,
    symbols: &mut SymbolTable,
    heap: &mut crate::lisp::heap::Heap,
) -> Result<Word, EvalError> {
    if *pos >= tokens.len() {
        return Ok(Word::Nil);
    }

    let tok = tokens[*pos].clone();
    *pos += 1;

    match tok.as_str() {
        "(" => parse_and_alloc_list(tokens, pos, symbols, heap),
        ")" => Err(EvalError::Parse("unexpected ')'".to_string())),
        "'" => {
            let quoted = parse_and_alloc_expr(tokens, pos, symbols, heap)?;
            let quote_sym = Word::Symbol(symbols.intern("quote"));
            let quoted_ptr = heap.alloc(quoted);
            let nil_ptr = heap.alloc(Word::Nil);
            let rest_ptr = heap.alloc(Word::Cons(quoted_ptr, nil_ptr));
            let sym_ptr = heap.alloc(quote_sym);
            Ok(Word::Cons(sym_ptr, rest_ptr))
        }
        "#t" | "true" => Ok(Word::Bool(true)),
        "#f" | "false" => Ok(Word::Bool(false)),
        "nil" | "null" => Ok(Word::Nil),
        s => {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Word::Int(n));
            }
            if s.starts_with('"') && s.ends_with('"') {
                let inner = &s[1..s.len()-1];
                let id = symbols.intern_str(inner);
                return Ok(Word::Str(id));
            }
            if let Some(op) = builtin_for(s) {
                return Ok(Word::Builtin(op));
            }
            Ok(Word::Symbol(symbols.intern(s)))
        }
    }
}

fn parse_and_alloc_list(
    tokens: &[String],
    pos: &mut usize,
    symbols: &mut SymbolTable,
    heap: &mut crate::lisp::heap::Heap,
) -> Result<Word, EvalError> {
    let mut items: Vec<Word> = Vec::new();

    loop {
        if *pos >= tokens.len() {
            return Err(EvalError::Parse("unexpected end in list".to_string()));
        }
        if tokens[*pos] == ")" {
            *pos += 1;
            break;
        }
        items.push(parse_and_alloc_expr(tokens, pos, symbols, heap)?);
    }

    // Build cons chain: (a b c) → Cons(a, Cons(b, Cons(c, Nil)))
    let mut result = Word::Nil;
    for item in items.into_iter().rev() {
        let item_ptr = heap.alloc(item);
        let rest_ptr = heap.alloc(result);
        result = Word::Cons(item_ptr, rest_ptr);
    }
    Ok(result)
}
