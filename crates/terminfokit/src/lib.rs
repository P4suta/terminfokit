//! **terminfokit** — a pure-Rust terminfo *compiler* suite.
//!
//! The Rust ecosystem can read compiled terminfo, but it cannot write it.
//! `terminfokit` covers both directions: the core of `tic(1)`
//! (source → compiled), `infocmp(1)` (compiled → source), and `tput(1)`
//! (parameterized capability expansion).
//!
//! # Pipeline
//!
//! ```text
//!               parse_source                 resolve_use_chains
//!  source text ─────────────► [SourceEntry] ─────────────────► [ResolvedEntry]
//!                                                                     │
//!        bytes ◄──────────── Database ◄──────────────────────── lower ┘
//!              Database::write   ▲
//!        bytes ──────────────────┘
//!              Database::parse
//! ```
//!
//! # Crate features
//!
//! * `std` *(default)* — implies `alloc`; reserved for std-only conveniences.
//! * `alloc` — enables the pipeline modules ([`source`], [`resolve`],
//!   [`compiled`], [`expand`]), which need an allocator.
//!
//! With both features disabled, the crate still provides the capability
//! vocabulary ([`caps`]) and the [`error`] types.
//!
//! # Status
//!
//! Scaffold: the API is sketched and documented; every body is `todo!()`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod caps;
#[cfg(feature = "alloc")]
pub mod compiled;
pub mod error;
#[cfg(feature = "alloc")]
pub mod expand;
#[cfg(feature = "alloc")]
pub mod resolve;
#[cfg(feature = "alloc")]
pub mod source;

pub use error::{Error, Result};
