use crate::lisp::machine::LispMachine;
use std::io::{self, Write, BufRead};

/// The REPL — thought → expression → result → thought.
/// Continuous. No compile-link-run cycle.
pub struct Repl {
    pub machine: LispMachine,
    pub history: Vec<(String, String)>, // (input, output)
}

impl Repl {
    pub fn new() -> Self {
        Self {
            machine: LispMachine::new(),
            history: Vec::new(),
        }
    }

    pub fn run(&mut self) {
        println!("SnapKitty LISP Machine v0.1");
        println!("Tick: {} | Heap: {} cells | Chain: {} seals",
            self.machine.tick,
            self.machine.heap.len(),
            self.machine.vault.chain_length(),
        );
        println!("(seal! to checkpoint world state, (quit) to exit)\n");

        let stdin = io::stdin();
        loop {
            print!("λ> ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if stdin.lock().read_line(&mut line).is_err() { break; }
            let line = line.trim().to_string();
            if line.is_empty() { continue; }
            if line == "(quit)" || line == "quit" { break; }

            match self.eval_line(&line) {
                Ok(output) => {
                    println!("=> {}", output);
                    self.history.push((line, output));
                }
                Err(e) => {
                    eprintln!("! {}", e);
                }
            }
        }

        println!("\nMachine halted. Sealing final state...");
        if let Err(e) = self.machine.checkpoint("REPL") {
            eprintln!("Seal failed: {}", e);
        } else {
            println!("Final world state sealed. Chain length: {}", self.machine.vault.chain_length());
        }
    }

    pub fn eval_line(&mut self, input: &str) -> Result<String, String> {
        if input == "seal!" {
            self.machine.checkpoint("REPL")
                .map(|seal| format!("sealed: {}", &seal[..16]))
                .map_err(|e| e.to_string())
        } else if input == "stats" {
            let stats = self.machine.heap.stats();
            Ok(format!(
                "tick={} heap={} live={} free={} gc_runs={}",
                self.machine.tick,
                stats.total_cells,
                stats.live_cells,
                stats.free_cells,
                stats.gc_count,
            ))
        } else {
            let result = self.machine.eval_str(input)
                .map_err(|e| e.to_string())?;
            Ok(self.machine.display(&result))
        }
    }
}

impl Default for Repl {
    fn default() -> Self { Self::new() }
}
