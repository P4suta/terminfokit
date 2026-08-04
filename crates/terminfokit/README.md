<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# terminfokit

[![CI](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml/badge.svg)](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`terminfokit` compiles, decodes, edits, decompiles, queries, and expands
terminfo descriptions.

The library is `no_std + alloc` and `#![forbid(unsafe_code)]`. Its only
dependency, behind the default `std` feature, is `atomic-write-file` for
database installation; with default features off it has none.

The 0.1 series is alpha-quality. APIs and CLI behavior may change.

## Checked against ncurses, byte for byte

Compiled terminfo is a binary format whose exact layout matters, so
compatibility here is measured rather than asserted. The conformance harness
downloads and builds the real `tic` from ncurses 6.6, compiles all 1,861
entries of its `terminfo.src` with both implementations, and compares every
output file byte for byte in normal and `-x` extended mode. It then decodes and
re-encodes each file ncurses produced and requires the round trip to reproduce
the original bytes. Differences can be waived only through an allowlist whose
entries carry a mandatory expiry, and a stale allowlist is itself reported as a
difference. The comparison runs on every pull request.

## Installation

```console
cargo install terminfokit-cli
```

This installs `terminfokit` plus `tik-tic`, `tik-infocmp`, `tik-tput`,
`tik-captoinfo`, and `tik-infotocap`. The ncurses names are deliberately
prefixed: `cargo install` writes into a directory that rustup places ahead of
the system path, so binaries named `tput` or `tic` would replace the ncurses
tools for every shell script, prompt, and curses program on the machine.

## Usage

```console
terminfokit --help
terminfokit doctor
tik-tput colors
```

Compile a terminfo source entry in memory:

```rust
use terminfokit::Compiler;

let source = b"demo|demo terminal,am,cols#80,cup=\\E[%i%p1%d;%p2%dH,\n";
let compilation = Compiler::new().compile(source)?;
let demo = compilation.get("demo").expect("compiled entry");

assert_eq!(demo.entry().names().primary(), "demo");
assert!(!demo.bytes().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Windows

Windows has no system terminfo database, so there is no default search path to
fall back on. Every lookup fails with `NotFound` until you point the tools at a
directory you control:

```console
set TERMINFO=C:\terminfo
terminfokit doctor
```

`TERMINFO_DIRS` and the `hex:` and `b64:` inline transports work the same way
as on Unix. `$HOME/.terminfo` is Unix-only. The library's parsing, compilation,
formatting, and expansion are unaffected and behave identically everywhere.

## Not implemented

The 0.1 series covers the System V capability vocabulary and directory-backed
databases. It does not implement the ncurses hashed-database file format, and
`tic -R`, `infocmp -R`, `infocmp -e`, `infocmp -E`, and `infocmp -i` are
unsupported; each reports a diagnostic naming the supported alternative.

## Contributing

See
[CONTRIBUTING.md](https://github.com/P4suta/terminfokit/blob/main/CONTRIBUTING.md).
Report vulnerabilities with
[GitHub private vulnerability reporting](https://github.com/P4suta/terminfokit/security/advisories/new),
not a public issue.

## License

The code is licensed under [MIT](https://github.com/P4suta/terminfokit/blob/main/LICENSE-MIT)
OR [Apache-2.0](https://github.com/P4suta/terminfokit/blob/main/LICENSE-APACHE).
The ncurses-derived capability metadata is also covered by the
[X11 license](https://github.com/P4suta/terminfokit/blob/main/crates/terminfokit/LICENSE-NCURSES),
so the library's SPDX expression is `(MIT OR Apache-2.0) AND X11`. Downstream
license audits need to allow X11 for this crate.
