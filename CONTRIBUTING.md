<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Contributing

terminfokit is alpha software.
Compatibility may change to match ncurses or clarify the public API.

## Before opening a change

- Keep each pull request focused.
- Open an issue before large API changes, new compatibility targets, or changes to explicit non-goals.
- Report vulnerabilities privately as described in [SECURITY.md](SECURITY.md).

## Development setup

Install Rust 1.88 or newer.
CI lints with stable Rust and tests Rust 1.88 on Linux.
Stable tests also cover Windows and macOS.

The main local checks are:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check -p terminfokit --no-default-features
typos
reuse lint
dist generate --check
dist plan
```

Relevant source changes also select checks for `wasm32-unknown-unknown`, `thumbv7em-none-eabi`, fuzz target compilation, package contents, and rustdoc warnings.
Run fuzz targets locally when investigating parser defects.
CI checks workflow syntax, license notices, spelling, and matching README copies on every change.
It selects native tests, reference comparison, and release generation from the [affected paths](.github/filters/ci.yml).
The [verification decision](docs/adr/verification-contracts.md) describes the boundaries.
Manual CI selects every lane, and BSD and repository security checks remain available manually.

## Conformance changes

Parser, resolver, schema, and binary writer changes need a focused regression test.
If ncurses-compatible output can change, run the pinned oracle:

```text
./scripts/fetch-ncurses-oracle.sh target/oracle
cargo run --locked -p terminfokit-conformance -- \
  target/oracle/ncurses-6.6.tar.gz target/full-oracle "$(date -u +%F)"
```

On Windows, run this check in a Linux container to provide `sh` and `make`.

The required result covers all 1,861 logical ncurses 6.6 entries and both normal and `-x` trees.
Every difference needs an explicit accepted entry, and decoding and re-encoding oracle files must preserve every byte.

## Pull requests

Describe the result and reason.
Test new behavior, document API and CLI changes, and keep `README.md` and `crates/terminfokit/README.md` identical.

Use a [Conventional Commits](https://www.conventionalcommits.org/) title for every pull request, such as `fix: handle an empty terminal name`, `feat: add JSON output`, or `feat!: revise the public parser API`.
The squash merge commit uses the pull request title, and release-plz uses that commit to select the next version and build the changelog.

All required checks must pass before merge.
See [RELEASING.md](RELEASING.md) for release steps.

Contributions are licensed under MIT OR Apache-2.0 unless stated otherwise.
