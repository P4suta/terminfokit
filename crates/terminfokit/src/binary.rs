//! ncurses-compatible compiled terminfo decoding and encoding.

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::error::{DecodeError, DecodeErrorKind, EncodeError};
use crate::model::{
    BooleanState, CapabilityState, Entry, EntryNames, ExtendedCapability, ExtendedKind,
    ExtendedValue, Number,
};

const LEGACY_MAGIC: u16 = 0o432;
const EXTENDED_MAGIC: u16 = 0o1036;
const MAX_SECTION: usize = i16::MAX as usize;
const MAX_NAMES: usize = MAX_SECTION;
const DEFAULT_MAX_INPUT: usize = 16 * 1024 * 1024;
// ncurses keeps obsolete standard capabilities addressable, but its normal
// writer stops at these ABI-compatible ranges. \`tic -x\` uses the full tables.
const NORMAL_BOOLEAN_WRITE: usize = 37;
const NORMAL_NUMBER_WRITE: usize = 33;
const NORMAL_STRING_WRITE: usize = 394;

/// Numeric representation used in a compiled entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Magic {
    /// Original format with signed 16-bit numeric slots.
    Legacy,
    /// Extended format with signed 32-bit numeric slots.
    ExtendedNumbers,
}

impl Magic {
    /// Returns the little-endian header magic value.
    pub const fn value(self) -> u16 {
        match self {
            Self::Legacy => LEGACY_MAGIC,
            Self::ExtendedNumbers => EXTENDED_MAGIC,
        }
    }
}

/// Selection policy for numeric storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberFormat {
    /// Select 32-bit storage only when a value requires it.
    Auto,
    /// Require the original 16-bit numeric representation.
    Legacy,
    /// Always use the 32-bit numeric representation.
    Extended,
}

/// Resource limits for untrusted compiled entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BinaryLimits {
    max_input: usize,
    max_names: usize,
}

impl BinaryLimits {
    /// Returns defensive defaults suitable for untrusted input.
    pub const fn standard() -> Self {
        Self {
            max_input: DEFAULT_MAX_INPUT,
            max_names: MAX_NAMES,
        }
    }

    /// Returns limits which accept every representable allocation size.
    pub const fn unlimited() -> Self {
        Self {
            max_input: usize::MAX,
            max_names: usize::MAX,
        }
    }

    /// Returns the maximum complete input size.
    pub const fn max_input(self) -> usize {
        self.max_input
    }

    /// Returns the maximum names-section size.
    pub const fn max_names(self) -> usize {
        self.max_names
    }

    /// Replaces the maximum complete input size.
    pub const fn with_max_input(mut self, value: usize) -> Self {
        self.max_input = value;
        self
    }

    /// Sets the maximum accepted names-section size, including its trailing
    /// NUL. The compiled format can represent at most 32,767 bytes.
    pub const fn with_max_names(mut self, value: usize) -> Self {
        self.max_names = value;
        self
    }
}

impl Default for BinaryLimits {
    fn default() -> Self {
        Self::standard()
    }
}

/// Controls decoding of an untrusted compiled entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct DecodeOptions {
    limits: BinaryLimits,
}

impl DecodeOptions {
    /// Creates decode options with standard defensive limits.
    pub const fn new() -> Self {
        Self {
            limits: BinaryLimits::standard(),
        }
    }

    /// Returns the active resource limits.
    pub const fn limits(self) -> BinaryLimits {
        self.limits
    }

    /// Replaces the resource limits.
    pub const fn with_limits(mut self, limits: BinaryLimits) -> Self {
        self.limits = limits;
        self
    }
}

/// Controls compiled output without changing the logical entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct EncodeOptions {
    number_format: NumberFormat,
    extended: bool,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            number_format: NumberFormat::Auto,
            extended: true,
        }
    }
}

impl EncodeOptions {
    /// Creates options which select the numeric format automatically.
    pub const fn new() -> Self {
        Self {
            number_format: NumberFormat::Auto,
            extended: true,
        }
    }

    /// Returns the numeric-format policy.
    pub const fn number_format(self) -> NumberFormat {
        self.number_format
    }

    /// Reports whether the full ncurses \`-x\` standard-capability ranges are
    /// available to the writer.
    pub const fn extended(self) -> bool {
        self.extended
    }

    /// Replaces the numeric-format policy.
    pub const fn with_number_format(mut self, value: NumberFormat) -> Self {
        self.number_format = value;
        self
    }

    /// Enables or disables the full standard-capability ranges written by
    /// ncurses \`tic -x\`. User-defined capabilities are encoded in either
    /// mode when they are present on the logical entry.
    pub const fn with_extended(mut self, value: bool) -> Self {
        self.extended = value;
        self
    }
}

/// A decoded entry together with its exact compiled representation.
#[derive(Debug, Clone)]
pub struct BinaryDocument {
    entry: Entry,
    original: Vec<u8>,
    magic: Magic,
    edited: bool,
}

impl BinaryDocument {
    /// Returns the decoded logical entry.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// Returns the entry mutably and marks its representation as edited.
    pub fn entry_mut(&mut self) -> &mut Entry {
        self.edited = true;
        &mut self.entry
    }

    /// Consumes the document and returns its logical entry.
    pub fn into_entry(self) -> Entry {
        self.entry
    }

