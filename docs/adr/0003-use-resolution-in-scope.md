# ADR 0003: `use=` inheritance resolution is in scope (M1, non-negotiable)

## Status

Accepted (2026-08-01)

## Context

The only prior Rust `tic` attempt (`ncurses-tools` in
`infinityabundance/ncurses-native`) does not resolve `use=` inheritance: it can
only compile pre-resolved sources. That excludes the single most important
input in existence — ncurses' `terminfo.src` master file, which relies heavily
on `use=` chains. A `tic` that cannot compile `terminfo.src` is a demo, not a
replacement.

## Decision

`use=` resolution (`resolve::resolve_use_chains`) ships in M1 together with the
source parser: cycle detection, a well-defined merge order in which later
definitions override earlier inherited ones, and `cap@` cancellation of
inherited capabilities. It is the project's key differentiator and a release
blocker for M1.

## Consequences

- The M1 conformance target is the real `terminfo.src`, not curated
  pre-resolved fixtures.
- The pipeline gains an explicit intermediate type (`ResolvedEntry`) so that
  resolution is testable in isolation from parsing and encoding.
