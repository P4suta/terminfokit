<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# ADR 0003: Resolve `use=` inheritance

## Status

Accepted (2026-08-01)

## Context

ncurses `terminfo.src` relies on `use=` chains. Compiling it requires
inheritance resolution.

## Decision

Implement `use=` resolution with the source parser. Detect cycles, define merge
order, let later definitions override inherited values, and apply `cap@`
cancellation. Require this behavior for M1.

## Consequences

- M1 conformance uses ncurses `terminfo.src`, not pre-resolved fixtures.
- An explicit `ResolvedEntry` makes resolution testable without parsing or
  encoding.
