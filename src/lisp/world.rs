use crate::lisp::heap::{Heap, HeapStats};
use crate::lisp::env::EnvStore;
use crate::lisp::word::SymbolTable;
use crate::chain::Chain;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

/// A complete, serializable snapshot of the LISP machine state.
/// This IS the world dump — the soul of the machine.
/// Hash it, seal it to the Chain, and the machine can be restored from any prior tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorldDump {
    pub tick: u64,
    pub heap: Heap,
    pub symbols: SymbolTable,
    pub env: EnvStore,
    pub stats: HeapStats,
    pub seal: String,           // SHA-256 of the dump (excluding this field)
    pub prev_seal: Option<String>,
    pub timestamp_ms: u64,
    pub agent_id: String,
}

impl WorldDump {
    pub fn new(
        tick: u64,
        heap: Heap,
        symbols: SymbolTable,
        env: EnvStore,
        prev_seal: Option<String>,
        agent_id: String,
    ) -> Self {
        let stats = heap.stats();
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut dump = Self {
            tick,
            heap,
            symbols,
            env,
            stats,
            seal: String::new(),
            prev_seal,
            timestamp_ms,
            agent_id,
        };

        dump.seal = dump.compute_seal();
        dump
    }

    fn compute_seal(&self) -> String {
        // Serialize without the seal field
        let payload = serde_json::json!({
            "tick": self.tick,
            "heap_len": self.stats.total_cells,
            "live_cells": self.stats.live_cells,
            "prev_seal": self.prev_seal,
            "timestamp_ms": self.timestamp_ms,
            "agent_id": self.agent_id,
            // Full heap hash would be computed here in production
            // For Phase 1 we hash the stats + tick as the world fingerprint
        });

        let json = serde_json::to_string(&payload).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(json.as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn verify(&self) -> bool {
        let expected = self.compute_seal();
        self.seal == expected
    }

    /// Serialize the full dump to JSON bytes for WORM chain entry.
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Restore from serialized bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

/// Manages world dump lifecycle — create, seal to Chain, restore.
pub struct WorldVault {
    chain: Chain,
    dumps: Vec<WorldDump>,
}

impl WorldVault {
    pub fn new() -> Self {
        Self {
            chain: Chain::new(),
            dumps: Vec::new(),
        }
    }

    /// Seal a world dump to the immutable chain.
    pub fn seal_dump(&mut self, dump: WorldDump) -> Result<String, crate::chain::ChainError> {
        let payload = serde_json::to_string(&serde_json::json!({
            "type": "world_dump",
            "tick": dump.tick,
            "seal": dump.seal,
            "agent_id": dump.agent_id,
            "timestamp_ms": dump.timestamp_ms,
            "heap_stats": {
                "total_cells": dump.stats.total_cells,
                "live_cells": dump.stats.live_cells,
            }
        })).unwrap_or_default();

        let chain_seal = self.chain.append(&payload)?;
        let seal_hash = chain_seal.seal_hash()
            .map(|h| hex::encode(h))
            .unwrap_or_else(|_| dump.seal.clone());

        self.dumps.push(dump);
        Ok(seal_hash)
    }

    /// Get the most recent world dump.
    pub fn latest(&self) -> Option<&WorldDump> {
        self.dumps.last()
    }

    /// Get a dump by tick number.
    pub fn at_tick(&self, tick: u64) -> Option<&WorldDump> {
        self.dumps.iter().find(|d| d.tick == tick)
    }

    pub fn chain_length(&self) -> usize {
        self.chain.length()
    }

    pub fn verify_chain(&self) -> bool {
        self.chain.verify().is_ok()
    }

    pub fn tick_count(&self) -> usize {
        self.dumps.len()
    }
}

impl Default for WorldVault {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_dump_seal() {
        let heap = Heap::new();
        let symbols = SymbolTable::new();
        let env = EnvStore::new();
        let dump = WorldDump::new(0, heap, symbols, env, None, "METATRON".to_string());
        assert!(dump.verify());
        assert!(!dump.seal.is_empty());
    }

    #[test]
    fn test_vault_chain() {
        let mut vault = WorldVault::new();
        let dump = WorldDump::new(
            0, Heap::new(), SymbolTable::new(), EnvStore::new(),
            None, "METATRON".to_string()
        );
        let seal = vault.seal_dump(dump).unwrap();
        assert!(!seal.is_empty());
        assert_eq!(vault.chain_length(), 1);
        assert!(vault.verify_chain());
    }
}
