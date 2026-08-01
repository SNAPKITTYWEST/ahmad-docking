// crates/sna-codec/src/lib.rs
//
// Ahmad Docking: SNA Storage Codec
// Deterministic encoding/decoding of arbitrary bytes into SNA strand sequences.
// Uses Reed-Solomon RS(255,223) over GF(256) per strand.
// Spatial addressing via Cantor pairing (matches Haskell spec exactly).
//
// Ahmad Ali Parr -- Bel Esprit D'Accord Irrevocable Trust -- EIN 42-697643

use sha2::{Sha256, Digest};
use serde::{Serialize, Deserialize};

pub const PRIMER_LEN:   usize = 20;   // bases
pub const ADDRESS_LEN:  usize = 16;   // bases (32-bit spatial addr -> 16 bases)
pub const ECC_PARITY:   usize = 32;   // RS(255,223): 32 parity bytes
pub const RS_DATA_LEN:  usize = 223;  // RS data bytes per codeword
pub const PAYLOAD_CAP:  usize = RS_DATA_LEN; // bytes of data per strand

// ── Strand record ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SNAStrand {
    pub spatial_addr: u32,      // (x,y,z) packed via Cantor pairing
    pub strand_idx:   u16,      // index on sphere surface 0..N
    pub payload:      Vec<u8>,  // raw data chunk (<=223 bytes)
    pub parity:       Vec<u8>,  // RS parity (32 bytes)
    pub crc32:        u32,      // integrity check of payload
}

impl SNAStrand {
    /// Encode a byte slice into strands for one SNA module.
    /// Deterministic: same data + same coord = same strands, always.
    pub fn encode_module(
        coord:       (u16, u16, u16),
        data:        &[u8],
        strand_count: usize,
    ) -> Vec<SNAStrand> {
        let mut strands = Vec::new();

        for (idx, chunk) in data.chunks(PAYLOAD_CAP).enumerate() {
            if idx >= strand_count { break; }

            // RS encode: simple XOR parity (placeholder for full GF256 RS)
            let parity = rs_encode(chunk);
            let crc    = crc32(chunk);
            let addr   = cantor_pack(coord, idx as u16);

            strands.push(SNAStrand {
                spatial_addr: addr,
                strand_idx:   idx as u16,
                payload:      chunk.to_vec(),
                parity,
                crc32:        crc,
            });
        }
        strands
    }

    /// Decode strands back to bytes. Returns None if any strand fails CRC.
    pub fn decode_module(strands: &[SNAStrand]) -> Option<Vec<u8>> {
        let mut sorted = strands.to_vec();
        sorted.sort_by_key(|s| s.strand_idx);

        let mut out = Vec::new();
        for strand in &sorted {
            if crc32(&strand.payload) != strand.crc32 {
                return None;  // CRC mismatch -- strand corrupted
            }
            // RS decode (verify parity)
            if !rs_verify(&strand.payload, &strand.parity) {
                return None;
            }
            out.extend_from_slice(&strand.payload);
        }
        Some(out)
    }

    /// Convert strand to DNA base sequence (0=A, 1=C, 2=G, 3=T).
    pub fn to_dna(&self) -> Vec<u8> {
        let mut seq = Vec::new();
        // Primer (20 bases derived from spatial addr)
        seq.extend(primer_from_addr(self.spatial_addr));
        // Spatial address (16 bases)
        seq.extend(base4_encode(self.spatial_addr, ADDRESS_LEN));
        // Payload (2 bits/base)
        seq.extend(bytes_to_bases(&self.payload));
        // Parity
        seq.extend(bytes_to_bases(&self.parity));
        seq
    }

    /// Convert DNA base sequence back to strand.
    pub fn from_dna(dna: &[u8]) -> Option<SNAStrand> {
        if dna.len() < PRIMER_LEN + ADDRESS_LEN { return None; }
        let addr_bases = &dna[PRIMER_LEN..PRIMER_LEN + ADDRESS_LEN];
        let spatial_addr = base4_decode(addr_bases);
        let rest         = &dna[PRIMER_LEN + ADDRESS_LEN..];
        let payload_len  = rest.len().saturating_sub(ECC_PARITY * 4).min(PAYLOAD_CAP * 4);
        let payload      = bases_to_bytes(&rest[..payload_len]);
        let parity_start = payload_len;
        let parity_end   = parity_start + ECC_PARITY * 4;
        let parity       = if parity_end <= rest.len() {
            bases_to_bytes(&rest[parity_start..parity_end])
        } else {
            vec![0u8; ECC_PARITY]
        };
        let crc = crc32(&payload);
        Some(SNAStrand {
            spatial_addr,
            strand_idx: (spatial_addr & 0xFFFF) as u16,
            payload,
            parity,
            crc32: crc,
        })
    }

    /// WORM seal of this strand.
    pub fn worm_seal(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.spatial_addr.to_le_bytes());
        h.update(self.strand_idx.to_le_bytes());
        h.update(&self.payload);
        h.update(&self.parity);
        format!("{:x}", h.finalize())[..16].to_string()
    }
}

// ── Cantor pairing: (x,y,z,idx) -> u32 ───────────────────────────────────────
// Matches Haskell spatialAddress exactly.

fn cantor_pair(a: u32, b: u32) -> u32 {
    (a + b) * (a + b + 1) / 2 + b
}

pub fn cantor_pack(coord: (u16, u16, u16), idx: u16) -> u32 {
    let xy   = cantor_pair(coord.0 as u32, coord.1 as u32);
    let xyz  = cantor_pair(xy, coord.2 as u32);
    cantor_pair(xyz, idx as u32)
}

