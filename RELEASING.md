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
workflow a crates.io token or an OpenID Connect permission. Publication always
happens in `publish-crates.yml`, whether it authenticates through trusted
publishing or, for a crate's first release, through a bootstrap token.

## Bootstrap release 0.1.0

A trusted publisher can only be registered against a crate that already exists,
so the first publication of each crate needs a token. It still goes through the
normal workflow rather than a local `cargo publish`:

1. Add `CARGO_REGISTRY_TOKEN` to the `release` environment. `publish-crates.yml`
   uses it in place of the OpenID Connect exchange whenever it is present, and
   says so in the job log.
2. Release `0.1.0` through the normal flow below. The publish job is idempotent:
   it queries crates.io for each version first, publishes `terminfokit`, waits
   for the index, and only then publishes `terminfokit-cli`.
3. Register trusted publishers for both crates using this repository, the
   `release` environment, and `.github/workflows/publish-crates.yml`.
4. Delete the `CARGO_REGISTRY_TOKEN` secret. From then on every release uses the
   short-lived OpenID Connect exchange and no long-lived credential exists.

## One-time repository setup

1. Create a GitHub environment named `release` and require maintainer approval.
2. Create a second environment named `release-plz` with no required reviewers
   and a deployment branch policy allowing only `main`. Keep the App
   credentials here so routine pushes cannot access them from another branch.
3. Enable immutable GitHub Releases.
4. If repository Actions are allowlisted, allow only the pinned
   `release-plz/action@2eb1d8bcb770b4c48ccfaad919734b38b51958c9` revision
   and keep SHA pinning required.
5. Use the account-wide `p4suta-release-plz` GitHub App already installed on
   this repository. It needs repository **Contents** and **Pull requests**
   read/write access; do not create a repository-specific App.
6. Store its client ID as the `release-plz` environment variable
   `RELEASE_PLZ_APP_CLIENT_ID` and its private key as the environment secret
   `RELEASE_PLZ_APP_PRIVATE_KEY`.
7. Protect existing `v*` tags from update, force-update, and deletion with a
   repository ruleset. Do not restrict tag creation, so the shared App does not
   need an Administration permission or ruleset bypass.
8. Complete the `0.1.0` bootstrap above, then configure both crates.io trusted
   publishers and delete the bootstrap token.

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
