# terminfokit

[![CI](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml/badge.svg)](https://github.com/P4suta/terminfokit/actions/workflows/ci.yml)
[![CodeQL](https://github.com/P4suta/terminfokit/actions/workflows/codeql.yml/badge.svg)](https://github.com/P4suta/terminfokit/actions/workflows/codeql.yml)
[![BSD checks](https://github.com/P4suta/terminfokit/actions/workflows/bsd.yml/badge.svg)](https://github.com/P4suta/terminfokit/actions/workflows/bsd.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/P4suta/terminfokit/badge)](https://scorecard.dev/viewer/?uri=github.com/P4suta/terminfokit)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](https://github.com/P4suta/terminfokit#license)

`terminfokit` is a pure-Rust toolkit for compiling, decoding, editing,
decompiling, querying, and expanding terminfo descriptions. Its transformation
core is `no_std + alloc`; the default `std` feature adds directory databases,
ncurses-style search paths, atomic installation, and portable single-entry
transport.

The reason to choose it is the combination: most Rust terminal projects focus
on either reading a database or driving a terminal, while terminfokit keeps the
compiler, lossless editor, resolver, converter, database, parameter VM, and
daily command-line tools on one C-free data model.

The source repository is public, while the crates intentionally remain
`publish = false` until release packaging is ready. The current API is
pre-release and may still change before the final 1.0 release.

## Build from source

```text
git clone https://github.com/P4suta/terminfokit.git
cd terminfokit
cargo run -p terminfokit-cli --bin terminfokit -- --help
```

## What works

- both compiled magics (`0o432` 16-bit and `0o1036` 32-bit numbers), extended
  storage, alignment, absent/cancelled sentinels, and binary-safe strings;
- all 497 fixed ncurses capability slots generated from one declaration table;
- terminfo source escapes, comments, continuation, mixed capability ordering,
  aliases, forward references, reverse-processed multiple `use=` inheritance
  (the leftmost conflicting `use=` wins), cancellation, external providers,
  and complete cycle chains;
- deterministic compact/wrapped/one-per-line source output and logical diffs;
- the `tparm` stack language, persistent/static variables, conditionals,
  printf formatting, legacy operations, and structured padding-delay events;
- letter/hex directory layouts, alias links (copy fallback), standard Unix
  search roots, and `TERMINFO=hex:...` / `b64:...` transport;
- the unified `terminfokit` CLI plus `tic`, `infocmp`, `tput`, `captoinfo`,
  and `infotocap` compatibility binaries;
- conservative termcap conversion with strict and ncurses-style lossy modes.

## Rust API

Compile source entirely in memory:

```rust
use terminfokit::Compiler;

let source = b"demo|demo terminal,am,cols#80,cup=\\E[%i%p1%d;%p2%dH,\n";
let compilation = Compiler::new().compile(source)?;
let demo = compilation.get("demo").expect("compiled entry");
assert!(!demo.bytes().is_empty());
assert_eq!(demo.entry().names().primary(), "demo");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Install an entry in a directory database (aliases become hard links where the
filesystem permits it):

```no_run
# #[cfg(feature = "std")]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use terminfokit::Compiler;
use terminfokit::database::{DirectoryDatabase, InstallOptions};

let compilation = Compiler::new().compile(b"demo|demo terminal,am,\n")?;
let database = DirectoryDatabase::new("./terminfo-out");
database.install(
    compilation.get("demo").unwrap().entry(),
    InstallOptions::default(),
)?;
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

Load and query the current terminal:

```no_run
# #[cfg(feature = "std")]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use terminfokit::CapabilityState;
use terminfokit::caps::NumericCap;
use terminfokit::database::load_from_env;

let term = std::env::var("TERM")?;
let entry = load_from_env(&term)?;
if let CapabilityState::Value(columns) = entry.number(NumericCap::COLUMNS) {
    println!("database width: {columns}");
}
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

Expand `cup` without assuming UTF-8:

```rust
use terminfokit::expand::{Param, expand};

let cup = b"\x1b[%i%p1%d;%p2%dH";
let bytes = expand(cup, &[Param::Number(3), Param::Number(7)])?;
assert_eq!(bytes, b"\x1b[4;8H");
# Ok::<(), terminfokit::ExpandError>(())
```

Carry one compiled entry through an environment variable (useful for Windows,
SSH, and containers with no installed terminfo tree):

```rust
# #[cfg(feature = "std")]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use terminfokit::Compiler;
use terminfokit::database::{TransportEncoding, encode_transport};

let compilation = Compiler::new().compile(b"demo|demo terminal,am,\n")?;
let value = encode_transport(
    compilation.get("demo").unwrap().entry(),
    TransportEncoding::Base64,
)?;
assert!(value.starts_with("b64:"));
# Ok(())
# }
# #[cfg(not(feature = "std"))]
# fn main() {}
```

Load provenance is available through `load_from_env_report` and
`SearchPath::load_report`; it distinguishes inline hex/base64 transport from
the exact selected directory path and letter/hex layout.

## Unified CLI

`terminfokit` is the human-facing entry point. It dispatches in-process to the
same command implementations used by the compatibility binaries.

```text
terminfokit compile -x -e alacritty -o ./db alacritty.info
terminfokit inspect -1 -x -A ./db alacritty
terminfokit query -T alacritty cup 3 7
terminfokit convert termcap-to-terminfo legacy.termcap
terminfokit convert terminfo-to-termcap modern.info
terminfokit doctor
```

`doctor` is side-effect-free: it reports `TERM`, directory search order, exact
transport/path/layout, structured names, binary format, size and color values,
major capabilities, and the extended capability count. It never runs `iprog`
or changes terminal modes.

## Compatibility binaries

```text
tic -x -e alacritty,alacritty-direct -o ./db alacritty.info
infocmp -1 -x -A ./db alacritty
tput -T alacritty cup 3 7
captoinfo legacy.termcap
infotocap -K modern.info
```

`tic` supports compilation, selection with `-e`, database installation,
check-only and directory modes, short/long source and termcap translation,
transport output, width/layout selection, extended capabilities, and
dot-commented capability retention. `infocmp` supports listing, typed
comparison, ordered relative `use=` output, padding-insensitive comparison,
separate database roots, transport output, and termcap translation. `tput`
supports `-T`, `-S`, `-V`, multiple operands, standard and extended lookup,
binary stdout, `clear -x`, `longname`, runtime-aware `cols`/`lines`, and ordered
`init`/`reset` processing. Explicit initialization runs `iprog` unless
`--no-init-program` is supplied. On Unix, terminal mode changes use a safe
termios wrapper and search stderr, stdout, stdin, then `/dev/tty`.
Every binary also supports human or newline-delimited JSON diagnostics with
`--diagnostic-format`.

## Design and compatibility

The compiled binary format, source escapes and inheritance semantics, and the
parameter VM follow ncurses. Byte-exact comparison is a conformance technique,
not a promise to reproduce every future ncurses serialization choice.

The current `ncurses-native` project already includes a writer and a `tic`
implementation. Its compiler operates on resolved entries; terminfokit focuses
on unresolved source graphs, a `no_std` transformation core, editable typed
entries that preserve cancellation, external `EntryProvider`s, and termcap
conversion.

## Explicit non-goals

Hashed Berkeley DB and NetBSD CDB databases are deliberately unsupported in the
first release. `tic/infocmp -R`, `infocmp -e/-E/-i`, exact ncurses diagnostic
wording, archaic subset generation, crate publication, release artifacts, and
distribution workflows are also outside this internal-completion milestone.
Unsupported compatibility options are usage errors rather than silently
ignored.

The pinned full-oracle runner verifies the unmodified ncurses 6.6 archive and
`terminfo.src` hashes, requires all 1,861 logical entries, compares normal and
`-x` compiled trees, and independently decode/re-encodes every oracle file.
The current fixture passes with 2,899 files in each mode, zero tree
differences, and zero re-encode mismatches. Ordinary tests remain offline.

See [ARCHITECTURE.md](https://github.com/P4suta/terminfokit/blob/main/ARCHITECTURE.md)
and the [architecture decision records](https://github.com/P4suta/terminfokit/tree/main/docs/adr).
The binary reference is ncurses [`term(5)`](https://invisible-island.net/ncurses/man/term.5.html),
and source inheritance follows [`terminfo(5)`](https://invisible-island.net/ncurses/man/terminfo.5.html).

## Contributing and security

See [CONTRIBUTING.md](https://github.com/P4suta/terminfokit/blob/main/CONTRIBUTING.md)
for the development workflow. Please report suspected vulnerabilities through
[GitHub private vulnerability reporting](https://github.com/P4suta/terminfokit/security/advisories/new),
not a public issue; details are in the
[security policy](https://github.com/P4suta/terminfokit/blob/main/SECURITY.md).

## License

Licensed under either
[Apache-2.0](https://github.com/P4suta/terminfokit/blob/main/LICENSE-APACHE) or
[MIT](https://github.com/P4suta/terminfokit/blob/main/LICENSE-MIT), at your
option. Capability metadata derived from ncurses retains its
[third-party notice](https://github.com/P4suta/terminfokit/blob/main/crates/terminfokit/LICENSE-NCURSES).