    /// Returns the numeric format read from the header.
    pub const fn magic(&self) -> Magic {
        self.magic
    }

    /// Preserve the original bytes until the logical entry is edited.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        if !self.edited {
            return Ok(self.original.clone());
        }
        encode(
            &self.entry,
            EncodeOptions::new().with_number_format(match self.magic {
                Magic::Legacy => NumberFormat::Legacy,
                Magic::ExtendedNumbers => NumberFormat::Extended,
            }),
        )
    }

    /// Encodes with explicit output options.
    pub fn to_bytes_with(&self, options: EncodeOptions) -> Result<Vec<u8>, EncodeError> {
        encode(&self.entry, options)
    }
}

/// Decode a compiled terminfo entry and retain its representation.
pub fn decode(data: &[u8]) -> Result<BinaryDocument, DecodeError> {
    decode_with(data, DecodeOptions::default())
}

/// Decodes a compiled entry with caller-supplied defensive limits.
pub fn decode_with(data: &[u8], options: DecodeOptions) -> Result<BinaryDocument, DecodeError> {
    let (entry, magic) = decode_entry(data, options.limits())?;
    Ok(BinaryDocument {
        entry,
        original: data.to_vec(),
        magic,
        edited: false,
    })
}

fn decode_entry(data: &[u8], limits: BinaryLimits) -> Result<(Entry, Magic), DecodeError> {
    if data.len() > limits.max_input() {
        return Err(DecodeError::new(0, DecodeErrorKind::SizeLimit));
    }
    let mut cursor = Cursor::new(data);
    let magic_offset = cursor.position();
    let magic = match cursor.u16()? {
        LEGACY_MAGIC => Magic::Legacy,
        EXTENDED_MAGIC => Magic::ExtendedNumbers,
        value => {
            return Err(DecodeError::new(
                magic_offset,
                DecodeErrorKind::BadMagic(value),
            ));
        }
    };
    let names_size = cursor.count()?;
    if names_size > limits.max_names() {
        return Err(DecodeError::new(2, DecodeErrorKind::SizeLimit));
    }
    let boolean_count = cursor.count()?;
    let number_count = cursor.count()?;
    let string_count = cursor.count()?;
    let table_size = cursor.count()?;

    let names_offset = cursor.position();
    let names_raw = cursor.take(names_size)?;
    if names_raw.last() != Some(&0) {
        return Err(DecodeError::new(
            names_offset,
            DecodeErrorKind::UnterminatedString,
        ));
    }
    let names_text = core::str::from_utf8(&names_raw[..names_raw.len().saturating_sub(1)])
        .map_err(|_| DecodeError::new(names_offset, DecodeErrorKind::InvalidName))?;
    let names = EntryNames::unpack(names_text)
        .map_err(|_| DecodeError::new(names_offset, DecodeErrorKind::InvalidName))?;

    let mut booleans = Vec::with_capacity(boolean_count);
    for _ in 0..boolean_count {
        let offset = cursor.position();
        booleans.push(match cursor.byte()? {
            0 | 0xff => BooleanState::Absent,
            1 => BooleanState::Set,
            0xfe => BooleanState::Cancelled,
            value => {
                return Err(DecodeError::new(
                    offset,
                    DecodeErrorKind::InvalidBoolean(value),
                ));
            }
        });
    }
    cursor.align_word()?;

    let mut numbers = Vec::with_capacity(number_count);
    for _ in 0..number_count {
        let offset = cursor.position();
        let value = cursor.number(magic)?;
        numbers.push(decode_number(value, offset)?);
    }

    let mut string_offsets = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        string_offsets.push((cursor.position(), cursor.i16()?));
    }
    let table_offset = cursor.position();
    let table = cursor.take(table_size)?;
    let mut strings = Vec::with_capacity(string_count);
    for (offset_at, value) in string_offsets {
        strings.push(decode_string_offset(value, table, table_offset, offset_at)?);
    }

    let mut extended = Vec::new();
    if cursor.remaining() != 0 {
        cursor.align_word()?;
        if cursor.remaining() != 0 {
            decode_extended(&mut cursor, magic, &mut extended)?;
        }
    }
    if cursor.remaining() != 0 {
        return Err(DecodeError::new(
            cursor.position(),
            DecodeErrorKind::TrailingData,
        ));
    }

    Ok((
        Entry {
            names,
            booleans,
            numbers,
            strings,
            extended,
        },
        magic,
    ))
}

