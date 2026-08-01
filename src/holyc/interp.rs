// HolyC minimal interpreter — subset sufficient for LOC triad execution.
//
// Supports:
//   Print("msg")               → emit to WORM log
//   I64 arithmetic             → +, -, *, / on integer literals
//   FreqAnchor(hz)             → golden-ratio timing gate (1618 Hz canonical)
//   Assign: name=expr          → local variable store (single-pass, no heap)
//
// What this IS NOT: a full TempleOS runtime. It is the verifiable, borrow-safe
// subset that LOC can execute inside the triad cycle with compile-time safety.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH, Duration};

#[derive(Debug, Clone, PartialEq)]
pub enum HolyCValue {
    Int(i64),
    Str(String),
    Void,
}

impl std::fmt::Display for HolyCValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HolyCValue::Int(n) => write!(f, "{}", n),
            HolyCValue::Str(s) => write!(f, "{}", s),
            HolyCValue::Void   => write!(f, "Void"),
        }
    }
}

#[derive(Debug)]
pub struct HolyCResult {
    pub value:   HolyCValue,
    pub log:     Vec<String>,
    pub seal:    String,
    pub freq_hz: Option<u64>,
}

pub struct HolyCInterp {
    vars: HashMap<String, HolyCValue>,
    log:  Vec<String>,
}

impl HolyCInterp {
    pub fn new() -> Self {
        Self { vars: HashMap::new(), log: Vec::new() }
    }

    pub fn exec(&mut self, src: &str) -> HolyCResult {
        let src = src.trim();
        let value = self.eval_stmt(src);
        let seal  = self.seal_log();
        HolyCResult {
            freq_hz: self.vars.get("__freq_hz").and_then(|v| {
                if let HolyCValue::Int(n) = v { Some(*n as u64) } else { None }
            }),
            value,
            log: self.log.clone(),
            seal,
        }
    }

