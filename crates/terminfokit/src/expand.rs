//! Parameterized string expansion — the `tparm`/`tput` virtual machine.
//!
//! Terminfo string capabilities embed a small stack language: `%p1`..`%p9`
//! push parameters, `%d`/`%s`/`%c` format the top of stack, `%{n}` and `%'c'`
//! push constants, `%+ %- %* %/ %m` do arithmetic, `%= %> %< %A %O %!` do
//! comparison/logic, `%? %t %e %;` provide conditionals, `%i` increments the
//! first two parameters (1-based cursor addressing), and `%P`/`%g` access
//! static and dynamic variables. This module interprets that language.

use alloc::vec::Vec;

use crate::error::Result;

/// A parameter passed to [`expand`], corresponding to `tparm(3)` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Param {
    /// A numeric parameter (e.g. a row or column for `cup`).
    Number(i64),
    /// A string parameter (raw bytes; used by e.g. `pfkey`).
    Str(Vec<u8>),
}

/// Expand the parameterized capability string `cap`, substituting `params`.
///
/// Returns the raw escape-sequence bytes to send to the terminal. Fails with
/// [`crate::Error::Expansion`] on stack underflow, malformed `%` escapes, or
/// parameter type mismatches.
///
/// Example (once implemented): expanding `\E[%i%p1%d;%p2%dH` with
/// `[Number(3), Number(7)]` yields `\E[4;8H`.
pub fn expand(cap: &str, params: &[Param]) -> Result<Vec<u8>> {
    let _ = (cap, params);
    todo!("tparm parameter VM (M2, needed by tput)")
}
