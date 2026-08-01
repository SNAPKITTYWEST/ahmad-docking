//! SnapKitty Virtual LISP Machine
//!
//! A persistent, WORM-sealed LISP environment where agents are first-class heap objects.
//! The machine never forgets — every tick is sealed to the chain.
//! Restore from any prior sealed state. Fork. Inspect. Continue.
//!
//! ## Architecture
//!
//! - `word`    — Tagged word type (the fundamental data type)
//! - `heap`    — Agent heap with mark-and-sweep GC
//! - `env`     — Lexical environment store (frame-based)
//! - `eval`    — The evaluator — derives actions, no instructions
//! - `parser`  — S-expression parser → heap-allocated Cons chains
//! - `world`   — World dump + WORM chain integration (the soul)
//! - `machine` — LispMachine struct (owns everything)
//! - `repl`    — The REPL — thought → expression → result → thought

pub mod word;
pub mod heap;
pub mod env;
pub mod eval;
pub mod parser;
pub mod world;
pub mod machine;
pub mod repl;

pub use word::{Word, Tag, SymbolTable};
pub use heap::{Heap, HeapStats};
pub use env::EnvStore;
pub use eval::EvalError;
pub use world::{WorldDump, WorldVault};
pub use machine::LispMachine;
pub use repl::Repl;
