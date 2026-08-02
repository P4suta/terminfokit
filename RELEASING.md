<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Release process

The first crates.io release is manual. Later releases use the generated dist
workflow and a protected tag.

## One-time repository setup

1. Create a GitHub environment named `release` and require maintainer approval.
2. Protect `v*` tags so only release maintainers can create them.
3. Enable immutable GitHub Releases.
4. Publish `terminfokit` and then `terminfokit-cli` once from a maintainer
   workstation. Do not reverse this order.
5. On crates.io, register this repository, the `release` environment, and
   `.github/workflows/publish-crates.yml` as a trusted publisher for both
   crates. Remove any long-lived CI publishing token.

## Each release

1. Update versions, exact internal dependency versions, and `CHANGELOG.md`.
2. Run the checks in `CONTRIBUTING.md`, `reuse lint`, package both public
   crates, and run `dist generate --check` plus `dist plan`.
3. Run the **Release dry run** workflow. Inspect all archives, six binaries,
   CycloneDX JSON SBOMs, `sha256.sum`, and auditable dependency metadata.
4. Merge through the required checks, then create and push the protected
   `v<version>` tag, for example `v0.1.0`.
5. Approve the `release` environment deployment and verify the GitHub Release,
   artifact attestations, and both crates.io versions.

Release only reviewed commits. Workflows skip existing crate versions.
Published crates and immutable release assets cannot be replaced.
