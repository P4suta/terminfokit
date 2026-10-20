---
date: 2026-10-21
---

<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Verification contracts

## Status

Accepted.

## Context

The format core, native database access, ncurses comparison, and release generator have different inputs.
Running every check for an operating guide adds no evidence for those boundaries.
Repeating checks after merge and on a schedule also repeats evidence for the same source.

## Decision

Select verification from [affected paths](../../.github/filters/ci.yml).
Keep workflow lint, license notices, spelling, matching README copies, and a required status on every change.
Check handwritten workflows with actionlint and generated release output with the existing generator check.
Keep stable native tests on Linux, Windows, and macOS, and test Rust 1.88 once on Linux.
Keep portable library targets, dependency checks, source package checks, and fuzz target compilation for code changes.

Keep the complete [ncurses comparison](0001-oracle-differential-testing.md) for changes to its library implementation, capability data, dependencies, harness, and pinned inputs.
Select the release generator check for its source configuration and package metadata.
The required status rejects failed selection, failed selected jobs, and unexpected skipped jobs.
A manual CI request selects every lane.

Keep fuzz targets available for local investigations.
Remove the automatic fuzz campaign and duplicate main verification.
Keep BSD and repository security workflows available manually without schedules.

## Consequences

Operating guides need only the fast checks.
Compiled crate documentation still selects its native tests.
Library compatibility and supported host coverage retain distinct evidence.
The workflow selector and hosted event semantics remain external dependencies, so configuration changes need workflow validation and hosted checks.
Unconditional full builds waste work, while removing native tests or reference comparison would discard useful boundary coverage.
