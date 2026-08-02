<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# ADR 0001: Compare output with ncurses `tic`

## Status

Accepted (2026-08-01)

## Context

The compiled format has under-documented rules for padding, alignment,
string-table offsets, the extended `-x` section, and cancelled `-2` values.
Tests derived only from a written specification can repeat interpretation
errors.

## Decision

Compile the pinned ncurses `terminfo.src` with ncurses `tic` and terminfokit,
then compare every output byte. Keep the harness in
`terminfokit-conformance`.

## Consequences

- Treat each byte difference as a bug until triaged. Allowlist deliberate
  differences with a reason.
- The harness requires ncurses `tic`, `std`, and external build tools, so it
  remains outside the core crate.