fn decode_extended(
    cursor: &mut Cursor<'_>,
    magic: Magic,
    target: &mut Vec<ExtendedCapability>,
) -> Result<(), DecodeError> {
    let bool_count = cursor.count()?;
    let number_count = cursor.count()?;
    let string_count = cursor.count()?;
    // Since ncurses 6.2 this field is the number of strings which are
    // actually present in the extended string table: every capability name
    // plus every non-cancelled, non-absent string value.  Older writers used
    // the number of offset slots instead, so decoding deliberately treats it
    // as representation metadata rather than rejecting the old value.
    let string_usage = cursor.count()?;
    let table_size = cursor.count()?;
    let name_count = bool_count
        .checked_add(number_count)
        .and_then(|v| v.checked_add(string_count))
        .ok_or_else(|| DecodeError::new(cursor.position(), DecodeErrorKind::SizeLimit))?;
    let expected_offsets = name_count
        .checked_add(string_count)
        .ok_or_else(|| DecodeError::new(cursor.position(), DecodeErrorKind::SizeLimit))?;
    if string_usage > expected_offsets {
        return Err(DecodeError::new(
            cursor.position().saturating_sub(4),
            DecodeErrorKind::SizeLimit,
        ));
    }

    let mut bools = Vec::with_capacity(bool_count);
    for _ in 0..bool_count {
        let offset = cursor.position();
        bools.push(match cursor.byte()? {
            0 | 0xff => CapabilityState::Absent,
            1 => CapabilityState::Value(true),
            0xfe => CapabilityState::Cancelled,
            value => {
                return Err(DecodeError::new(
                    offset,
                    DecodeErrorKind::InvalidBoolean(value),
                ));
            }
        });
    }
    cursor.align_word()?;

    let mut numbers = Vec::with_capacity(number_count);
    for _ in 0..number_count {
        let offset = cursor.position();
        numbers.push(decode_number(cursor.number(magic)?, offset)?);
    }
    let mut value_offsets = Vec::with_capacity(string_count);
    for _ in 0..string_count {
        value_offsets.push((cursor.position(), cursor.i16()?));
    }
    let mut name_offsets = Vec::with_capacity(name_count);
    for _ in 0..name_count {
        name_offsets.push((cursor.position(), cursor.i16()?));
    }
    let table_start = cursor.position();
    let table = cursor.take(table_size)?;

    let mut name_base = 0usize;
    for (at, offset) in &value_offsets {
        if *offset >= 0 {
            let value = string_at(*offset, table, table_start, *at)?;
            name_base = name_base
                .checked_add(value.len() + 1)
                .ok_or_else(|| DecodeError::new(*at, DecodeErrorKind::SizeLimit))?;
        }
    }
    let name_table = table
        .get(name_base..)
        .ok_or_else(|| DecodeError::new(table_start, DecodeErrorKind::SizeLimit))?;
    let mut names = Vec::with_capacity(name_count);
    for (at, offset) in name_offsets {
        let bytes = string_at(offset, name_table, table_start + name_base, at)?;
        let name = core::str::from_utf8(bytes)
            .map_err(|_| DecodeError::new(at, DecodeErrorKind::InvalidName))?;
        names.push(name.to_string());
    }
    let mut names = names.into_iter();
    for state in bools {
        target.push(ExtendedCapability {
            name: names.next().unwrap_or_default(),
            kind: ExtendedKind::Boolean,
            state: state.map(|_| ExtendedValue::Boolean),
        });
    }
    for state in numbers {
        target.push(ExtendedCapability {
            name: names.next().unwrap_or_default(),
            kind: ExtendedKind::Number,
            state: state.map(ExtendedValue::Number),
        });
    }
    for (at, offset) in value_offsets {
        let state =
            decode_string_offset(offset, table, table_start, at)?.map(ExtendedValue::String);
        target.push(ExtendedCapability {
            name: names.next().unwrap_or_default(),
            kind: ExtendedKind::String,
            state,
        });
    }
    Ok(())
}

trait MapState<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> CapabilityState<U>;
}
impl<T> MapState<T> for CapabilityState<T> {
    fn map<U>(self, f: impl FnOnce(T) -> U) -> CapabilityState<U> {
        match self {
            CapabilityState::Absent => CapabilityState::Absent,
            CapabilityState::Cancelled => CapabilityState::Cancelled,
            CapabilityState::Value(value) => CapabilityState::Value(f(value)),
        }
    }
}

fn decode_number(value: i64, offset: usize) -> Result<CapabilityState<Number>, DecodeError> {
    match value {
        -1 => Ok(CapabilityState::Absent),
        -2 => Ok(CapabilityState::Cancelled),
        value if (0..=i64::from(i32::MAX)).contains(&value) => Ok(CapabilityState::Value(
            Number::new(value)
                .map_err(|_| DecodeError::new(offset, DecodeErrorKind::InvalidSentinel(value)))?,
        )),
        _ => Err(DecodeError::new(
            offset,
            DecodeErrorKind::InvalidSentinel(value),
        )),
    }
}

fn decode_string_offset(
    value: i16,
    table: &[u8],
    base: usize,
    at: usize,
) -> Result<CapabilityState<Vec<u8>>, DecodeError> {
    match value {
        -1 => Ok(CapabilityState::Absent),
        -2 => Ok(CapabilityState::Cancelled),
        0.. => Ok(CapabilityState::Value(
            string_at(value, table, base, at)?.to_vec(),
        )),
        _ => Err(DecodeError::new(
            at,
            DecodeErrorKind::InvalidSentinel(i64::from(value)),
        )),
    }
}

fn string_at(offset: i16, table: &[u8], base: usize, at: usize) -> Result<&[u8], DecodeError> {
    let offset = usize::try_from(offset)
        .map_err(|_| DecodeError::new(at, DecodeErrorKind::InvalidStringOffset))?;
    let tail = table
        .get(offset..)
        .ok_or_else(|| DecodeError::new(at, DecodeErrorKind::InvalidStringOffset))?;
    let end = tail
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| DecodeError::new(base + offset, DecodeErrorKind::UnterminatedString))?;
    Ok(&tail[..end])
}

