<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Architecture

`Entry` is the public logical model. Generated identifiers address fixed
boolean, numeric, and string capabilities. Values are `Absent`, `Cancelled`,
or `Value(T)`. Terminal strings remain raw bytes.

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

The library always uses `alloc`. Disabling default features preserves all data
transformations without runtime dependencies. The `std` feature adds
`DirectoryDatabase`, `SearchPath`, environment lookup, portable transport, and
atomic installation through `atomic_write_file`. CLI dependencies remain in
`terminfokit-cli`.

`crates/terminfokit/capabilities.tsv` defines capability metadata. The
dependency-free build script generates typed identifiers, constants, metadata,
and lookup tables from it.

The private conformance crate compares output with pinned ncurses. CI also runs
offline tests, target and OS checks, documentation checks, and package audits.
