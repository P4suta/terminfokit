<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Release process

release-plz prepares lockstep versions and the shared changelog in a release
pull request, then creates one protected `v<version>` tag when that pull request
is merged. The tag starts cargo-dist, which builds the binaries and SBOMs,
creates attestations and the immutable GitHub Release, and publishes both crates
through crates.io trusted publishing.

release-plz never publishes crates or creates a GitHub Release. Do not give its
workflow a crates.io token or an OpenID Connect permission.

## Bootstrap release 0.1.0

The first crates.io publication cannot use trusted publishing. Before enabling
the release-plz workflow, release the already reviewed `0.1.0` commit manually:

1. From that exact commit, run all checks in [CONTRIBUTING.md](CONTRIBUTING.md),
   package both public crates, and inspect their contents.
2. Publish `terminfokit`, wait until it is available from the crates.io index,
   and then publish `terminfokit-cli`. Do not reverse this order.
3. Create and push `v0.1.0` on the same commit. Approve the `release`
   environment deployment and let cargo-dist complete the immutable GitHub
   Release.
4. Verify both crates, all release assets, CycloneDX JSON SBOMs, checksums,
   auditable dependency metadata, and artifact attestations.
5. Only after the manual publication succeeds, register trusted publishers for
   both crates using this repository, the `release` environment, and
   `.github/workflows/publish-crates.yml`.

## One-time repository setup

1. Create a GitHub environment named `release` and require maintainer approval.
2. Enable immutable GitHub Releases.
3. If repository Actions are allowlisted, allow only the pinned
   `release-plz/action@2eb1d8bcb770b4c48ccfaad919734b38b51958c9` revision
   and keep SHA pinning required.
4. Create a GitHub App with repository **Contents** and **Pull requests** set to
   read/write. Set **Administration** to read/write so the App can create
   protected tags, disable its webhook, and install it only on this repository.
5. Store the App client ID in the repository variable
   `RELEASE_PLZ_APP_CLIENT_ID` and its private key in the repository secret
   `RELEASE_PLZ_APP_PRIVATE_KEY`.
6. Protect `v*` tags with a repository ruleset and add the App to the bypass
   list. Release maintainers may retain a manual bootstrap path.
7. Complete the `0.1.0` bootstrap above, then configure both crates.io trusted
   publishers. Remove any long-lived CI publishing token.

## Each release

1. Merge normal pull requests with Conventional Commits titles. On every push
   to `main`, release-plz opens or updates its release pull request.
2. Review the release pull request. Confirm that `terminfokit` and
   `terminfokit-cli` have the same version, the CLI's exact `terminfokit`
   dependency and `Cargo.lock` match it, and `CHANGELOG.md` contains all intended
   changes. Adjust the pull request before merge when needed.
3. Require the normal checks to pass. Run the **Release dry run** workflow and
   inspect all archives, six binaries, CycloneDX JSON SBOMs, `sha256.sum`, and
   auditable dependency metadata.
4. Merge the release pull request. The GitHub App creates exactly one protected
   `v<version>` tag; do not create another tag or GitHub Release manually.
5. Confirm that only the cargo-dist **Release** workflow starts from the tag,
   then approve the `release` environment deployment.
6. Verify the immutable GitHub Release, checksums, SBOMs, artifact attestations,
   and both crates.io versions.

Release only reviewed commits. Workflows skip existing crate versions.
Published crates and immutable release assets cannot be replaced.
