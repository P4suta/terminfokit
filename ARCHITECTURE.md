# Architecture

## Pipeline

terminfokit is organized around one bidirectional pipeline. Every stage is a plain
data transformation with no I/O, so the core stays `no_std`.

```text
              parse_source              resolve_use_chains
 source text ────────────► [SourceEntry] ────────────────► [ResolvedEntry]
 (terminfo.src)  source.rs                 resolve.rs             │
                                                                  │ lower
                                                                  ▼
   compiled bytes ◄───────────────────────────────────────── Database
   (magic 0o432 /      Database::write            compiled.rs
    magic 0o1036)  ────────────────────►
                   ◄────────────────────
                       Database::parse

   Database strings ──► expand.rs (the tparm/tput parameter VM) ──► escape bytes
```

- `tic` = `parse_source` → `resolve_use_chains` → `Database::write`
- `infocmp` = `Database::parse` → (formatting back to source)
- `tput` = `Database::parse` → `expand`
- M0's round-trip gate = `Database::parse` ∘ `Database::write` is byte-identical

## Module map (crates/terminfokit)

| Module | Responsibility |
| --- | --- |
| `source` | Text parser for the terminfo source format (`SourceEntry`). |
| `resolve` | `use=` inheritance resolution: cycle detection, merge order, `cap@` cancellation (`ResolvedEntry`). |
| `compiled` | Reader *and writer* for the compiled binary format (`Database`, `Magic`). |
| `expand` | Parameterized string expansion VM (`%p1%d`, `%?%t%e%;`, …). |
| `caps` | Typed capability vocabulary: predefined bool/num/string tables + extended (`-x`) names. |
| `error` | Dependency-free, `no_std` error type. |

## `no_std` core / `std` shell boundary

- `crates/terminfokit`: `#![no_std]` + `alloc`, zero dependencies,
  `#![forbid(unsafe_code)]`. Features: `default = ["std"]`, `std = ["alloc"]`,
  `alloc = []`. Bytes in, bytes out — no filesystem, no environment access.
- `crates/terminfokit-cli`: `std` only. Owns argv parsing, file I/O, `TERMINFO`
  path lookup, and exit codes for the `tic` / `infocmp` / `tput` binaries.
- `crates/terminfokit-conformance`: `std` only. Drives the system `tic` as an
  oracle and byte-compares its output against ours (see ADR 0001).
