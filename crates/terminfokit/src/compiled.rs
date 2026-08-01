//! Reader *and writer* for the compiled terminfo binary format.
//!
//! Reading this format is well covered in the Rust ecosystem; **writing it is
//! not covered anywhere** — that write path is the reason this crate exists.
//!
//! Format sketch (see `term(5)`): a little-endian header with a magic number,
//! then the names section, a boolean section, a padding byte for alignment
//! (if needed), a numeric section, a string-offset section, and a string
//! table. An optional extended storage section (`tic -x`) follows with its
//! own counts, values, and a name table for user-defined capabilities.
//! Absent values are stored as `-1`, cancelled values as `-2`.

use alloc::string::String;
use alloc::vec::Vec;

use crate::caps::Value;
use crate::error::Result;

/// The magic number selecting the on-disk flavor of a compiled entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Magic {
    /// Legacy format, magic `0o432` (`0x011a`): numeric capabilities are
    /// signed 16-bit. Cannot represent numbers above `0x7fff`
    /// ([`crate::Error::Unrepresentable`]).
    Legacy,
    /// Extended-number format, magic `0o1036` (`0x021e`), introduced by
    /// ncurses 6.1: numeric capabilities are signed 32-bit.
    ExtendedNumbers,
}

impl Magic {
    /// The on-disk (little-endian) magic value: `0o432` for [`Magic::Legacy`],
    /// `0o1036` for [`Magic::ExtendedNumbers`].
    pub const fn value(self) -> u16 {
        match self {
            Magic::Legacy => 0o432,
            Magic::ExtendedNumbers => 0o1036,
        }
    }
}

/// An in-memory compiled terminfo entry: the three predefined sections plus
/// the extended (`-x`) section.
///
/// This is the hub of the pipeline: `tic` lowers a
/// [`crate::resolve::ResolvedEntry`] into a `Database` and calls [`write`];
/// `infocmp` and `tput` call [`parse`] and walk the sections.
///
/// [`write`]: Database::write
/// [`parse`]: Database::parse
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Database {
    /// The names section: `alias|alias|long description`.
    pub names: String,
    /// Predefined boolean capabilities, indexed by [`crate::caps::BooleanCap`]
    /// discriminant. Trailing absent entries may be omitted.
    pub booleans: Vec<bool>,
    /// Predefined numeric capabilities, indexed by [`crate::caps::NumericCap`]
    /// discriminant. `None` is stored as `-1` (absent); cancelled values are
    /// represented via [`Value::Cancelled`] in the extended model and `-2` on
    /// disk.
    pub numbers: Vec<Option<i32>>,
    /// Predefined string capabilities, indexed by [`crate::caps::StringCap`]
    /// discriminant. On disk these are offsets into the string table; `None`
    /// is stored as `-1`.
    pub strings: Vec<Option<Vec<u8>>>,
    /// Extended (`tic -x`) capabilities: user-defined names with their
    /// values, in the order required by the extended storage format.
    pub extended: Vec<(String, Value)>,
}

impl Database {
    /// Parse a compiled terminfo entry from raw bytes.
    ///
    /// Accepts both [`Magic::Legacy`] and [`Magic::ExtendedNumbers`] inputs
    /// and the optional extended section. Fails with
    /// [`crate::Error::BadMagic`] or [`crate::Error::Corrupt`].
    pub fn parse(data: &[u8]) -> Result<Database> {
        let _ = data;
        todo!("compiled-format reader (M0)")
    }

    /// Serialize this entry to the compiled on-disk format using `magic`.
    ///
    /// The M0 gate is that `parse` ∘ `write` round-trips byte-identically and
    /// that output matches ncurses `tic` bit for bit (ADR 0001). Writing a
    /// number above `0x7fff` with [`Magic::Legacy`] fails with
    /// [`crate::Error::Unrepresentable`].
    pub fn write(&self, magic: Magic) -> Result<Vec<u8>> {
        let _ = magic;
        todo!("compiled-format writer (M0) — the gap this crate exists to fill")
    }
}
