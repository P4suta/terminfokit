//! `infocmp` — decompile and compare terminfo entries.
//!
//! Planned argument surface (compatible subset of ncurses `infocmp(1)`):
//!
//! ```text
//! infocmp [-1] [-x] [-C] [-I] [-A dir] [-B dir] [term ...]
//!
//!   -1      print one capability per line
//!   -x      include extended (user-defined) capabilities
//!   -C      emit termcap format instead of terminfo source
//!   -I      emit terminfo source (default)
//!   term    entries to decompile (default: $TERM); with two terms, print
//!           a capability-by-capability comparison
//! ```
//!
//! Pipeline: locate the compiled entry under `$TERMINFO` search paths →
//! `terminfokit::compiled::Database::parse` → format back to source (or
//! termcap) via the `terminfokit::caps` tables.

fn main() {
    todo!("infocmp CLI (M2)")
}