/// Encode a logical terminfo entry.
pub fn encode(entry: &Entry, options: EncodeOptions) -> Result<Vec<u8>, EncodeError> {
    let magic = choose_magic(entry, options)?;
    let names = entry.names.packed();
    let names_bytes = names.as_bytes();
    if names_bytes.contains(&0) {
        return Err(EncodeError::InvalidName(names));
    }

    let (boolean_limit, number_limit, string_limit) = standard_write_limits(entry, options);
    let bool_len = entry.booleans[..boolean_limit]
        .iter()
        .rposition(|value| *value == BooleanState::Set)
        .map_or(0, |index| index + 1);
    let number_len = significant_len(&entry.numbers[..number_limit]);
    let string_len = significant_len(&entry.strings[..string_limit]);
    let mut string_table = Vec::new();
    let mut string_offsets = Vec::with_capacity(string_len);
    for (index, state) in entry.strings.iter().take(string_len).enumerate() {
        let state = match state {
            CapabilityState::Value(value) => {
                CapabilityState::Value(canonicalize_parameter_constants(value))
            }
            CapabilityState::Absent => CapabilityState::Absent,
            CapabilityState::Cancelled => CapabilityState::Cancelled,
        };
        string_offsets.push(encode_string_state(
            &state,
            &mut string_table,
            crate::caps::StringCap::from_index(index)
                .map(|c| c.short_name())
                .unwrap_or("string"),
        )?);
    }

    check_size("names", names_bytes.len() + 1)?;
    if names_bytes.len() + 1 > MAX_NAMES {
        return Err(EncodeError::SizeLimit("names"));
    }
    check_size("booleans", bool_len)?;
    check_size("numbers", number_len)?;
    check_size("strings", string_len)?;
    check_size("string table", string_table.len())?;

    let mut out = Vec::new();
    push_u16(&mut out, magic.value());
    push_count(&mut out, names_bytes.len() + 1, "names")?;
    push_count(&mut out, bool_len, "booleans")?;
    push_count(&mut out, number_len, "numbers")?;
    push_count(&mut out, string_len, "strings")?;
    push_count(&mut out, string_table.len(), "string table")?;
    out.extend_from_slice(names_bytes);
    out.push(0);
    for state in entry.booleans.iter().take(bool_len) {
        out.push(encode_bool(state));
    }
    align(&mut out);
    for state in entry.numbers.iter().take(number_len) {
        push_number(&mut out, state, magic)?;
    }
    for offset in string_offsets {
        push_i16(&mut out, offset);
    }
    out.extend_from_slice(&string_table);

    if !entry.extended.is_empty() {
        align(&mut out);
        encode_extended(entry, magic, &mut out)?;
    }
    if out.len() > DEFAULT_MAX_INPUT {
        return Err(EncodeError::SizeLimit("compiled entry"));
    }
    Ok(out)
}

fn canonicalize_parameter_constants(value: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        output.push(byte);
        index += 1;
        if byte == b'\\' {
            if let Some(escaped) = value.get(index) {
                output.push(*escaped);
                index += 1;
            }
            continue;
        }
        if byte != b'%' || value.get(index) != Some(&b'{') {
            continue;
        }
        let Some(relative_end) = value[index + 1..].iter().position(|byte| *byte == b'}') else {
            continue;
        };
        let end = index + 1 + relative_end;
        let Some(number) = parse_c_integer(&value[index + 1..end]) else {
            continue;
        };
        if (1..127).contains(&number)
            && number != i64::from(b'\\')
            && ((number as u8).is_ascii_graphic() || number == i64::from(b' '))
        {
            output.extend_from_slice(&[b'\'', number as u8, b'\'']);
            index = end + 1;
        }
    }
    if output.len() < value.len() {
        output
    } else {
        value.to_vec()
    }
}

fn parse_c_integer(value: &[u8]) -> Option<i64> {
    let text = core::str::from_utf8(value).ok()?;
    let (negative, unsigned) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };
    let (radix, digits) = if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (16, hex)
    } else if unsigned.len() > 1 && unsigned.starts_with('0') {
        (8, &unsigned[1..])
    } else {
        (10, unsigned)
    };
    if digits.is_empty() {
        return None;
    }
    let number = i64::from_str_radix(digits, radix).ok()?;
    Some(if negative { -number } else { number })
}

fn choose_magic(entry: &Entry, options: EncodeOptions) -> Result<Magic, EncodeError> {
    match options.number_format() {
        NumberFormat::Legacy => {
            validate_legacy(entry, options)?;
            Ok(Magic::Legacy)
        }
        NumberFormat::Extended => {
            validate_numbers(entry)?;
            Ok(Magic::ExtendedNumbers)
        }
        NumberFormat::Auto => {
            if all_numbers(entry, options).any(|value| value > i16::MAX as i32) {
                Ok(Magic::ExtendedNumbers)
            } else {
                Ok(Magic::Legacy)
            }
        }
    }
}

