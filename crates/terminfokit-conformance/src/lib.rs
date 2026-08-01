//! Conformance harness for terminfokit: differential testing against ncurses'
//! `tic` as a bit-exact oracle (ADR 0001).
//!
//! # Plan
//!
//! The oracle method, to be implemented alongside M0/M1:
//!
//! 1. Obtain ncurses' `terminfo.src` master file (~2,900 entries).
//! 2. Compile every entry twice: once with the system `tic` (the oracle),
//!    once with terminfokit's `tic`, into separate output trees
//!    (`tic -x -o <dir> <file>` on both sides).
//! 3. Byte-compare each compiled entry. Any difference is a failure; any
//!    deliberate, triaged difference must be allowlisted with a written
//!    rationale.
//! 4. Repeat for both magics where applicable: legacy `0o432` and
//!    extended-number `0o1036`.
//!
//! Feasibility of the bit-exact bar is proven: `ncurses-tools` (in
//! `infinityabundance/ncurses-native`) matched ncurses 6.4's `tic`
//! byte-for-byte across all 2,869 terminals. Unlike that implementation,
//! terminfokit resolves `use=` chains itself (ADR 0003), so this harness
//! feeds `tic` the *unresolved* `terminfo.src` directly rather than
//! pre-resolved fixtures.
//!
//! This crate is intentionally an empty shell for now: it exists to reserve
//! the workspace slot and document the method. It is never published
//! (`publish = false`).

#![forbid(unsafe_code)]
