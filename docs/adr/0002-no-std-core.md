# ADR 0002: always-`alloc` core with optional `std` services

## Status

Accepted (2026-08-01)

## Context

terminfo is consumed in unusual places: terminal emulators, SSH tooling that
ships entries to remote hosts, and build scripts. The core work — parsing text,
resolving inheritance, encoding/decoding a binary format, running the parameter
VM — is pure data transformation and needs no OS services.

## Decision

`crates/terminfokit` always links `alloc` and is
`#![cfg_attr(not(feature = "std"), no_std)]`. Its features are
`default = ["std"]` and `std = ["dep:atomic-write-file"]`; there is no
separately selectable `alloc` feature. The transformation modules have no
runtime dependencies. The optional `std` feature owns filesystem databases,
environment lookup, transport, and atomic installation. Argv and terminal I/O
remain in `terminfokit-cli`. All workspace Rust code forbids unsafe code.

## Consequences

- The API is bytes-in/bytes-out; embedding in emulators or build tools cannot
  drag in a dependency tree.
- Error types may use `alloc` collections but implement `core::error::Error`.
- Filesystem convenience APIs are available from the same crate only when
  `std` is enabled; no-std targets retain every pure transformation.
- Atomic installation is the sole optional runtime dependency of the library.