fn all_numbers(entry: &Entry, options: EncodeOptions) -> impl Iterator<Item = i32> + '_ {
    let (_, number_limit, _) = standard_write_limits(entry, options);
    entry
        .numbers
        .iter()
        .take(number_limit)
        .filter_map(|state| {
            if let CapabilityState::Value(value) = state {
                Some(value.get())
            } else {
                None
            }
        })
        .chain(entry.extended.iter().filter_map(|cap| {
            if let CapabilityState::Value(ExtendedValue::Number(value)) = &cap.state {
                Some(value.get())
            } else {
                None
            }
        }))
}
fn validate_numbers(_entry: &Entry) -> Result<(), EncodeError> {
    Ok(())
}
fn validate_legacy(entry: &Entry, options: EncodeOptions) -> Result<(), EncodeError> {
    validate_numbers(entry)?;
    for value in all_numbers(entry, options) {
        if value > i16::MAX as i32 {
            return Err(EncodeError::LegacyNumber(value));
        }
    }
    Ok(())
}

fn encode_extended(entry: &Entry, magic: Magic, out: &mut Vec<u8>) -> Result<(), EncodeError> {
    let mut bools: Vec<_> = entry
        .extended
        .iter()
        .filter(|cap| cap.kind == ExtendedKind::Boolean)
        .collect();
    let mut numbers: Vec<_> = entry
        .extended
        .iter()
        .filter(|cap| cap.kind == ExtendedKind::Number)
        .collect();
    let mut strings: Vec<_> = entry
        .extended
        .iter()
        .filter(|cap| cap.kind == ExtendedKind::String)
        .collect();
    for capabilities in [&mut bools, &mut numbers, &mut strings] {
        capabilities
            .sort_unstable_by(|left, right| left.name.as_bytes().cmp(right.name.as_bytes()));
    }
    let name_count = bools.len() + numbers.len() + strings.len();
    let offset_count = name_count + strings.len();
    for value in [bools.len(), numbers.len(), strings.len(), offset_count] {
        check_size("extended count", value)?;
    }

    let mut table = Vec::new();
    let mut value_offsets = Vec::new();
    for cap in &strings {
        let state = match &cap.state {
            CapabilityState::Value(ExtendedValue::String(value)) => {
                CapabilityState::Value(value.clone())
            }
            CapabilityState::Absent => CapabilityState::Absent,
            CapabilityState::Cancelled => CapabilityState::Cancelled,
            _ => return Err(EncodeError::InvalidName(cap.name.clone())),
        };
        value_offsets.push(encode_string_state(&state, &mut table, cap.name())?);
    }
    let string_usage = name_count + value_offsets.iter().filter(|offset| **offset >= 0).count();
    let name_base = table.len();
    let mut name_offsets = Vec::new();
    for cap in bools.iter().chain(numbers.iter()).chain(strings.iter()) {
        crate::model::validate_capability_name(cap.name())
            .map_err(|_| EncodeError::InvalidName(cap.name().to_string()))?;
        if cap.name().as_bytes().contains(&0) {
            return Err(EncodeError::InvalidName(cap.name().to_string()));
        }
        let offset = i16::try_from(table.len() - name_base)
            .map_err(|_| EncodeError::SizeLimit("extended string table"))?;
        name_offsets.push(offset);
        table.extend_from_slice(cap.name().as_bytes());
        table.push(0);
    }
    check_size("extended string table", table.len())?;
    push_count(out, bools.len(), "extended booleans")?;
    push_count(out, numbers.len(), "extended numbers")?;
    push_count(out, strings.len(), "extended strings")?;
    push_count(out, string_usage, "extended string usage")?;
    push_count(out, table.len(), "extended string table")?;
    for cap in &bools {
        out.push(match &cap.state {
            CapabilityState::Value(ExtendedValue::Boolean) => 1,
            CapabilityState::Absent => 0,
            CapabilityState::Cancelled => 0xfe,
            _ => return Err(EncodeError::InvalidName(cap.name.clone())),
        });
    }
    align(out);
    for cap in &numbers {
        let state = match &cap.state {
            CapabilityState::Value(ExtendedValue::Number(value)) => CapabilityState::Value(*value),
            CapabilityState::Absent => CapabilityState::Absent,
            CapabilityState::Cancelled => CapabilityState::Cancelled,
            _ => return Err(EncodeError::InvalidName(cap.name.clone())),
        };
        push_number(out, &state, magic)?;
    }
    for value in value_offsets.into_iter().chain(name_offsets) {
        push_i16(out, value);
    }
    out.extend_from_slice(&table);
    Ok(())
}

