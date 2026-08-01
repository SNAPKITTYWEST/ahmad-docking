use crate::lisp::word::Word;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single environment frame — one scope level.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvFrame {
    pub id: u32,
    pub parent_id: Option<u32>,
    pub bindings: HashMap<u32, Word>, // symbol_id → value
}

/// The full environment store — a flat map of frame id → frame.
/// Frames are never mutated after creation (except the global frame).
/// Closures capture their frame_id; lookup walks the parent chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EnvStore {
    frames: HashMap<u32, EnvFrame>,
    next_id: u32,
}

impl EnvStore {
    pub fn new() -> Self {
        let mut store = Self {
            frames: HashMap::new(),
            next_id: 1,
        };
        // Frame 0 = global env
        store.frames.insert(0, EnvFrame {
            id: 0,
            parent_id: None,
            bindings: HashMap::new(),
        });
        store
    }

    pub fn global_id(&self) -> u32 { 0 }

    /// Create a new child frame under parent_id.
    pub fn extend(&mut self, parent_id: u32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.frames.insert(id, EnvFrame {
            id,
            parent_id: Some(parent_id),
            bindings: HashMap::new(),
        });
        id
    }

    /// Look up a symbol in frame_id, walking parent chain.
    pub fn get(&self, frame_id: u32, sym: u32) -> Option<Word> {
        let mut current_id = Some(frame_id);
        while let Some(fid) = current_id {
            if let Some(frame) = self.frames.get(&fid) {
                if let Some(val) = frame.bindings.get(&sym) {
                    return Some(val.clone());
                }
                current_id = frame.parent_id;
            } else {
                break;
            }
        }
        None
    }

    /// Define a binding in the given frame (not parents).
    pub fn define(&mut self, frame_id: u32, sym: u32, val: Word) {
        if let Some(frame) = self.frames.get_mut(&frame_id) {
            frame.bindings.insert(sym, val);
        }
    }

    /// Set an existing binding, walking up to find it.
    /// If not found anywhere, defines it in frame_id.
    pub fn set(&mut self, frame_id: u32, sym: u32, val: Word) {
        let mut current_id = Some(frame_id);
        while let Some(fid) = current_id {
            if let Some(frame) = self.frames.get_mut(&fid) {
                if frame.bindings.contains_key(&sym) {
                    frame.bindings.insert(sym, val);
                    return;
                }
                current_id = frame.parent_id;
            } else {
                break;
            }
        }
        // Not found — define in given frame
        self.define(frame_id, sym, val);
    }

    pub fn frame(&self, id: u32) -> Option<&EnvFrame> {
        self.frames.get(&id)
    }

    /// Mark all heap pointers reachable from this env store.
    /// Called during GC mark phase.
    pub fn mark_roots(&self, heap: &mut crate::lisp::heap::Heap) {
        for frame in self.frames.values() {
            for val in frame.bindings.values() {
                mark_word(val, heap);
            }
        }
    }
}

impl Default for EnvStore {
    fn default() -> Self { Self::new() }
}

pub fn mark_word(word: &Word, heap: &mut crate::lisp::heap::Heap) {
    match word {
        Word::Cons(car, cdr) => {
            heap.mark(*car);
            heap.mark(*cdr);
        }
        Word::Closure { params, body, .. } => {
            heap.mark(*params);
            heap.mark(*body);
        }
        _ => {}
    }
}
