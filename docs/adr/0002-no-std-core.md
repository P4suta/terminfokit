<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# ADR 0002: Use `alloc` with optional `std`

## Status

Accepted (2026-08-01)

## Context

Parsing, inheritance resolution, binary encoding and decoding, and parameter
expansion require allocation but no OS services.

## Decision

`crates/terminfokit` always links `alloc` and uses
`#![cfg_attr(not(feature = "std"), no_std)]`. It defines
`default = ["std"]` and `std = ["dep:atomic-write-file"]`, with no separate
`alloc` feature. The optional `std` feature provides filesystem databases,
environment lookup, transport, and atomic installation. CLI I/O remains in
`terminfokit-cli`. Workspace Rust code forbids unsafe code.

## Consequences

- The transformation API works without runtime dependencies.
- Error types may use `alloc` collections but implement `core::error::Error`.
- Filesystem APIs require `std`; pure transformations do not.
- Atomic installation is the library's only optional runtime dependency.
