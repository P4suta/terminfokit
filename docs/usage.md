<!--
SPDX-FileCopyrightText: 2026 Yasunobu Sakashita

SPDX-License-Identifier: MIT OR Apache-2.0
-->

# Usage guide

## Commands and library

The CLI compiles, decodes, edits, decompiles, queries, and expands terminfo descriptions.
Installation provides `terminfokit`, `tik-tic`, `tik-infocmp`, `tik-tput`, `tik-captoinfo`, and `tik-infotocap`.
The prefixed names preserve the system ncurses commands when the Cargo binary directory comes first in the search path.

```console
terminfokit --help
terminfokit doctor
tik-tput colors
```

The library supports `no_std` with `alloc` and forbids unsafe code.
The default `std` feature enables database installation through `atomic-write-file`.
Disabling that feature removes the library's dependencies.
The 0.1 series supports the System V capability vocabulary and directory databases.
It omits the ncurses hashed database format and the `tic -R`, `infocmp -R`, `infocmp -e`, `infocmp -E`, and `infocmp -i` options.
Each unsupported option names a supported alternative.

## Windows database setup

Windows has no default system terminfo database.
Set `TERMINFO` to an existing database directory before looking up entries.
For Command Prompt:

```console
set TERMINFO=C:\terminfo
terminfokit doctor
```

`TERMINFO_DIRS` and the `hex:` and `b64:` inline transports also work on Windows.
The home directory database at `$HOME/.terminfo` is specific to Unix.
Parsing, compilation, formatting, and expansion use the same implementation on every host.

## Compatibility checks

The conformance harness builds ncurses 6.6 and compiles its 1,861 logical entries with both implementations.
It compares every output byte in normal and extended `-x` modes.
It also requires decoding and re-encoding ncurses files to reproduce the original bytes.
Each accepted difference needs a reason and expiry, and an expired entry fails the comparison.
CI selects this comparison for library format, capability, conformance, dependency, and oracle input changes.
See the [verification decision](adr/verification-contracts.md) and [contribution guide](../CONTRIBUTING.md).

## Licenses and contributions

The code uses MIT OR Apache-2.0.
The ncurses capability metadata also carries X11 terms, so library consumers need `(MIT OR Apache-2.0) AND X11`.
See [LICENSE-NCURSES](../crates/terminfokit/LICENSE-NCURSES) for the preserved notice.

See [CONTRIBUTING.md](../CONTRIBUTING.md) to contribute.
Report vulnerabilities through [private vulnerability reporting](https://github.com/P4suta/terminfokit/security/advisories/new).
