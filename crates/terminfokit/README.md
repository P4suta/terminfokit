<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# terminfokit

Rust terminfo compiler, database, editor, and CLI tools.
Requires Rust 1.88 or newer.
The 0.1 series is alpha software, and its APIs may change.

## Installation

```console
cargo install terminfokit-cli --locked
```

## Usage

```console
terminfokit --help
```

See the [usage guide](https://github.com/P4suta/terminfokit/blob/main/docs/usage.md) for database setup, compatibility, and command details.

## Library

```rust
use terminfokit::Compiler;

let source = b"demo|demo terminal,am,cols#80,cup=\\E[%i%p1%d;%p2%dH,\n";
let compilation = Compiler::new().compile(source)?;
let demo = compilation.get("demo").expect("compiled entry");

assert_eq!(demo.entry().names().primary(), "demo");
assert!(!demo.bytes().is_empty());
# Ok::<(), Box<dyn std::error::Error>>(())
```

## License

[MIT](https://github.com/P4suta/terminfokit/blob/main/LICENSE-MIT) OR [Apache-2.0](https://github.com/P4suta/terminfokit/blob/main/LICENSE-APACHE).
The library also includes ncurses metadata under [X11](https://github.com/P4suta/terminfokit/blob/main/crates/terminfokit/LICENSE-NCURSES), with the expression `(MIT OR Apache-2.0) AND X11`.
