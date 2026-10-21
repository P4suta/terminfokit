---
doppler_automation_app: release_plz
---

<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Credential delivery

GitHub Actions exchanges its workload identity for a short-lived Doppler token at the credential-consuming step.
Each purpose has its own read-only service account, config, and environment variable.
Shared signing credentials stay in the canonical signing project and are referenced from the consumer config.
The shared credential still carries the signing provider's authority after retrieval.

| Purpose | Doppler config | GitHub environment variable | Environments |
| --- | --- | --- | --- |
| automation | `terminfokit.automation` | `DOPPLER_AUTOMATION_IDENTITY_ID` | release-plz |

The manual **Verify credential access** workflow checks required fields and rejects access to an unrelated config.
Automation also creates a repository-scoped App token with contents read access.
This workflow performs no signing, notarization, registry publication, release creation, or deployment.
Existing environment protections remain in force; their approval does not grant authority for another operation.

After the reviewed workflows reach the default branch, run the credential-access check for that exact revision.
Keep the current credential delivery until every purpose and environment succeeds and the denial checks pass.
Confirm an explicit access-denied response for each negative check.
Keep the selected identity fixed for that scope check.
A workflow can exchange for every identity whose claims it satisfies; separate accounts alone don't isolate stages with matching claim policies.
Network, authentication, and runner failures leave that evidence inconclusive.
Then disconnect the corresponding Doppler-to-GitHub sync while preserving its target fields, check for remaining consumers, and remove the obsolete GitHub fields and legacy configs.
An expired or revoked registry key is a retirement candidate and must not be copied into a replacement config.
Use the registry's native trusted publisher where supported; an unconfigured publisher stays unavailable until its owner configures the exact binding.

Store only field names, identity metadata, source revisions, and verification outcomes in issues and logs.
Keep credential values and native authentication state out of the repository.

The independent crates.io publication path uses native Trusted Publishing for `terminfokit` and `terminfokit-cli` in the protected `release` environment.
The current crates.io verifier selects the workflow filename from `workflow_ref`, so bind this reusable call to its `v-release.yml` caller and confirm that claim before registration.
The [official claim parser](https://github.com/rust-lang/crates.io/blob/main/crates/crates_io_trustpub/src/github/claims.rs) defines this boundary.
Keep each package's registration explicit and leave an unregistered or unpublished package unavailable.
The workflow has no bootstrap-token selection or fallback to a saved registry key.
