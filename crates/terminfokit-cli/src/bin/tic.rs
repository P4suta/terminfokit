//! `tic` — the terminfo entry-description compiler.
//!
//! Planned argument surface (compatible subset of ncurses `tic(1)`):
//!
//! ```text
//! tic [-x] [-e names] [-o dir] [-1] [-C] [-c] [-v[n]] [file | -]
//!
//!   -x        include extended (user-defined) capabilities
//!   -e names  only compile the entries named (comma-separated), plus their
//!             use= dependencies — the Alacritty invocation shape
//!             (`tic -xe alacritty,alacritty-direct`)
//!   -o dir    write the compiled database under dir instead of $TERMINFO
//!   -c        check only; report errors without writing
//!   -         read source from stdin — the yantra/ori-term invocation shape
//!             (`tic -x -`, `tic -x -o tempdir`)
//! ```
//!
//! Pipeline: read source → `terminfokit::source::parse_source` →
//! `terminfokit::resolve::resolve_use_chains` → lower each entry →
//! `terminfokit::compiled::Database::write` → place under `dir/<first-char>/`
//! (or `dir/<hex>/` on platforms with case-insensitive filesystems).

fn main() {
    todo!("tic CLI (M1)")
}
