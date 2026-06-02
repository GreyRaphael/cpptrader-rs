// ---------------------------------------------------------------------------
//  CppTrader Rust Port — Symbol
//  Mirrors: include/trader/matching/symbol.h
// ---------------------------------------------------------------------------

/// A financial instrument / ticker symbol.
#[derive(Debug, Clone, Copy)]
pub struct Symbol {
    /// Numeric identifier (matches `StockLocate` in ITCH).
    pub id: u32,
    /// Fixed-width 8-byte name (null-padded).
    pub name: [u8; 8],
}

impl Symbol {
    /// Create a new symbol.
    pub fn new(id: u32, name: &[u8; 8]) -> Self {
        Self { id, name: *name }
    }

    /// Return the name as a UTF-8 string (trimmed at the first NUL byte).
    pub fn name_str(&self) -> &str {
        let end = self.name.iter().position(|&b| b == 0).unwrap_or(8);
        // SAFETY: we only slice up to `end`; the caller is responsible for
        // providing valid UTF-8 in the name bytes (ASCII stock symbols are safe).
        std::str::from_utf8(&self.name[..end]).unwrap_or("")
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", self.name_str(), self.id)
    }
}