    /// Run multiple statements line by line. Returns the value of the last non-empty line.
    pub fn exec_lines(&mut self, src: &str) -> HolyCResult {
        let mut last = HolyCValue::Void;
        for line in src.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") { continue; }
            last = self.eval_stmt(line);
        }
        let seal = self.seal_log();
        HolyCResult {
            freq_hz: self.vars.get("__freq_hz").and_then(|v| {
                if let HolyCValue::Int(n) = v { Some(*n as u64) } else { None }
            }),
            value: last,
            log: self.log.clone(),
            seal,
        }
    }

    /// Inject a pre-computed integer signal as a named variable before exec.
    pub fn inject_var(&mut self, name: &str, value: i64) {
        self.vars.insert(name.to_string(), HolyCValue::Int(value));
    }

    fn eval_stmt(&mut self, s: &str) -> HolyCValue {
        let s = s.trim();

        // Print("...")
        if let Some(inner) = strip_call(s, "Print") {
            let val = self.eval_expr(inner);
            self.log.push(format!("[HolyC::Print] {}", val));
            return HolyCValue::Void;
        }

        // FreqAnchor(hz)
        if let Some(inner) = strip_call(s, "FreqAnchor") {
            let hz = match self.eval_expr(inner) {
                HolyCValue::Int(n) => n as u64,
                _ => 1618,
            };
            let now_ns = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .subsec_nanos() as u64;
            let aligned = now_ns % (1_000_000_000 / hz.max(1));
            self.log.push(format!("[HolyC::FreqAnchor] hz={} drift_ns={}", hz, aligned));
            self.vars.insert("__freq_hz".to_string(), HolyCValue::Int(hz as i64));
            return HolyCValue::Int(aligned as i64);
        }

        // JitCompile("expr") — compile + cache in vars as __jit_<hash>
        if let Some(inner) = strip_call(s, "JitCompile") {
            let result = self.eval_expr(inner);
            let key = format!("__jit_{}", short_hash(inner));
            self.log.push(format!("[HolyC::JitCompile] cached {} = {}", key, result));
            self.vars.insert(key.clone(), result.clone());
            return result;
        }

        // Assign: name=expr
        if let Some((lhs, rhs)) = split_assign(s) {
            let val = self.eval_expr(rhs);
            self.vars.insert(lhs.to_string(), val.clone());
            self.log.push(format!("[HolyC::Assign] {} = {}", lhs, val));
            return val;
        }

        // Bare expression
        self.eval_expr(s)
    }

    fn eval_expr(&self, s: &str) -> HolyCValue {
        let s = s.trim();

        // String literal
        if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
            return HolyCValue::Str(s[1..s.len()-1].to_string());
        }

        // Variable lookup
        if let Some(val) = self.vars.get(s) {
            return val.clone();
        }

        // Integer literal
        if let Ok(n) = s.parse::<i64>() {
            return HolyCValue::Int(n);
        }

        // Binary arithmetic: a op b
        if let Some((lhs, op, rhs)) = find_binop(s) {
            let l = self.to_i64(self.eval_expr(lhs));
            let r = self.to_i64(self.eval_expr(rhs));
            let result = match op {
                "+" => l + r,
                "-" => l - r,
                "*" => l * r,
                "/" => if r != 0 { l / r } else { 0 },
                "%" => if r != 0 { l % r } else { 0 },
                _   => 0,
            };
            return HolyCValue::Int(result);
        }

        HolyCValue::Str(format!("[unresolved: {}]", s))
    }

    fn to_i64(&self, v: HolyCValue) -> i64 {
        match v {
            HolyCValue::Int(n) => n,
            HolyCValue::Str(s) => s.parse().unwrap_or(0),
            HolyCValue::Void   => 0,
        }
    }

    fn seal_log(&self) -> String {
        use sha2::{Sha256, Digest};
        let mut h = Sha256::new();
        for line in &self.log { h.update(line.as_bytes()); h.update(b"\n"); }
        format!("{:x}", h.finalize())[..16].to_string()
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn strip_call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}(", name);
    if s.starts_with(&prefix) && s.ends_with(')') {
        Some(&s[prefix.len()..s.len()-1])
    } else {
        None
    }
}

fn split_assign(s: &str) -> Option<(&str, &str)> {
    if let Some(pos) = s.find('=') {
        let lhs = s[..pos].trim();
        let rhs = s[pos+1..].trim();
        // Ensure lhs is a valid identifier (no spaces, no operators)
        if lhs.chars().all(|c| c.is_alphanumeric() || c == '_') && !lhs.is_empty() {
            return Some((lhs, rhs));
        }
    }
    None
}

fn find_binop(s: &str) -> Option<(&str, &str, &str)> {
    for op in &["+", "-", "*", "/", "%"] {
        if let Some(pos) = s.rfind(op) {
            if pos > 0 && pos < s.len() - 1 {
                return Some((&s[..pos].trim_end(), op, &s[pos+1..].trim_start()));
            }
        }
    }
    None
}

fn short_hash(s: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    format!("{:x}", h.finalize())[..8].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print() {
        let mut i = HolyCInterp::new();
        let r = i.exec(r#"Print("hello sovereign")"#);
        assert!(r.log[0].contains("hello sovereign"));
    }

    #[test]
    fn test_arithmetic() {
        let mut i = HolyCInterp::new();
        let r = i.exec("1618 + 618");
        assert_eq!(r.value, HolyCValue::Int(2236));
    }

    #[test]
    fn test_assign_and_read() {
        let mut i = HolyCInterp::new();
        i.exec("x=42");
        let r = i.exec("x + 8");
        assert_eq!(r.value, HolyCValue::Int(50));
    }

    #[test]
    fn test_freq_anchor() {
        let mut i = HolyCInterp::new();
        let r = i.exec("FreqAnchor(1618)");
        assert_eq!(i.vars.get("__freq_hz"), Some(&HolyCValue::Int(1618)));
        assert!(r.freq_hz == Some(1618));
    }
}
