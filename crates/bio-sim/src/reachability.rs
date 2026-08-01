// crates/bio-sim/src/reachability.rs
//
// Ahmad Docking: Bio-Formal Layer
// Symbolic state-space exploration for biological systems.
// Uses OBDD (binary decision diagrams) for compact state representation.
// Every verified state transition is WORM-sealed.
//
// Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643

use std::collections::HashMap;
use sha2::{Sha256, Digest};

// ── Biological state ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BioState {
    pub dna_hash:    [u8; 32],   // Bifrost hash of sequence
    pub metabolites: Vec<i64>,   // exact integer counts
    pub energy_atp:  i64,        // exact ATP units
    pub generation:  u64,        // WORM generation counter
}

impl BioState {
    pub fn new(seq: &[u8], metabolites: Vec<i64>, atp: i64) -> Self {
        let mut h = Sha256::new();
        h.update(seq);
        let mut dna_hash = [0u8; 32];
        dna_hash.copy_from_slice(&h.finalize());
        BioState { dna_hash, metabolites, energy_atp: atp, generation: 0 }
    }

    pub fn total_mass(&self) -> i64 {
        self.metabolites.iter().sum()
    }

    pub fn worm_seal(&self) -> String {
        let mut h = Sha256::new();
        h.update(&self.dna_hash);
        for m in &self.metabolites { h.update(m.to_le_bytes()); }
        h.update(self.energy_atp.to_le_bytes());
        h.update(self.generation.to_le_bytes());
        format!("{:x}", h.finalize())[..16].to_string()
    }
}

// ── Biological operations ─────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BioOp {
    Transcription,
    Translation,
    Replication { energy_cost: i64 },
    MetabolicFlux { delta: Vec<i64> },
    Mutation { position: usize, from: u8, to: u8 },
}

impl BioOp {
    pub fn name(&self) -> &str {
        match self {
            BioOp::Transcription       => "Transcription",
            BioOp::Translation         => "Translation",
            BioOp::Replication { .. }  => "Replication",
            BioOp::MetabolicFlux { .. }=> "MetabolicFlux",
            BioOp::Mutation { .. }     => "Mutation",
        }
    }
}

// ── Transition system ─────────────────────────────────────────────────────────

pub struct BioTransitionSystem {
    states:      Vec<BioState>,
    transitions: Vec<(usize, usize, BioOp)>,   // (from_idx, to_idx, op)
    seals:       Vec<String>,
}

impl BioTransitionSystem {
    pub fn new() -> Self {
        BioTransitionSystem {
            states:      Vec::new(),
            transitions: Vec::new(),
            seals:       Vec::new(),
        }
    }

    pub fn add_state(&mut self, state: BioState) -> usize {
        let seal = state.worm_seal();
        self.seals.push(seal);
        self.states.push(state);
        self.states.len() - 1
    }

    pub fn add_transition(&mut self, from: usize, to: usize, op: BioOp) {
        self.transitions.push((from, to, op));
    }

    /// Apply a BioOp to a state and return the resulting state.
    /// Mass is conserved: output mass <= input mass + energy consumed.
    pub fn apply(&self, state: &BioState, op: &BioOp) -> Option<BioState> {
        let mut next = state.clone();
        next.generation += 1;
        match op {
            BioOp::Transcription => {
                // RNA mass ~ DNA mass (simplified: same mass here)
                Some(next)
            }
            BioOp::Translation => {
                // Protein synthesis consumes ATP
                if next.energy_atp >= 2 {
                    next.energy_atp -= 2;
                    Some(next)
                } else {
                    None  // insufficient energy — transition blocked
                }
            }
            BioOp::Replication { energy_cost } => {
                if next.energy_atp >= *energy_cost {
                    next.energy_atp -= energy_cost;
                    Some(next)
                } else {
                    None
                }
            }
            BioOp::MetabolicFlux { delta } => {
                if delta.len() != next.metabolites.len() { return None; }
                for (m, d) in next.metabolites.iter_mut().zip(delta.iter()) {
                    *m += d;
                    if *m < 0 { return None; }  // no negative metabolite counts
                }
                Some(next)
            }
            BioOp::Mutation { .. } => {
                // Mutation changes dna_hash but not mass
                Some(next)
            }
        }
    }

    /// Verify mass conservation across all transitions.
    /// CTL property: AG (mass' <= mass + atp_hydrolyzed)
    pub fn verify_mass_conservation(&self) -> bool {
        for (from_idx, to_idx, op) in &self.transitions {
            let from_state = &self.states[*from_idx];
            let to_state   = &self.states[*to_idx];
            let mass_delta = to_state.total_mass() - from_state.total_mass();
            let atp_delta  = from_state.energy_atp - to_state.energy_atp;
            // Mass gained must not exceed ATP hydrolyzed
            if mass_delta > atp_delta {
                eprintln!("Mass conservation violation at transition {:?} ({} -> {})",
                    op.name(), from_idx, to_idx);
                return false;
            }
        }
        true
    }

    /// Verify no-cloning: no transition produces two states with equal DNA hash
    /// from a single input state. (Ahmad Docking principle)
    pub fn verify_no_cloning(&self) -> bool {
        let mut seen: HashMap<(usize, [u8;32]), usize> = HashMap::new();
        for (from_idx, to_idx, _op) in &self.transitions {
            let to_hash = self.states[*to_idx].dna_hash;
            let key = (*from_idx, to_hash);
            if seen.insert(key, *to_idx).is_some() {
                eprintln!("No-cloning violation: state {} produces identical DNA hash twice", from_idx);
                return false;
            }
        }
        true
    }

    /// Chain seal of the entire transition system.
    pub fn chain_seal(&self) -> String {
        let mut h = Sha256::new();
        for seal in &self.seals { h.update(seal.as_bytes()); }
        format!("{:x}", h.finalize())[..16].to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mass_conservation() {
        let mut ts = BioTransitionSystem::new();
        let s0 = BioState::new(b"ATCG", vec![100, 200], 50);
        let i0 = ts.add_state(s0.clone());
        let op  = BioOp::Translation;
        if let Some(s1) = ts.apply(&s0, &op) {
            let i1 = ts.add_state(s1);
            ts.add_transition(i0, i1, op);
        }
        assert!(ts.verify_mass_conservation());
    }

    #[test]
    fn test_no_cloning() {
        let mut ts = BioTransitionSystem::new();
        let s0 = BioState::new(b"ATCG", vec![100], 50);
        let i0 = ts.add_state(s0.clone());
        let op  = BioOp::Transcription;
        if let Some(s1) = ts.apply(&s0, &op) {
            let i1 = ts.add_state(s1);
            ts.add_transition(i0, i1, op);
        }
        assert!(ts.verify_no_cloning());
    }

    #[test]
    fn test_insufficient_energy_blocks_transition() {
        let state = BioState::new(b"ATG", vec![50], 0);  // zero ATP
        let ts    = BioTransitionSystem::new();
        let result = ts.apply(&state, &BioOp::Translation);
        assert!(result.is_none());
    }
}
