<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# terminfokit

[![CI](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml/badge.svg)](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

`terminfokit` compiles, decodes, edits, decompiles, queries, and expands
terminfo descriptions. The library supports `no_std + alloc`. The CLI provides
`terminfokit`, `tic`, `infocmp`, `tput`, `captoinfo`, and `infotocap`.

The 0.1 series is alpha-quality. APIs and CLI behavior may change.

## Build from source

Rust 1.88 or newer is required.

```console
git clone https://github.com/P4suta/terminfokit.git
cd terminfokit
cargo build --release -p terminfokit-cli
```

## Usage

```console
target/release/terminfokit --help
target/release/terminfokit doctor
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
[X11 license](https://github.com/P4suta/terminfokit/blob/main/crates/terminfokit/LICENSE-NCURSES).
