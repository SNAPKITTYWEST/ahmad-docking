use crate::lisp::word::Word;
use serde::{Deserialize, Serialize};

/// The agent heap — all LISP values live here.
/// Allocation returns a u32 pointer (index). GC is mark-and-sweep.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heap {
    cells: Vec<Word>,
    marks: Vec<bool>,
    free_list: Vec<u32>,
    alloc_count: u64,
    gc_count: u64,
}

impl Heap {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            marks: Vec::new(),
            free_list: Vec::new(),
            alloc_count: 0,
            gc_count: 0,
        }
    }

    /// Allocate a new cell. Returns a stable pointer (index).
    pub fn alloc(&mut self, word: Word) -> u32 {
        self.alloc_count += 1;
        if let Some(idx) = self.free_list.pop() {
            self.cells[idx as usize] = word;
            self.marks[idx as usize] = false;
            idx
        } else {
            let idx = self.cells.len() as u32;
            self.cells.push(word);
            self.marks.push(false);
            idx
        }
    }

    pub fn get(&self, ptr: u32) -> Option<&Word> {
        self.cells.get(ptr as usize)
    }

    pub fn get_mut(&mut self, ptr: u32) -> Option<&mut Word> {
        self.cells.get_mut(ptr as usize)
    }

    /// Mark a cell as live (reachable).
    pub fn mark(&mut self, ptr: u32) {
        if (ptr as usize) < self.marks.len() && !self.marks[ptr as usize] {
            self.marks[ptr as usize] = true;
            // Recursively mark referenced cells
            match self.cells[ptr as usize].clone() {
                Word::Cons(car, cdr) => {
                    self.mark(car);
                    self.mark(cdr);
                }
                Word::Closure { params, body, env_id } => {
                    self.mark(params);
                    self.mark(body);
                    // env_id is an env frame id, not a heap ptr — skip
                    let _ = env_id;
                }
                _ => {}
            }
        }
    }

    /// Sweep: free all unmarked cells.
    pub fn sweep(&mut self) -> usize {
        self.gc_count += 1;
        let mut freed = 0;
        for i in 0..self.marks.len() {
            if !self.marks[i] {
                if !matches!(self.cells[i], Word::Nil) {
                    self.cells[i] = Word::Nil;
                    self.free_list.push(i as u32);
                    freed += 1;
                }
            }
            self.marks[i] = false;
        }
        freed
    }

    pub fn len(&self) -> usize { self.cells.len() }
    pub fn free_count(&self) -> usize { self.free_list.len() }
    pub fn alloc_count(&self) -> u64 { self.alloc_count }
    pub fn gc_count(&self) -> u64 { self.gc_count }

    /// Stats for world dump metadata
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            total_cells: self.cells.len(),
            free_cells: self.free_list.len(),
            live_cells: self.cells.len() - self.free_list.len(),
            alloc_count: self.alloc_count,
            gc_count: self.gc_count,
        }
    }
}

impl Default for Heap {
    fn default() -> Self { Self::new() }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeapStats {
    pub total_cells: usize,
    pub free_cells: usize,
    pub live_cells: usize,
    pub alloc_count: u64,
    pub gc_count: u64,
}