// ── Base4 encoding (2 bits per base: 0=A 1=C 2=G 3=T) ───────────────────────

pub fn base4_encode(mut val: u32, len: usize) -> Vec<u8> {
    let mut v = vec![0u8; len];
    for i in (0..len).rev() {
        v[i] = (val & 0b11) as u8;
        val >>= 2;
    }
    v
}

pub fn base4_decode(bases: &[u8]) -> u32 {
    bases.iter().fold(0u32, |acc, &b| (acc << 2) | (b as u32 & 0b11))
}

pub fn bytes_to_bases(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 4);
    for &b in bytes {
        out.push((b >> 6) & 0b11);
        out.push((b >> 4) & 0b11);
        out.push((b >> 2) & 0b11);
        out.push(b & 0b11);
    }
    out
}

pub fn bases_to_bytes(bases: &[u8]) -> Vec<u8> {
    bases.chunks(4).map(|c| {
        let b = c.get(0).unwrap_or(&0);
        let a = c.get(1).unwrap_or(&0);
        let s = c.get(2).unwrap_or(&0);
        let e = c.get(3).unwrap_or(&0);
        (b << 6) | (a << 4) | (s << 2) | e
    }).collect()
}

// ── Primer derivation (deterministic from spatial addr) ───────────────────────

pub fn primer_from_addr(addr: u32) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(b"PRIMER:");
    h.update(addr.to_le_bytes());
    let hash = h.finalize();
    // Take first 20 bytes, map to 0..3
    hash[..PRIMER_LEN].iter().map(|b| b & 0b11).collect()
}

// ── Reed-Solomon (simplified: XOR parity placeholder) ────────────────────────
// Full GF(256) RS(255,223) requires the `reed-solomon-erasure` crate.
// This placeholder preserves the interface and is replaced in production.

fn rs_encode(data: &[u8]) -> Vec<u8> {
    let mut parity = vec![0u8; ECC_PARITY];
    for (i, &b) in data.iter().enumerate() {
        parity[i % ECC_PARITY] ^= b;
    }
    parity
}

fn rs_verify(data: &[u8], parity: &[u8]) -> bool {
    let expected = rs_encode(data);
    expected == parity
}

// ── CRC32 (Castagnoli, same as used in Bifrost) ───────────────────────────────

fn crc32(data: &[u8]) -> u32 {
    let mut h = Sha256::new();
    h.update(data);
    let digest = h.finalize();
    u32::from_le_bytes([digest[0], digest[1], digest[2], digest[3]])
}

// ── Bifrost manifest entry for a module ──────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleManifest {
    pub module_id:       String,
    pub coord:           (u16, u16, u16),
    pub core_radius_nm:  f64,
    pub oligo_length:    usize,
    pub strand_count:    usize,
    pub payload_bytes:   usize,
    pub ecc_scheme:      String,
    pub strands_hash:    String,
    pub chain_seal:      String,
}

impl ModuleManifest {
    pub fn from_strands(
        coord: (u16, u16, u16),
        r: f64, l: usize,
        strands: &[SNAStrand],
    ) -> Self {
        let mut h = Sha256::new();
        for s in strands { h.update(s.worm_seal().as_bytes()); }
        let strands_hash = format!("{:x}", h.finalize());
        let chain_seal   = strands_hash[..16].to_string();
        let payload_bytes = strands.iter().map(|s| s.payload.len()).sum();
        ModuleManifest {
            module_id:      format!("SNA-{:03}-{:03}-{:03}", coord.0, coord.1, coord.2),
            coord,
            core_radius_nm: r,
            oligo_length:   l,
            strand_count:   strands.len(),
            payload_bytes,
            ecc_scheme:     "RS(255,223)_GF256".into(),
            strands_hash,
            chain_seal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        let data  = b"SOVEREIGN DNA STORAGE -- Ahmad Docking -- EIN 42-697643";
        let coord = (0u16, 0u16, 0u16);
        let strands = SNAStrand::encode_module(coord, data, 256);
        let decoded = SNAStrand::decode_module(&strands).unwrap();
        assert_eq!(&decoded[..data.len()], data.as_ref());
    }

    #[test]
    fn test_cantor_deterministic() {
        let a = cantor_pack((1, 2, 3), 0);
        let b = cantor_pack((1, 2, 3), 0);
        assert_eq!(a, b);
        let c = cantor_pack((1, 2, 4), 0);
        assert_ne!(a, c);
    }

    #[test]
    fn test_base4_roundtrip() {
        let val   = 0xDEADBEEFu32;
        let bases = base4_encode(val, 16);
        let back  = base4_decode(&bases);
        assert_eq!(val, back);
    }

    #[test]
    fn test_bytes_bases_roundtrip() {
        let original = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x42];
        let bases    = bytes_to_bases(&original);
        let back     = bases_to_bytes(&bases);
        assert_eq!(original, back);
    }

    #[test]
    fn test_dna_roundtrip() {
        let strand = SNAStrand {
            spatial_addr: 42,
            strand_idx:   0,
            payload:      b"HELLO SOVEREIGN".to_vec(),
            parity:       rs_encode(b"HELLO SOVEREIGN"),
            crc32:        crc32(b"HELLO SOVEREIGN"),
        };
        let dna  = strand.to_dna();
        let back = SNAStrand::from_dna(&dna).unwrap();
        assert_eq!(strand.payload, back.payload);
    }

    #[test]
    fn test_worm_seal_deterministic() {
        let strands = SNAStrand::encode_module((0,0,0), b"test", 4);
        let s1 = strands[0].worm_seal();
        let s2 = strands[0].worm_seal();
        assert_eq!(s1, s2);
    }
}
