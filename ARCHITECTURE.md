# Architecture

The public hub is a logical `Entry` with private storage. Fixed boolean,
numeric, and string capabilities use generated typed identifiers; every value
is `Absent`, `Cancelled`, or `Value(T)`. Terminal strings are raw bytes.

```text
terminfo source ── parse ──► unresolved SourceEntry graph
                                  │
                         Compiler + EntryProvider
                                  │
                                  ▼
termcap source ── convert ──────► Entry ── binary::encode ──► compiled bytes
                                  │  ▲
                                  │  └──── binary::decode
                                  ├──────► SourceFormatter / Entry::diff
                                  ├──────► Expander / padding events
                                  └──────► termcap writer
```

The library always has `alloc` available. With default features disabled every
transformation above remains available without runtime dependencies. The `std`
feature adds `DirectoryDatabase`, `SearchPath`, environment lookup, portable
transport, and atomic installation through `atomic_write_file`. CLI
dependencies remain confined to `terminfokit-cli`.

Capability metadata is declared once in `crates/terminfokit/capabilities.tsv`.
The dependency-free build script generates the three private-index typed
newtypes, associated constants, metadata, and lookup tables, keeping binary
indices, short/long names, termcap codes, parameter signatures, and capability
versions aligned.

All workspace crates forbid publishing until the offline tests, pinned ncurses
full-oracle suite, target OS matrix, documentation, and package audit pass.
