# terminfokit

A pure-Rust **terminfo compiler suite**: the cores of `tic(1)` (source → compiled),
`infocmp(1)` (compiled → source), and `tput(1)` (parameterized capability expansion) —
with first-class support for *writing* the compiled format, not just reading it.

> **Status: scaffold.** The public API is sketched (types, signatures, docs); every
> implementation body is a `todo!()` stub. See the [Roadmap](#roadmap).

## Why

**The Rust ecosystem can read terminfo, but it cannot write it.**

Reading is a solved problem: the `terminfo` crate alone has 26.8M total downloads,
feeding termwiz/WezTerm (12.7M), color-print (6.0M), and sapling-streampager (2.0M).
But not a single published crate can *compile or write* a terminfo entry. All 8 crates
under the crates.io keyword `terminfo` and all 24 under `curses` were checked
individually; the closest candidate, `infoterm`, states in its documentation that it
has "no support for writing entries".

Real projects pay for that gap today by shelling out to C ncurses' `tic`:

| Project | External `tic` dependency |
| --- | --- |
| Alacritty | `Makefile` / `INSTALL.md` require `tic -xe alacritty,alacritty-direct` |
| WezTerm | `ci/deploy.sh` declares `makedepends="cmd:tic"`; the FAQ walks users through running `tic` themselves |
| Ghostty | terminfo step in `build.zig`; `src/cli/ssh.zig` distributes terminfo to SSH hosts via external `tic` |
| bootty (Rust) | runs `Command::new("tic")` at startup |
| yantra (Rust) | pipes `\| tic -x -` to the remote host over SSH |
| ori-term (Rust) | runs `tic -x -o tempdir` for every test |

The gap is not unique to Rust: neither Go nor Zig has a complete `tic` either —
`xo/terminfo` (Go) only decodes, and Ghostty's `Source.zig` encodes the *source*
format but cannot emit the compiled one.

## Prior art

The only known attempt at a Rust `tic` is `ncurses-tools` (`tic.rs` / `infocmp.rs`)
inside the [`infinityabundance/ncurses-native`] repository. It deserves real credit:
it proved the hard part is feasible by achieving **byte-identical output with ncurses
6.4's `tic` across all 2,869 terminals**. However, it is not published on crates.io
(`publish = false`), and it does not implement `use=` inheritance resolution — it can
only compile pre-resolved sources, which means it cannot compile ncurses'
`terminfo.src` master file.

terminfokit adopts the same bit-exact differential-testing bar
([ADR 0001](docs/adr/0001-oracle-differential-testing.md)) and treats `use=`
resolution as in-scope from the start
([ADR 0003](docs/adr/0003-use-resolution-in-scope.md)).

[`infinityabundance/ncurses-native`]: https://github.com/infinityabundance/ncurses-native

## Roadmap

- **M0** — compiled-format reader/writer with byte-identical round-trips:
  legacy 16-bit magic `0o432`, extended-number 32-bit magic `0o1036` (ncurses 6.1+),
  the extended (`-x`) capability section, string-table offsets, cancelled (`-2`) values.
- **M1** — terminfo source parser + `use=` chain resolution (cycle detection,
  well-defined merge order, `cap@` cancellation) + a `tic`-compatible CLI.
- **M2** — `infocmp` decompiler (`-1`, `-x`, `-C` termcap conversion) + `tput`.

## Non-goals

- **Not a curses reimplementation** — no windows, no panels, no input handling.
- **Not a rendering or TUI framework** — terminfokit produces and consumes terminal
  *descriptions*, it does not draw.
- **Not a terminal emulator.**

## Workspace layout

| Crate | Purpose |
| --- | --- |
| [`crates/terminfokit`](crates/terminfokit) | Core library: parse, resolve, compile, decompile, expand. `no_std` + `alloc`, zero dependencies, `forbid(unsafe_code)`. |
| [`crates/terminfokit-cli`](crates/terminfokit-cli) | `tic` / `infocmp` / `tput`-compatible command-line binaries. |
| [`crates/terminfokit-conformance`](crates/terminfokit-conformance) | Differential-testing harness against ncurses' `tic` as the oracle. |

See [ARCHITECTURE.md](ARCHITECTURE.md) for the data-flow pipeline.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
