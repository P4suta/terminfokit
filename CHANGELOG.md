<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Changelog

This file lists notable changes. Versions follow Semantic Versioning.

## Unreleased

- Initial release of the `terminfokit` library and six command-line binaries.
- Added source, compiled-format, database, expansion, and termcap support.
- Added reproducible release archives, SBOMs, checksums, embedded dependency
  metadata, and GitHub artifact attestations.
- Added a CI check that the `dist`-generated release workflow still matches
  `[workspace.metadata.dist]`, and packaged the CLI alongside the library so
  the order-dependent publish path is exercised before a release.
