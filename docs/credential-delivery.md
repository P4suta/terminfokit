---
doppler_automation_app: release_plz
---

<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Credential delivery

The release App credentials use Doppler's [native GitHub synchronization](https://docs.doppler.com/docs/github-actions).
The [delivery decision](adr/sync-release-app-credentials-to-the-protected-environment.md) governs the cutover.
Canonical values remain in Doppler, and a purpose-specific config references the automation fields.
The [manifest](../automation/release-app-sync.toml) identifies the repository, environment, and field mapping.

The release-plz environment admits main jobs and prevents administrator bypass.
Its RELEASE_APP_CLIENT_ID and RELEASE_APP_PRIVATE_KEY secrets reach only the App token action.
Consumers reject empty inputs and unresolved references before minting tokens for the current repository and their explicit permissions.
The shared App key retains authority across its installations.
Protected main and environment policies replace Doppler workflow and event claims.

The manual **Verify credential access** workflow checks the selected fields and creates a repository-scoped token with contents read access.
It requires the exact repository identity and main ref.
The action revokes the token during cleanup.
Existing publication and registry identity gates remain independent of authentication.

Verify the exact synchronization target and successful initial synchronization before merging the workflow cutover.
Pause the **Release-plz** workflow during the cutover because main updates can create version tags.
After merging, run the manual authentication workflow for reviewed main and restore the publisher's previous workflow state.
Keep the existing Doppler identity until authentication succeeds and all consumers use the replacement.
Replace credentials in their canonical Doppler config, verify synchronization, and repeat authentication before retiring the previous key.

Store only field names, identity metadata, source revisions, and verification outcomes in issues and logs.
Keep credential values and native authentication state out of the repository.

The independent crates.io publication path uses native Trusted Publishing for `terminfokit` and `terminfokit-cli` in the protected `release` environment.
The current crates.io verifier selects the workflow filename from `workflow_ref`, so bind this reusable call to its `v-release.yml` caller and confirm that claim before registration.
The [official claim parser](https://github.com/rust-lang/crates.io/blob/main/crates/crates_io_trustpub/src/github/claims.rs) defines this boundary.
Keep each package's registration explicit and leave an unregistered or unpublished package unavailable.
The workflow has no bootstrap-token selection or fallback to a saved registry key.
