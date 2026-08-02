// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Errors and machine-readable diagnostics.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Stable source-buffer identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SourceId(u32);

impl SourceId {
    /// Creates an identity from a caller-assigned integer.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the caller-assigned integer.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A half-open byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextRange {
    start: usize,
    end: usize,
}

impl TextRange {
    /// Creates a half-open range.
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the inclusive start byte offset.
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the exclusive end byte offset.
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns the saturating byte length.
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Reports whether both bounds are equal.
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A source identifier and byte range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    source_id: SourceId,
    range: TextRange,
}

impl Span {
    /// Pairs a source identity with a byte range.
    pub const fn new(source_id: SourceId, range: TextRange) -> Self {
        Self { source_id, range }
    }

    /// Creates a span in source zero from offset and length.
    pub const fn at(offset: usize, length: usize) -> Self {
        Self::new(
            SourceId::new(0),
            TextRange::new(offset, offset.saturating_add(length)),
        )
    }

    /// Returns the source identity.
    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    /// Returns the byte range.
    pub const fn range(self) -> TextRange {
        self.range
    }

    /// Reassigns the source identity while retaining the range.
    pub const fn with_source_id(mut self, value: SourceId) -> Self {
        self.source_id = value;
        self
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Severity {
    /// Processing continues, but output may be lossy.
    Warning,
    /// Processing cannot produce the requested result.
    Error,
}

/// A primary or secondary source label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticLabel {
    span: Span,
    message: Option<String>,
}

impl DiagnosticLabel {
    /// Creates a label with an optional rendered explanation.
    pub fn new(span: Span, message: Option<String>) -> Self {
        Self { span, message }
    }

    /// Returns the labelled span.
    pub const fn span(&self) -> Span {
        self.span
    }

    /// Returns the optional label-specific explanation.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }
}

/// A stable, render-independent diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    severity: Severity,
    code: &'static str,
    message: String,
    primary: Option<DiagnosticLabel>,
    secondary: Vec<DiagnosticLabel>,
    notes: Vec<String>,
}

impl Diagnostic {
    /// Creates an error diagnostic with an optional primary span.
    pub fn error(code: &'static str, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: span.map(|span| DiagnosticLabel::new(span, None)),
            secondary: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Creates a warning diagnostic with an optional primary span.
    pub fn warning(code: &'static str, message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            primary: span.map(|span| DiagnosticLabel::new(span, None)),
            secondary: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Returns diagnostic severity.
    pub const fn severity(&self) -> Severity {
        self.severity
    }

    /// Returns the stable machine-readable code.
    pub const fn code(&self) -> &'static str {
        self.code
    }

    /// Returns the main human-readable message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the optional primary label.
    pub fn primary(&self) -> Option<&DiagnosticLabel> {
        self.primary.as_ref()
    }

    /// Returns related source labels.
    pub fn secondary(&self) -> &[DiagnosticLabel] {
        &self.secondary
    }

    /// Returns additional explanatory notes.
    pub fn notes(&self) -> &[String] {
        &self.notes
    }

    /// Adds a message to the primary label when present.
    pub fn with_primary_message(mut self, message: impl Into<String>) -> Self {
        if let Some(primary) = &mut self.primary {
            primary.message = Some(message.into());
        }
        self
    }

    /// Adds a related source location.
    pub fn with_secondary(mut self, span: Span, message: impl Into<String>) -> Self {
        self.secondary
            .push(DiagnosticLabel::new(span, Some(message.into())));
        self
    }

    /// Adds a free-form explanatory note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Compiled-format decode failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeErrorKind {
    /// Input ended before the current section was complete.
    Truncated,
    /// Header magic is not a supported terminfo format.
    BadMagic(u16),
    /// A signed section count was negative.
    NegativeSize,
    /// A configured defensive or format size limit was exceeded.
    SizeLimit,
    /// Boolean slot contained an unknown sentinel.
    InvalidBoolean(u8),
    /// Numeric or string slot contained an invalid sentinel.
    InvalidSentinel(i64),
    /// String offset points outside its table.
    InvalidStringOffset,
    /// String table item has no terminating NUL.
    UnterminatedString,
    /// Terminal or extended capability name is invalid.
    InvalidName,
    /// Bytes remain after the final recognized section.
    TrailingData,
}

/// Compiled-format failure and byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError {
    offset: usize,
    kind: DecodeErrorKind,
}

