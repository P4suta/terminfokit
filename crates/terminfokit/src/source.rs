//! Parser for the terminfo *source* format — the text format consumed by
//! `tic(1)` and produced by `infocmp(1)`.
//!
//! A source file (such as ncurses' `terminfo.src` master file) is a sequence
//! of entries. Each entry starts with a `|`-separated name field and continues
//! with comma-separated capabilities: booleans (`am`), numerics (`cols#80`),
//! strings (`cup=\E[%i%p1%d;%p2%dH`), cancellations (`smso@`), and inheritance
//! references (`use=xterm`).
//!
//! The output of this module is deliberately *unresolved*: `use=` references
//! are preserved as-is and handed to [`crate::resolve`].

use alloc::string::String;
use alloc::vec::Vec;

use crate::error::Result;

/// A single entry exactly as written in a terminfo source file, before any
/// `use=` resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    /// The `|`-separated name field: one or more aliases, with the long
    /// human-readable description conventionally last
    /// (e.g. `xterm|xterm-debian|X11 terminal emulator`).
    pub names: Vec<String>,
    /// Capabilities in source order, including `use=` references and `cap@`
    /// cancellations. Order matters for resolution semantics.
    pub capabilities: Vec<Capability>,
}

/// One capability as written in source form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    /// A boolean capability, e.g. `am`.
    Boolean {
        /// Short capability name, e.g. `"am"`.
        name: String,
    },
    /// A numeric capability, e.g. `cols#80`. Source accepts decimal, octal
    /// (`#017`), and hexadecimal (`#0x10`) literals.
    Numeric {
        /// Short capability name, e.g. `"cols"`.
        name: String,
        /// Parsed value.
        value: i32,
    },
    /// A string capability, e.g. `cup=\E[%i%p1%d;%p2%dH`. The value is stored
    /// with source-level escapes (`\E`, `^X`, `\n`, `\0ctal`) already decoded
    /// to raw bytes.
    Str {
        /// Short capability name, e.g. `"cup"`.
        name: String,
        /// Decoded raw bytes of the capability value.
        value: Vec<u8>,
    },
    /// A cancellation, e.g. `smso@` — explicitly removes a capability,
    /// typically one that would otherwise be inherited via `use=`.
    Cancel {
        /// Short name of the cancelled capability.
        name: String,
    },
    /// An inheritance reference, e.g. `use=xterm-256color`.
    Use {
        /// Primary name of the referenced entry.
        name: String,
    },
}

/// Parse an entire terminfo source text into its entries.
///
/// Handles comment lines (`#`), continuation whitespace, and the escape
/// syntax of string values. Returns [`crate::Error::Syntax`] with the
/// offending line number on malformed input.
///
/// `use=` references are *not* resolved here — feed the result to
/// [`crate::resolve::resolve_use_chains`].
pub fn parse_source(source: &str) -> Result<Vec<SourceEntry>> {
    let _ = source;
    todo!("terminfo source parser (M1)")
}
