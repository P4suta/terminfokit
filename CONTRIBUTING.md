# Contributing to terminfokit

Thanks for helping improve terminfokit. The project is currently an alpha and
keeps all crates at `publish = false`, so compatibility may still change when a
change is justified by ncurses behavior or a clearer public API.

## Before opening a change

- Use a focused branch and keep unrelated changes out of the same pull request.
- Open an issue first for large API changes, new compatibility targets, or
  changes to explicit non-goals.
- Report suspected vulnerabilities privately as described in
  [SECURITY.md](SECURITY.md).

## Development setup

Install Rust 1.88 or newer. The stable toolchain is used for linting, while
1.88 is the minimum-supported Rust version (MSRV).

The main local checks are:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check -p terminfokit --no-default-features
```

The CI workflow also checks `wasm32-unknown-unknown`,
`thumbv7em-none-eabi`, fuzz targets, package contents, rustdoc warnings, and
the declared MSRV.

## Conformance changes

Parser, resolver, schema, or binary writer changes should include a focused
regression test. When they can affect ncurses output, also run the pinned full
oracle:

```text
./scripts/fetch-ncurses-oracle.sh target/oracle
cargo run --locked -p terminfokit-conformance -- \
  target/oracle/ncurses-6.6.tar.gz target/full-oracle "$(date -u +%F)"
```

On Windows, running this check in a Linux Docker container is supported and
does not require installing `sh` or `make` on the host.

The acceptance baseline is all 1,861 logical ncurses 6.6 entries, both normal
and `-x` trees, zero unallowlisted differences, and byte-exact oracle
decode/re-encode.

## Pull requests

Explain the user-visible result and why the change is needed. Include tests for
new behavior, update public documentation when APIs or commands change, and
keep `README.md` and `crates/terminfokit/README.md` identical.

All required GitHub Actions and code-scanning checks must pass before merge.

Unless you state otherwise, contributions intentionally submitted for
inclusion in this project are licensed under MIT OR Apache-2.0.
