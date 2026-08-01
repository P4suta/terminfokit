//! Resolution of `use=` inheritance chains.
//!
//! **This module is the project's key differentiator.** The only known prior
//! Rust `tic` implementation (`ncurses-tools` in
//! `infinityabundance/ncurses-native`) does not resolve `use=` at all: it can
//! only compile pre-resolved sources, so it cannot process ncurses'
//! `terminfo.src` master file, which relies heavily on `use=` chains.
//! terminfokit treats `use=` resolution as a hard M1 requirement (ADR 0003).
//!
//! Resolution semantics:
//!
//! * every `use=` reference is followed transitively;
//! * cycles are detected and rejected ([`crate::Error::UseCycle`]);
//! * merging is last-wins: later definitions override earlier inherited ones;
//! * `cap@` cancels an inherited capability so it does not reappear from a
//!   deeper `use=` link.

use alloc::string::String;
use alloc::vec::Vec;

use crate::caps::Value;
use crate::error::Result;
use crate::source::SourceEntry;

/// A fully resolved entry: all `use=` chains flattened, all cancellations
/// applied. This is the input to lowering into [`crate::compiled::Database`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    /// The `|`-separated name field, unchanged from the source entry.
    pub names: Vec<String>,
    /// The merged capability set, keyed by short capability name. Contains no
    /// `use=` references and no unapplied cancellations.
    pub values: Vec<(String, Value)>,
}

/// Resolve every `use=` chain in `entries`.
///
/// All entries referenced by `use=` must be present in `entries` itself
/// (as is the case when compiling a self-contained file like `terminfo.src`);
/// a dangling reference yields [`crate::Error::UnresolvedUse`], a cyclic one
/// [`crate::Error::UseCycle`].
pub fn resolve_use_chains(entries: Vec<SourceEntry>) -> Result<Vec<ResolvedEntry>> {
    let _ = entries;
    todo!("use= chain resolution (M1) — cycle detection, last-wins merge, cap@ cancellation")
}
