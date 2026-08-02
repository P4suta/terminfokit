<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Contributing

terminfokit is alpha-quality. Compatibility may change to match ncurses or
clarify the public API.

## Before opening a change

- Keep each pull request focused.
- Open an issue before large API changes, new compatibility targets, or changes
  to explicit non-goals.
- Report vulnerabilities privately as described in
  [SECURITY.md](SECURITY.md).

## Development setup

Install Rust 1.88 or newer. CI lints with stable and tests the minimum supported
Rust version (MSRV), 1.88.

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

CI also checks `wasm32-unknown-unknown`, `thumbv7em-none-eabi`, fuzz targets,
package contents, rustdoc warnings, and the MSRV.

## Conformance changes

Parser, resolver, schema, and binary writer changes need a focused regression
test. If ncurses-compatible output can change, run the pinned oracle:

```text
./scripts/fetch-ncurses-oracle.sh target/oracle
cargo run --locked -p terminfokit-conformance -- \
  target/oracle/ncurses-6.6.tar.gz target/full-oracle "$(date -u +%F)"
```

On Windows, run this check in a Linux container to avoid host `sh` and `make`
dependencies.

The required result is all 1,861 logical ncurses 6.6 entries, normal and `-x`
trees, no unallowlisted differences, and byte-exact oracle decode/re-encode.

## Pull requests

Describe the result and reason. Test new behavior, document API and CLI changes,
and keep `README.md` and `crates/terminfokit/README.md` identical.

All required checks must pass before merge. See
[RELEASING.md](RELEASING.md) for release steps.

Contributions are licensed under MIT OR Apache-2.0 unless stated otherwise.
