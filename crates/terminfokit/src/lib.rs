// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Terminfo compilation, formatting, database access, and expansion.
//!
//! The transformation core is `no_std` with `alloc`; enabling the default
//! `std` feature adds filesystem and environment-backed database access.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

extern crate alloc;

pub mod binary;
pub mod caps;
#[cfg(feature = "std")]
pub mod database;
pub mod error;
pub mod expand;
pub mod format;
pub mod model;
pub mod resolve;
pub mod source;
pub mod termcap;

pub use binary::{BinaryDocument, BinaryLimits, DecodeOptions, EncodeOptions, Magic, NumberFormat};
pub use caps::{
    BooleanCap, CapabilityId, CapabilityKind, CapabilityMetadata, Lookup, NameNamespace,
    NumericCap, ParameterType, StringCap, all_capabilities, lookup, lookup_all,
};
pub use error::{
    BuildError, CompileError, ConvertError, DecodeError, Diagnostic, EncodeError, ExpandError,
    ParseError, SourceId, Span, TextRange,
};
pub use model::{
    BooleanState, CapabilityState, CapabilityValueRef, Entry, EntryBuilder, EntryNames,
    ExtendedCapability, ExtendedKind, ExtendedValue, Number,
};
pub use resolve::{
    Compilation, CompiledEntry, Compiler, CompilerOptions, EntryProvider, ProviderError, Resolution,
};
pub use source::SourceDocument;