impl DecodeError {
    pub(crate) const fn new(offset: usize, kind: DecodeErrorKind) -> Self {
        Self { offset, kind }
    }

    /// Returns the byte offset nearest the failure.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the structural failure category.
    pub const fn kind(&self) -> DecodeErrorKind {
        self.kind
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid compiled terminfo at byte {}: {:?}",
            self.offset, self.kind
        )
    }
}

/// Logical-entry serialization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// A named binary section exceeded a count or allocation limit.
    SizeLimit(&'static str),
    /// A number does not fit the requested legacy representation.
    LegacyNumber(i32),
    /// A number is outside the terminfo value domain.
    InvalidNumber(i32),
    /// A terminal or extended capability name is invalid.
    InvalidName(String),
    /// A named string contains an unrepresentable NUL.
    StringContainsNul(String),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SizeLimit(section) => write!(f, "{section} exceeds the compiled-format limit"),
            Self::LegacyNumber(value) => write!(f, "{value} does not fit the legacy number format"),
            Self::InvalidNumber(value) => {
                write!(f, "{value} is not a valid terminfo numeric value")
            }
            Self::InvalidName(name) => write!(f, "invalid terminal or capability name {name:?}"),
            Self::StringContainsNul(name) => write!(f, "capability {name:?} contains a NUL byte"),
        }
    }
}

/// Logical-entry construction or edit failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// No primary terminal name was supplied.
    EmptyPrimaryName,
    /// Terminal name is empty, unsafe, or contains a delimiter.
    InvalidName(String),
    /// Alias duplicates the primary name or another alias.
    DuplicateAlias(String),
    /// Extended capability name is empty or unsafe.
    InvalidCapabilityName(String),
    /// Numeric value is negative or too large.
    InvalidNumber(i64),
    /// Named string contains an unrepresentable NUL.
    StringContainsNul(String),
    /// Cancelling an unknown extension requires its type.
    ExtendedKindRequired(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid terminfo entry: {self:?}")
    }
}

/// Source parse failure and diagnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub(crate) diagnostic: Box<Diagnostic>,
}

impl ParseError {
    /// Wraps a structured parser diagnostic.
    pub fn new(diagnostic: Diagnostic) -> Self {
        Self {
            diagnostic: Box::new(diagnostic),
        }
    }

    /// Returns the parser diagnostic.
    pub fn diagnostic(&self) -> &Diagnostic {
        &self.diagnostic
    }

    /// Consumes the error and returns its diagnostic.
    pub fn into_diagnostic(self) -> Diagnostic {
        *self.diagnostic
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.diagnostic.code, self.diagnostic.message)
    }
}

/// Stage and category of a compilation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CompileErrorKind {
    /// Source tokenization or parsing failed.
    Parse,
    /// An inheritance target could not be found.
    MissingUse {
        /// Requested target name.
        name: String,
    },
    /// Inheritance contains a cycle.
    UseCycle {
        /// Complete cycle including the repeated first name.
        chain: Vec<String>,
    },
    /// One user-defined name has conflicting types.
    ExtendedTypeConflict {
        /// Conflicting capability name.
        name: String,
    },
    /// An external entry provider failed.
    Provider {
        /// Requested external name.
        name: String,
        /// Provider-supplied explanation.
        message: String,
    },
    /// Logical entry validation failed.
    Build(BuildError),
    /// Binary serialization failed.
    Encode(EncodeError),
}

/// Compilation failure and diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub(crate) kind: CompileErrorKind,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl CompileError {
    /// Constructs an error from its category and diagnostics.
    pub fn new(kind: CompileErrorKind, diagnostics: Vec<Diagnostic>) -> Self {
        Self { kind, diagnostics }
    }

    /// Returns the compilation failure category.
    pub fn kind(&self) -> &CompileErrorKind {
        &self.kind
    }

    /// Returns structured diagnostics in reporting order.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            CompileErrorKind::MissingUse { name } => write!(f, "unknown use= entry {name:?}"),
            CompileErrorKind::UseCycle { chain } => write!(f, "use= cycle: {}", chain.join(" -> ")),
            other => write!(f, "terminfo compilation failed: {other:?}"),
        }
    }
}