fn encode_bool(state: &BooleanState) -> u8 {
    match state {
        BooleanState::Set => 1,
        BooleanState::Absent | BooleanState::Cancelled => 0,
    }
}
fn standard_write_limits(entry: &Entry, options: EncodeOptions) -> (usize, usize, usize) {
    let extended = options.extended() || !entry.extended.is_empty();
    (
        entry.booleans.len().min(if extended {
            entry.booleans.len()
        } else {
            NORMAL_BOOLEAN_WRITE
        }),
        entry.numbers.len().min(if extended {
            entry.numbers.len()
        } else {
            NORMAL_NUMBER_WRITE
        }),
        entry.strings.len().min(if extended {
            entry.strings.len()
        } else {
            NORMAL_STRING_WRITE
        }),
    )
}
fn encode_string_state(
    state: &CapabilityState<Vec<u8>>,
    table: &mut Vec<u8>,
    name: &str,
) -> Result<i16, EncodeError> {
    match state {
        CapabilityState::Absent => Ok(-1),
        CapabilityState::Cancelled => Ok(-2),
        CapabilityState::Value(value) => {
            if value.contains(&0) {
                return Err(EncodeError::StringContainsNul(name.to_string()));
            }
            let offset =
                i16::try_from(table.len()).map_err(|_| EncodeError::SizeLimit("string table"))?;
            table.extend_from_slice(value);
            table.push(0);
            Ok(offset)
        }
    }
}
fn significant_len<T>(values: &[CapabilityState<T>]) -> usize {
    values
        .iter()
        .rposition(|value| !value.is_absent())
        .map_or(0, |index| index + 1)
}
fn check_size(section: &'static str, size: usize) -> Result<(), EncodeError> {
    if size > MAX_SECTION {
        Err(EncodeError::SizeLimit(section))
    } else {
        Ok(())
    }
}
fn push_count(out: &mut Vec<u8>, count: usize, section: &'static str) -> Result<(), EncodeError> {
    check_size(section, count)?;
    push_i16(out, count as i16);
    Ok(())
}
fn push_number(
    out: &mut Vec<u8>,
    state: &CapabilityState<Number>,
    magic: Magic,
) -> Result<(), EncodeError> {
    let value = match state {
        CapabilityState::Absent => -1,
        CapabilityState::Cancelled => -2,
        CapabilityState::Value(value) => i64::from(value.get()),
    };
    match magic {
        Magic::Legacy => {
            if value > i16::MAX as i64 {
                return Err(EncodeError::LegacyNumber(value as i32));
            }
            push_i16(out, value as i16);
        }
        Magic::ExtendedNumbers => out.extend_from_slice(&(value as i32).to_le_bytes()),
    }
    Ok(())
}
fn align(out: &mut Vec<u8>) {
    if !out.len().is_multiple_of(2) {
        out.push(0);
    }
}
fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_i16(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_le_bytes());
}

