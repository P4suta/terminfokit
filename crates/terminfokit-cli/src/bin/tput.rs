//! `tput` — query the terminfo database and emit capability values.
//!
//! Planned argument surface (compatible subset of ncurses `tput(1)`):
//!
//! ```text
//! tput [-T term] capname [param ...]
//!
//!   -T term  use term instead of $TERM
//!   capname  capability to query: booleans set the exit code, numerics
//!            print the value, strings are parameter-expanded and written
//!            to stdout (e.g. `tput cup 3 7`)
//! ```
//!
//! Pipeline: locate the compiled entry → `terminfokit::compiled::Database::parse`
//! → look up `capname` via `terminfokit::caps` → for strings, run
//! `terminfokit::expand::expand` with the given parameters.

fn main() {
    todo!("tput CLI (M2)")
}