/// Parameter-program failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExpandErrorKind {
    /// An operator is missing required following bytes.
    TruncatedOperator,
    /// An operator needed more stack values.
    StackUnderflow,
    /// A value had the wrong number/string type.
    TypeMismatch,
    /// Parameter index is outside the supported range.
    InvalidParameter,
    /// Variable name is outside the supported range.
    InvalidVariable,
    /// Numeric literal or formatting operand is invalid.
    InvalidNumber,
    /// Division or remainder used a zero divisor.
    DivideByZero,
    /// Conditional markers are unbalanced.
    UnbalancedConditional,
    /// Expanded bytes exceed the configured limit.
    OutputLimit,
    /// Executed operators exceed the configured limit.
    StepLimit,
    /// Stack depth exceeds the configured limit.
    StackLimit,
}

/// Expansion failure and byte offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpandError {
    pub(crate) offset: usize,
    pub(crate) kind: ExpandErrorKind,
}

impl ExpandError {
    pub(crate) const fn new(offset: usize, kind: ExpandErrorKind) -> Self {
        Self { offset, kind }
    }

    /// Returns the program byte offset nearest the failure.
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the expansion failure category.
    pub const fn kind(&self) -> ExpandErrorKind {
        self.kind
    }
}

/// Terminfo/termcap conversion failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConvertError {
    /// Named capability has no representation under a strict profile.
    LossyCapability(String),
    /// Named parameter expression cannot be represented by termcap.
    UnsupportedExpression(String),
    /// Rendered termcap entry exceeds its profile limit.
    EntryTooLong {
        /// Actual rendered byte length.
        length: usize,
        /// Active profile limit.
        limit: usize,
    },
    /// Termcap inheritance compilation failed.
    Compile(CompileError),
}

impl fmt::Display for ConvertError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LossyCapability(name) => write!(
                f,
                "capability {name:?} has no lossless termcap representation"
            ),
            Self::UnsupportedExpression(name) => write!(
                f,
                "capability {name:?} uses a parameter expression termcap cannot represent"
            ),
            Self::EntryTooLong { length, limit } => {
                write!(f, "termcap entry is {length} bytes (profile limit {limit})")
            }
            Self::Compile(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "parameter expansion failed at byte {}: {:?}",
            self.offset, self.kind
        )
    }
}

/// Database access or decode failure.
#[cfg(feature = "std")]
#[derive(Debug)]
#[non_exhaustive]
pub enum DatabaseError {
    /// Requested terminal name is unsafe for filesystem lookup.
    InvalidName(String),
    /// No backend contained the requested terminal.
    NotFound(String),
    /// Selected backend or transport is unsupported or malformed.
    UnsupportedBackend(String),
    /// Decoded entry does not claim the requested name.
    NameMismatch {
        /// Name requested by the caller.
        requested: String,
        /// Primary name found in the decoded entry.
        decoded: String,
    },
    /// Lookup encountered a symlink which is not trusted.
    UntrustedSymlink(std::path::PathBuf),
    /// Compiled bytes were malformed.
    Decode(DecodeError),
    /// Operating-system I/O failed.
    Io(std::io::Error),
}

#[cfg(feature = "std")]
impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(name) => write!(f, "unsafe terminal name {name:?}"),
            Self::NotFound(name) => write!(f, "terminfo entry {name:?} was not found"),
            Self::UnsupportedBackend(kind) => write!(f, "unsupported terminfo backend: {kind}"),
            Self::NameMismatch { requested, decoded } => write!(
                f,
                "terminfo entry name mismatch: requested {requested:?}, decoded {decoded:?}"
            ),
            Self::UntrustedSymlink(path) => {
                write!(f, "refusing untrusted terminfo symlink {}", path.display())
            }
            Self::Decode(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
        }
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for DatabaseError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl core::error::Error for DecodeError {}
impl core::error::Error for EncodeError {}
impl core::error::Error for BuildError {}
impl core::error::Error for ParseError {}
impl core::error::Error for CompileError {}
impl core::error::Error for ExpandError {}
impl core::error::Error for ConvertError {}
#[cfg(feature = "std")]
impl core::error::Error for DatabaseError {}
