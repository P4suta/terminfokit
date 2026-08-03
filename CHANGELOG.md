<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

This file lists notable changes. Versions follow Semantic Versioning.

## Unreleased


## [0.1.0](https://github.com/P4suta/terminfokit/releases/tag/v0.1.0) - 2026-08-03

### Added

- complete Rust terminfo toolkit
- scaffold the terminfokit workspace

### Other

- Harden repository governance and CI ([#6](https://github.com/P4suta/terminfokit/pull/6))
- document repository governance ([#3](https://github.com/P4suta/terminfokit/pull/3))
- Initial release of the `terminfokit` library and six command-line binaries.
- Added source, compiled-format, database, expansion, and termcap support.
- Added reproducible release archives, SBOMs, checksums, embedded dependency
  metadata, and GitHub artifact attestations.
- Added a CI check that the `dist`-generated release workflow still matches
  `[workspace.metadata.dist]`, and packaged the CLI alongside the library so
  the order-dependent publish path is exercised before a release.
