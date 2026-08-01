# ADR 0001: Bit-exact differential testing against ncurses `tic` as the oracle

## Status

Accepted (2026-08-01)

## Context

The compiled terminfo format has many under-documented corner cases (padding,
alignment, string-table offset ordering, the extended `-x` section, cancelled
`-2` values). A hand-written spec test suite would encode our own
misunderstandings. Prior art proves a better bar is reachable: `ncurses-tools`
(in `infinityabundance/ncurses-native`) achieved byte-identical output with
ncurses 6.4's `tic` across all 2,869 terminals of the terminfo database.

## Decision

The primary correctness gate is differential: compile every entry of ncurses'
`terminfo.src` (~2,900 entries) with both the system `tic` and terminfokit's
`tic`, then compare outputs byte for byte. The `terminfokit-conformance` crate
exists solely to host this harness.

## Consequences

- Any byte difference is a bug by definition until triaged; triaged, deliberate
  differences must be allowlisted with a written rationale.
- The harness needs a system ncurses `tic` and is therefore `std`-only and
  environment-dependent; it lives outside the core crate.
