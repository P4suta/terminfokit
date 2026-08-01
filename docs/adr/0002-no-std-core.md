# ADR 0002: `no_std` + `alloc` core with zero dependencies

## Status

Accepted (2026-08-01)

## Context

terminfo is consumed in unusual places: terminal emulators, SSH tooling that
ships entries to remote hosts, and build scripts. The core work — parsing text,
resolving inheritance, encoding/decoding a binary format, running the parameter
VM — is pure data transformation and needs no OS services.

## Decision

`crates/terminfokit` is `#![cfg_attr(not(feature = "std"), no_std)]` with
features `default = ["std"]`, `std = ["alloc"]`, `alloc = []`, has zero
dependencies, and is `#![forbid(unsafe_code)]`. All I/O (files, `TERMINFO`
search paths, argv) lives in `terminfokit-cli` and downstream crates.

## Consequences

- The API is bytes-in/bytes-out; embedding in emulators or build tools cannot
  drag in a dependency tree.
- Error types must not require `std::error::Error` or `String` payloads that
  assume an allocator beyond the `alloc` feature.
- Convenience I/O helpers, if ever wanted, belong behind `std` or in the CLI.
