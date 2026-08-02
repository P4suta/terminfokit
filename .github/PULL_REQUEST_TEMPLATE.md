<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

## Summary

Describe the result and reason.

Fixes #

## Pull request title

Use a Conventional Commits title (`fix: ...`, `feat: ...`, or `feat!: ...`).
The title becomes the squash merge commit that release-plz analyzes.

## Validation

- [ ] Added focused tests or explained why none are needed
- [ ] Ran `cargo fmt --all -- --check`
- [ ] Ran relevant tests and Clippy checks
- [ ] Ran the full ncurses oracle when parser, resolver, schema, or binary output changed
- [ ] Documented API or CLI changes
- [ ] Kept both README copies identical

## Compatibility

List public API, ncurses compatibility, platform, or accepted-difference
changes. Write "None" when there are none.
