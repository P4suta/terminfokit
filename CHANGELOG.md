<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

This file lists notable changes. Versions follow Semantic Versioning.

## Unreleased

## 0.1.0 - 2026-08-03

Initial release of the `terminfokit` library and six command-line binaries.

- Added source, compiled-format, database, expansion, and termcap support.
- The command-line binaries are `terminfokit`, `tik-tic`, `tik-infocmp`,
  `tik-tput`, `tik-captoinfo`, and `tik-infotocap`. The ncurses names are
  prefixed on purpose: `cargo install` and the shell installer both write into
  a directory that rustup places ahead of the system path, so binaries named
  `tput` or `tic` would replace the ncurses tools for every shell script,
  prompt, and curses program on the machine. Target auto-discovery is disabled
  so the unprefixed names cannot reappear.
- Every error type formats its own message. `BuildError`, `CompileError`,
  `DecodeError`, and `ExpandError` previously fell back to `Debug` output, so
  callers saw `invalid terminfo entry: InvalidNumber(-1)` where they now see
  `-1 is not a valid terminfo numeric value`. `Display` text is effectively
  part of the API, so this is fixed before the first release rather than after.
- `terminfokit doctor` reports an empty search path explicitly. Windows has no
  system terminfo database and no conventional location for one, so lookups
  fail until `TERMINFO` or `TERMINFO_DIRS` is set; the README documents this
  and the diagnostic now names the fix instead of reporting a bare `NotFound`.
- Added reproducible release archives, SBOMs, checksums, embedded dependency
  metadata, and GitHub artifact attestations.
- Added a CI check that the `dist`-generated release workflow still matches
  `[workspace.metadata.dist]`, and packaged the CLI alongside the library so
  the order-dependent publish path is exercised before a release.

### Not implemented

The 0.1 series covers the System V capability vocabulary and directory-backed
databases. The ncurses hashed-database file format is not implemented, and
`tic -R`, `infocmp -R`, `infocmp -e`, `infocmp -E`, and `infocmp -i` are
unsupported; each reports a diagnostic naming the supported alternative.
