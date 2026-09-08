//! `SymbolId::derive` -- blake3 hash of `(path, name, arity)`, replacing `graph.rs`'s uuid path.

use crate::types::SymbolId;

impl SymbolId {
    /// `blake3(path || "\0" || name || "\0" || arity_as_decimal)`, hex-encoded. Deterministic:
    /// identical `(path, name, arity)` always yields the identical id.
    pub fn derive(path: &str, name: &str, arity: u64) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(path.as_bytes());
        hasher.update(&[0]);
        hasher.update(name.as_bytes());
        hasher.update(&[0]);
        hasher.update(arity.to_string().as_bytes());
        SymbolId(hasher.finalize().to_hex().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_id_is_deterministic_across_repeated_derivation() {
        let a = SymbolId::derive("f.rs", "foo", 2);
        let b = SymbolId::derive("f.rs", "foo", 2);
        assert_eq!(a, b);
        let c = SymbolId::derive("f.rs", "foo", 3);
        assert_ne!(a, c);
    }
}
