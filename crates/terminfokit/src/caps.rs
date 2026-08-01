//! Typed access to terminal capabilities.
//!
//! Predefined capabilities are identified by their fixed index into the
//! compiled format's boolean/numeric/string sections — the order of ncurses'
//! capability table. Extended capabilities (`tic -x`) have no fixed index and
//! are identified by name instead.
//!
//! The enums below are a *sketch*: a few representative variants with their
//! real table indices. The full predefined tables land with M0.

/// Predefined boolean capabilities.
///
/// The discriminant is the capability's index into the compiled boolean
/// section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u16)]
pub enum BooleanCap {
    /// `bw` — `cub1` wraps from column 0 to the last column.
    AutoLeftMargin = 0,
    /// `am` — terminal has automatic margins.
    AutoRightMargin = 1,
    /// `xenl` — newline is ignored after 80 columns.
    EatNewlineGlitch = 4,
    /// `km` — terminal has a meta key.
    HasMetaKey = 8,
}

/// Predefined numeric capabilities.
///
/// The discriminant is the capability's index into the compiled numeric
/// section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u16)]
pub enum NumericCap {
    /// `cols` — number of columns in a line.
    Columns = 0,
    /// `lines` — number of lines on the screen.
    Lines = 2,
    /// `colors` — maximum number of colors. Values above `0x7fff` (e.g.
    /// direct-color terminals) are exactly why the `0o1036` extended-number
    /// format exists.
    MaxColors = 13,
}

/// Predefined string capabilities.
///
/// The discriminant is the capability's index into the compiled string
/// section (a table of offsets into the string table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[repr(u16)]
pub enum StringCap {
    /// `cbt` — back tab.
    BackTab = 0,
    /// `bel` — audible bell.
    Bell = 1,
    /// `cr` — carriage return.
    CarriageReturn = 2,
    /// `clear` — clear screen and home cursor.
    ClearScreen = 5,
    /// `cup` — move cursor to row `%p1`, column `%p2` (parameterized; see
    /// [`crate::expand`]).
    CursorAddress = 10,
}

impl BooleanCap {
    /// The short capability name as used in terminfo source, e.g. `"am"`.
    pub fn short_name(self) -> &'static str {
        todo!("full predefined boolean table (M0)")
    }
}

impl NumericCap {
    /// The short capability name as used in terminfo source, e.g. `"cols"`.
    pub fn short_name(self) -> &'static str {
        todo!("full predefined numeric table (M0)")
    }
}

impl StringCap {
    /// The short capability name as used in terminfo source, e.g. `"cup"`.
    pub fn short_name(self) -> &'static str {
        todo!("full predefined string table (M0)")
    }
}

/// The value of a capability, predefined or extended.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A boolean capability. Absent booleans are simply missing; `false` only
    /// occurs through cancellation semantics in some representations.
    Boolean(bool),
    /// A numeric capability. The legacy on-disk format stores 16-bit numbers,
    /// the `0o1036` format 32-bit ones.
    Number(i32),
    /// A string capability: raw bytes, possibly containing `%` expansion
    /// operators (see [`crate::expand`]).
    Str(alloc::vec::Vec<u8>),
    /// Explicitly cancelled (`cap@` in source). Serialized as `-2` in the
    /// compiled format.
    Cancelled,
}

/// The name of an extended (user-defined, `tic -x`) capability, e.g. `"Smulx"`
/// or `"kUP5"`.
///
/// Extended capabilities live in the extended storage section of the compiled
/// format and carry their names with them, unlike predefined capabilities
/// which are identified purely by index.
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtendedCapName(pub alloc::string::String);
