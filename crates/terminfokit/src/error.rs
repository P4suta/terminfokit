//! Error types for the whole pipeline.
//!
//! Dependency-free and `no_std`-compatible: no variant carries a payload that
//! requires an allocator, so [`Error`] is usable even with the `alloc` feature
//! disabled (see ADR 0002).

use core::fmt;

/// Any failure produced by terminfokit's parsing, resolution, compilation, or
/// expansion stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Syntax error in terminfo source text at the given 1-based line.
    Syntax {
        /// 1-based line number in the source text.
        line: usize,
    },
    /// A `use=` reference names an entry that does not exist in the input set.
    UnresolvedUse,
    /// A `use=` chain forms a cycle (e.g. `a` uses `b` uses `a`).
    UseCycle,
    /// Compiled data is truncated, misaligned, or otherwise malformed.
    Corrupt,
    /// The compiled data starts with an unknown magic number (neither `0o432`
    /// nor `0o1036`).
    BadMagic(u16),
    /// A value cannot be represented in the requested output format (e.g. a
    /// numeric capability above `0x7fff` in the legacy 16-bit format).
    Unrepresentable,
    /// Parameterized string expansion failed (stack underflow, malformed `%`
    /// escape, type mismatch, ...).
    Expansion,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Syntax { line } => write!(f, "syntax error in terminfo source at line {line}"),
            Error::UnresolvedUse => write!(f, "use= reference to an unknown entry"),
            Error::UseCycle => write!(f, "cycle in use= inheritance chain"),
            Error::Corrupt => write!(f, "malformed compiled terminfo data"),
            Error::BadMagic(m) => write!(f, "unknown terminfo magic number {m:#o}"),
            Error::Unrepresentable => {
                write!(f, "value not representable in the requested output format")
            }
            Error::Expansion => write!(f, "parameterized string expansion failed"),
        }
    }
}

impl core::error::Error for Error {}

/// Convenience alias used across the crate.
pub type Result<T> = core::result::Result<T, Error>;