struct Cursor<'a> {
    data: &'a [u8],
    position: usize,
}
impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }
    fn position(&self) -> usize {
        self.position
    }
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| DecodeError::new(self.position, DecodeErrorKind::SizeLimit))?;
        let value = self
            .data
            .get(self.position..end)
            .ok_or_else(|| DecodeError::new(self.position, DecodeErrorKind::Truncated))?;
        self.position = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, DecodeError> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn i16(&mut self) -> Result<i16, DecodeError> {
        let bytes = self.take(2)?;
        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }
    fn count(&mut self) -> Result<usize, DecodeError> {
        let at = self.position;
        let value = self.i16()?;
        usize::try_from(value).map_err(|_| DecodeError::new(at, DecodeErrorKind::NegativeSize))
    }
    fn number(&mut self, magic: Magic) -> Result<i64, DecodeError> {
        match magic {
            Magic::Legacy => Ok(i64::from(self.i16()?)),
            Magic::ExtendedNumbers => {
                let bytes = self.take(4)?;
                Ok(i64::from(i32::from_le_bytes([
                    bytes[0], bytes[1], bytes[2], bytes[3],
                ])))
            }
        }
    }
    fn align_word(&mut self) -> Result<(), DecodeError> {
        if !self.position.is_multiple_of(2) {
            let offset = self.position;
            if self.byte()? != 0 {
                return Err(DecodeError::new(offset, DecodeErrorKind::TrailingData));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{BooleanCap, NumericCap, StringCap};
    use crate::model::{BooleanState, EntryBuilder};

    #[test]
    fn both_magics_round_trip_and_preserve_cancelled() {
        let mut entry = EntryBuilder::new("example")
            .unwrap()
            .alias("example-alias")
            .unwrap()
            .boolean(BooleanCap::AUTO_RIGHT_MARGIN)
            .number(NumericCap::COLUMNS, 80)
            .unwrap()
            .string(StringCap::CLEAR_SCREEN, b"\x1b[H\x1b[2J".to_vec())
            .unwrap()
            .build();
        entry.cancel_string(StringCap::BELL);
        for format in [NumberFormat::Legacy, NumberFormat::Extended] {
            let bytes = encode(&entry, EncodeOptions::new().with_number_format(format)).unwrap();
            let decoded = decode(&bytes).unwrap();
            assert_eq!(
                decoded.entry().boolean(BooleanCap::AUTO_RIGHT_MARGIN),
                BooleanState::Set
            );
            assert_eq!(
                decoded.entry().number(NumericCap::COLUMNS),
                CapabilityState::Value(Number::new(80).unwrap())
            );
            assert_eq!(
                decoded.entry().string(StringCap::BELL),
                CapabilityState::Cancelled
            );
            assert_eq!(decoded.to_bytes().unwrap(), bytes);
        }
    }

    #[test]
    fn arbitrary_input_does_not_panic() {
        for size in 0..128 {
            let _ = decode(&vec![0xa5; size]);
        }
    }

    #[test]
    fn names_section_uses_the_compiled_format_limit() {
        for section_size in [129, 154, MAX_NAMES] {
            let primary = "n".repeat(section_size - 1);
            let entry = EntryBuilder::new(primary.clone()).unwrap().build();
            let bytes = entry.to_bytes().unwrap();
            let decoded = decode(&bytes).unwrap();
            assert_eq!(decoded.entry().names().primary(), primary);
            assert_eq!(
                usize::from(u16::from_le_bytes([bytes[2], bytes[3]])),
                section_size
            );
        }

        let entry = EntryBuilder::new("n".repeat(128)).unwrap().build();
        let limited =
            DecodeOptions::new().with_limits(BinaryLimits::standard().with_max_names(128));
        assert!(decode_with(&entry.to_bytes().unwrap(), limited).is_err());
    }

    #[test]
    fn extended_values_and_types_round_trip() {
        let mut entry = EntryBuilder::new("extended").unwrap().build();
        entry.set_extended("AX", ExtendedValue::Boolean).unwrap();
        entry
            .set_extended(
                "RGB",
                ExtendedValue::Number(Number::new(0x100_0000).unwrap()),
            )
            .unwrap();
        entry
            .set_extended("Smulx", ExtendedValue::String(b"\x1b[4:%p1%dm".to_vec()))
            .unwrap();
        entry
            .set_extended("Gone", ExtendedValue::String(b"old".to_vec()))
            .unwrap();
        entry.cancel("Gone").unwrap();
        let bytes = entry.to_bytes().unwrap();
        assert_eq!(u16::from_le_bytes([bytes[0], bytes[1]]), EXTENDED_MAGIC);
        let decoded = decode(&bytes).unwrap();
        assert!(
            decoded
                .entry()
                .extended()
                .iter()
                .any(|cap| cap.name() == "AX" && cap.kind() == ExtendedKind::Boolean)
        );
        assert!(
            decoded
                .entry()
                .extended()
                .iter()
                .any(|cap| cap.name() == "RGB" && cap.kind() == ExtendedKind::Number)
        );
        assert!(
            decoded
                .entry()
                .extended()
                .iter()
                .any(|cap| cap.name() == "Smulx" && cap.kind() == ExtendedKind::String)
        );
        assert!(
            decoded
                .entry()
                .extended()
                .iter()
                .any(|cap| cap.name() == "Gone"
                    && cap.kind() == ExtendedKind::String
                    && matches!(cap.state(), CapabilityState::Cancelled))
        );
    }

    #[test]
    fn extended_header_counts_only_strings_present_in_the_table() {
        let mut entry = EntryBuilder::new("x").unwrap().build();
        entry.set_extended("AX", ExtendedValue::Boolean).unwrap();
        entry
            .set_extended("Smulx", ExtendedValue::String(b"present".to_vec()))
            .unwrap();
        entry.cancel_extended("Gone", ExtendedKind::String).unwrap();

        let bytes = entry.to_bytes().unwrap();
        // The primary section is 12-byte header + "x\\0".  In the extended
        // header at byte 14, field four counts three names and one present
        // string value; the cancelled value has an offset but no table item.
        assert_eq!(i16::from_le_bytes([bytes[20], bytes[21]]), 4);
        // ncurses stores name offsets relative to the names subtable, after
        // all present extended string values.
        assert_eq!(
            [
                i16::from_le_bytes([bytes[30], bytes[31]]),
                i16::from_le_bytes([bytes[32], bytes[33]]),
                i16::from_le_bytes([bytes[34], bytes[35]]),
            ],
            [0, 3, 8]
        );
        let decoded = decode(&bytes).unwrap();
        assert!(decoded.entry().extended().iter().any(|cap| {
            cap.name() == "Gone" && matches!(cap.state(), CapabilityState::Cancelled)
        }));
    }

    #[test]
    fn extended_capabilities_are_encoded_in_ncurses_name_order_within_each_type() {
        let mut entry = EntryBuilder::new("ordered").unwrap().build();
        entry.set_extended("Zb", ExtendedValue::Boolean).unwrap();
        entry.set_extended("Ab", ExtendedValue::Boolean).unwrap();
        entry
            .set_extended("Zn", ExtendedValue::Number(Number::new(2).unwrap()))
            .unwrap();
        entry
            .set_extended("An", ExtendedValue::Number(Number::new(1).unwrap()))
            .unwrap();
        entry
            .set_extended("Zs", ExtendedValue::String(b"z".to_vec()))
            .unwrap();
        entry
            .set_extended("As", ExtendedValue::String(b"a".to_vec()))
            .unwrap();

        let decoded = decode(&entry.to_bytes().unwrap()).unwrap();
        assert_eq!(
            decoded
                .entry()
                .extended()
                .iter()
                .map(ExtendedCapability::name)
                .collect::<Vec<_>>(),
            ["Ab", "Zb", "An", "Zn", "As", "Zs"]
        );
    }

    #[test]
    fn standard_parameter_constants_use_ncurses_binary_canonicalization() {
        let entry = EntryBuilder::new("canonical")
            .unwrap()
            .string(
                StringCap::CURSOR_ADDRESS,
                b"\x1b=%p1%{32}%+%c%p2%{0x20}%+%c".to_vec(),
            )
            .unwrap()
            .build();
        let decoded = decode(&entry.to_bytes().unwrap()).unwrap();
        assert_eq!(
            decoded.entry().string(StringCap::CURSOR_ADDRESS),
            CapabilityState::Value(b"\x1b=%p1%' '%+%c%p2%' '%+%c".as_slice())
        );
    }

    #[test]
    fn normal_and_extended_modes_use_ncurses_standard_write_ranges() {
        let mut entry = EntryBuilder::new("ranges").unwrap().build();
        let last_normal_boolean =
            BooleanCap::from_index(NORMAL_BOOLEAN_WRITE - 1).expect("normal boolean boundary");
        let first_extended_boolean =
            BooleanCap::from_index(NORMAL_BOOLEAN_WRITE).expect("extended boolean boundary");
        let last_normal_number =
            NumericCap::from_index(NORMAL_NUMBER_WRITE - 1).expect("normal number boundary");
        let first_extended_number =
            NumericCap::from_index(NORMAL_NUMBER_WRITE).expect("extended number boundary");
        let last_normal_string =
            StringCap::from_index(NORMAL_STRING_WRITE - 1).expect("normal string boundary");
        let first_extended_string =
            StringCap::from_index(NORMAL_STRING_WRITE).expect("extended string boundary");

        entry.set_boolean(last_normal_boolean);
        entry.set_boolean(first_extended_boolean);
        entry.set_number(last_normal_number, 1).unwrap();
        entry.set_number(first_extended_number, 2).unwrap();
        entry
            .set_string(last_normal_string, b"normal".to_vec())
            .unwrap();
        entry
            .set_string(first_extended_string, b"extended".to_vec())
            .unwrap();

        let normal = encode(&entry, EncodeOptions::new().with_extended(false)).unwrap();
        assert_eq!(
            [
                i16::from_le_bytes([normal[4], normal[5]]),
                i16::from_le_bytes([normal[6], normal[7]]),
                i16::from_le_bytes([normal[8], normal[9]]),
            ],
            [
                NORMAL_BOOLEAN_WRITE as i16,
                NORMAL_NUMBER_WRITE as i16,
                NORMAL_STRING_WRITE as i16,
            ]
        );
        let decoded = decode(&normal).unwrap();
        assert_eq!(
            decoded.entry().boolean(first_extended_boolean),
            BooleanState::Absent
        );
        assert!(decoded.entry().number(first_extended_number).is_absent());
        assert!(decoded.entry().string(first_extended_string).is_absent());

        let extended = encode(&entry, EncodeOptions::new().with_extended(true)).unwrap();
        assert_eq!(
            [
                i16::from_le_bytes([extended[4], extended[5]]),
                i16::from_le_bytes([extended[6], extended[7]]),
                i16::from_le_bytes([extended[8], extended[9]]),
            ],
            [
                (NORMAL_BOOLEAN_WRITE + 1) as i16,
                (NORMAL_NUMBER_WRITE + 1) as i16,
                (NORMAL_STRING_WRITE + 1) as i16,
            ]
        );
    }

    #[test]
    fn standard_boolean_cancels_are_written_as_false_and_do_not_extend_the_section() {
        let mut entry = EntryBuilder::new("bool-cancel").unwrap().build();
        let cancelled = BooleanCap::from_index(1).unwrap();
        let final_set = BooleanCap::from_index(2).unwrap();
        let later_cancelled = BooleanCap::from_index(8).unwrap();
        entry.cancel_boolean(cancelled);
        entry.set_boolean(final_set);
        entry.cancel_boolean(later_cancelled);

        let bytes = encode(&entry, EncodeOptions::new().with_extended(false)).unwrap();
        assert_eq!(i16::from_le_bytes([bytes[4], bytes[5]]), 3);
        let names_length = usize::from(u16::from_le_bytes([bytes[2], bytes[3]]));
        let booleans = &bytes[12 + names_length..12 + names_length + 3];
        assert_eq!(booleans, [0, 0, 1]);
        assert_eq!(
            decode(&bytes).unwrap().entry().boolean(cancelled),
            BooleanState::Absent
        );
    }

    #[test]
    fn binary_document_preserves_then_reencodes_after_edit() {
        let entry = EntryBuilder::new("representation").unwrap().build();
        let original = encode(
            &entry,
            EncodeOptions::new().with_number_format(NumberFormat::Extended),
        )
        .unwrap();
        let mut document = decode(&original).unwrap();
        assert_eq!(document.to_bytes().unwrap(), original);
        document
            .entry_mut()
            .set_number(NumericCap::COLUMNS, 132)
            .unwrap();
        let changed = document.to_bytes().unwrap();
        assert_ne!(changed, original);
        assert_eq!(
            decode(&changed)
                .unwrap()
                .entry()
                .number(NumericCap::COLUMNS),
            CapabilityState::Value(Number::new(132).unwrap())
        );
    }

    #[test]
    fn binary_document_explicit_options_reencode_even_when_unedited() {
        let mut entry = EntryBuilder::new("explicit-options").unwrap().build();
        let last_normal_string =
            StringCap::from_index(NORMAL_STRING_WRITE - 1).expect("normal string boundary");
        let extended_string =
            StringCap::from_index(NORMAL_STRING_WRITE).expect("extended string boundary");
        entry
            .set_string(last_normal_string, b"normal".to_vec())
            .unwrap();
        entry
            .set_string(extended_string, b"obsolete".to_vec())
            .unwrap();
        let original = encode(&entry, EncodeOptions::new().with_extended(true)).unwrap();
        assert_eq!(
            i16::from_le_bytes([original[8], original[9]]),
            (NORMAL_STRING_WRITE + 1) as i16
        );

        let document = decode(&original).unwrap();
        assert_eq!(document.to_bytes().unwrap(), original);
        let normal = document
            .to_bytes_with(EncodeOptions::new().with_extended(false))
            .unwrap();
        assert_eq!(
            i16::from_le_bytes([normal[8], normal[9]]),
            NORMAL_STRING_WRITE as i16
        );
        assert_ne!(normal, original);
    }
}
