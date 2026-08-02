//! Parsing of unresolved terminfo source entries.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{BuildError, Diagnostic, ParseError, Span};
pub use crate::error::{SourceId, TextRange};
use crate::model::EntryNames;

const DEFAULT_MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_ENTRIES: usize = 16_384;

/// Limits applied while tokenizing untrusted source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLimits {
    max_bytes: usize,
    max_entries: usize,
}

impl SourceLimits {
    /// Returns defensive limits for ordinary source files.
    pub const fn standard() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_SOURCE_BYTES,
            max_entries: DEFAULT_MAX_ENTRIES,
        }
    }

    /// Returns limits bounded only by address-space size.
    pub const fn unlimited() -> Self {
        Self {
            max_bytes: usize::MAX,
            max_entries: usize::MAX,
        }
    }

    /// Returns the maximum source byte length.
    pub const fn max_bytes(self) -> usize {
        self.max_bytes
    }

    /// Returns the maximum logical entry count.
    pub const fn max_entries(self) -> usize {
        self.max_entries
    }

    /// Replaces the source byte limit.
    pub const fn with_max_bytes(mut self, value: usize) -> Self {
        self.max_bytes = value;
        self
    }

    /// Replaces the logical entry-count limit.
    pub const fn with_max_entries(mut self, value: usize) -> Self {
        self.max_entries = value;
        self
    }
}

impl Default for SourceLimits {
    fn default() -> Self {
        Self::standard()
    }
}

/// Options for parsing a source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct ParseOptions {
    source_id: SourceId,
    limits: SourceLimits,
}

impl ParseOptions {
    /// Creates options with source id zero and standard limits.
    pub const fn new() -> Self {
        Self {
            source_id: SourceId::new(0),
            limits: SourceLimits::standard(),
        }
    }

    /// Returns the source identity embedded in produced spans.
    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    /// Returns tokenizer resource limits.
    pub const fn limits(self) -> SourceLimits {
        self.limits
    }

    /// Replaces the source identity.
    pub const fn with_source_id(mut self, value: SourceId) -> Self {
        self.source_id = value;
        self
    }

    /// Replaces tokenizer resource limits.
    pub const fn with_limits(mut self, value: SourceLimits) -> Self {
        self.limits = value;
        self
    }
}

/// One unresolved source entry with lossless names and capability tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceEntry {
    pub(crate) names: Vec<String>,
    pub(crate) capabilities: Vec<SourceCapability>,
    pub(crate) span: Span,
}

impl SourceEntry {
    /// Returns the first raw names field.
    pub fn primary_name(&self) -> &str {
        self.names.first().map_or("", String::as_str)
    }
    /// Reports whether a primary name or lookup alias matches.
    pub fn matches_name(&self, name: &str) -> bool {
        self.entry_names().is_ok_and(|names| {
            names.primary() == name || names.aliases().iter().any(|alias| alias == name)
        })
    }

    /// Returns the names interpreted according to terminfo's
    /// primary/alias/verbose-name rules.
    ///
    /// [Self::names] exposes the lossless sequence of fields as written,
    /// while this method exposes only the primary name and aliases as lookup
    /// targets. A final field without whitespace is both an alias and the
    /// verbose name, matching ncurses.
    pub fn entry_names(&self) -> Result<EntryNames, BuildError> {
        EntryNames::from_source_fields(&self.names)
    }

    /// Returns raw names fields in source order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// Returns parsed capability tokens in source order.
    pub fn capabilities(&self) -> &[SourceCapability] {
        &self.capabilities
    }

    /// Returns the full entry span.
    pub const fn span(&self) -> Span {
        self.span
    }
}

/// A parsed capability token and its source metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCapability {
    pub(crate) value: Capability,
    pub(crate) span: Span,
    pub(crate) commented: bool,
}

impl SourceCapability {
    /// Returns the parsed logical capability.
    pub fn value(&self) -> &Capability {
        &self.value
    }

    /// Whether the capability was disabled with terminfo's leading `.` syntax.
    pub const fn is_commented(&self) -> bool {
        self.commented
    }

    /// Returns the physical token span.
    pub const fn span(&self) -> Span {
        self.span
    }
}

/// Unresolved terminfo capability syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Capability {
    /// A present boolean capability.
    Boolean {
        /// Capability name.
        name: String,
    },
    /// A numeric capability and parsed integer.
    Numeric {
        /// Capability name.
        name: String,
        /// Parsed signed source value.
        value: i32,
    },
    /// A binary-safe decoded string capability.
    String {
        /// Capability name.
        name: String,
        /// Escape-decoded bytes.
        value: Vec<u8>,
    },
    /// A capability cancellation.
    Cancel {
        /// Capability name.
        name: String,
    },
    /// An unresolved inheritance edge.
    Use {
        /// Referenced primary name or alias.
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceEdit {
    range: TextRange,
    replacement: Vec<u8>,
}

/// A lossless source syntax document plus its parsed logical tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceDocument {
    source_id: SourceId,
    original: Vec<u8>,
    entries: Vec<SourceEntry>,
    line_starts: Vec<usize>,
    edits: Vec<SourceEdit>,
}

impl SourceDocument {
    /// Returns the identity used by every contained span.
    pub const fn source_id(&self) -> SourceId {
        self.source_id
    }

    /// Returns logical entries in document order.
    pub fn entries(&self) -> &[SourceEntry] {
        &self.entries
    }

    /// Returns the untouched source buffer.
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// Converts a byte offset to one-based line and column numbers.
    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let line = self.line_starts.partition_point(|start| *start <= offset);
        let line_index = line.saturating_sub(1);
        let start = self.line_starts.get(line_index).copied().unwrap_or(0);
        (line_index + 1, offset.saturating_sub(start) + 1)
    }

    /// Starts a lossless token editor.
    pub fn edit(&mut self) -> SourceEditor<'_> {
        SourceEditor { document: self }
    }

    /// Emit the original document with only explicitly edited tokens replaced.
    pub fn to_bytes_preserve(&self) -> Vec<u8> {
        let mut output = self.original.clone();
        let mut edits = self.edits.clone();
        edits.sort_by_key(|edit| core::cmp::Reverse(edit.range.start()));
        for edit in edits {
            output.splice(
                edit.range.start()..edit.range.end(),
                edit.replacement.iter().copied(),
            );
        }
        output
    }

    /// Emit deterministic source, intentionally dropping comments and spacing.
    pub fn to_bytes_canonical(&self) -> Vec<u8> {
        let mut output = String::new();
        for entry in &self.entries {
            output.push_str(&entry.names.join("|"));
            output.push_str(",\n");
            for capability in &entry.capabilities {
                output.push('\t');
                if capability.commented {
                    output.push('.');
                }
                output.push_str(&render_capability(&capability.value));
                output.push_str(",\n");
            }
        }
        output.into_bytes()
    }

    /// Emit deterministic unresolved source wrapped to the requested width.
    /// Unlike compilation this preserves `use=` fields as inheritance edges.
    pub fn to_bytes_canonical_with_width(&self, width: usize) -> Vec<u8> {
        let mut output = String::new();
        for entry in &self.entries {
            let capabilities: Vec<String> = entry
                .capabilities
                .iter()
                .map(|capability| {
                    let rendered = render_capability(&capability.value);
                    if capability.commented {
                        alloc::format!(".{rendered}")
                    } else {
                        rendered
                    }
                })
                .collect();
            output.push_str(&crate::format::render(
                entry.names.join("|"),
                &capabilities,
                crate::format::Layout::Wrapped { width },
            ));
        }
        output.into_bytes()
    }
}

/// A checked token editor which never rewrites unrelated source text.
pub struct SourceEditor<'a> {
    document: &'a mut SourceDocument,
}

impl SourceEditor<'_> {
    /// Replaces one parsed capability while preserving surrounding bytes.
    pub fn replace_capability(
        &mut self,
        entry_index: usize,
        capability_index: usize,
        replacement: Capability,
    ) -> Result<(), SourceEditError> {
        let capability = self
            .document
            .entries
            .get_mut(entry_index)
            .and_then(|entry| entry.capabilities.get_mut(capability_index))
            .ok_or(SourceEditError::UnknownToken)?;
        let range = capability.span.range();
        if self
            .document
            .edits
            .iter()
            .any(|edit| ranges_overlap(edit.range, range))
        {
            return Err(SourceEditError::OverlappingEdit);
        }
        let mut rendered = render_capability(&replacement);
        if capability.commented {
            rendered.insert(0, '.');
        }
        let rendered = rendered.into_bytes();
        capability.value = replacement;
        self.document.edits.push(SourceEdit {
            range,
            replacement: rendered,
        });
        Ok(())
    }
}

/// Failure while applying a lossless source edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SourceEditError {
    /// The requested entry or capability index does not exist.
    UnknownToken,
    /// The new edit overlaps an earlier pending edit.
    OverlappingEdit,
}

fn ranges_overlap(left: TextRange, right: TextRange) -> bool {
    left.start() < right.end() && right.start() < left.end()
}

fn render_capability(capability: &Capability) -> String {
    match capability {
        Capability::Boolean { name } => name.clone(),
        Capability::Numeric { name, value } => alloc::format!("{name}#{value}"),
        Capability::String { name, value } => {
            alloc::format!("{name}={}", crate::format::escape(value))
        }
        Capability::Cancel { name } => alloc::format!("{name}@"),
        Capability::Use { name } => alloc::format!("use={name}"),
    }
}

/// Parse all entries without resolving inheritance references.
pub fn parse(source: &[u8]) -> Result<SourceDocument, ParseError> {
    parse_with(source, ParseOptions::default())
}

/// Parses a source buffer with explicit identity and resource limits.
pub fn parse_with(source: &[u8], options: ParseOptions) -> Result<SourceDocument, ParseError> {
    if source.len() > options.limits().max_bytes() {
        return Err(error(
            "TIK1008",
            "source exceeds configured byte limit",
            Span::default(),
        ));
    }
    let entries = parse_bytes(source, options.source_id())?;
    if entries.len() > options.limits().max_entries() {
        return Err(error(
            "TIK1010",
            "source exceeds configured entry limit",
            Span::default(),
        ));
    }
    let mut line_starts = vec![0];
    line_starts.extend(
        source
            .iter()
            .enumerate()
            .filter_map(|(index, byte)| (*byte == b'\n').then_some(index + 1)),
    );
    Ok(SourceDocument {
        source_id: options.source_id(),
        original: source.to_vec(),
        entries,
        line_starts,
        edits: Vec::new(),
    })
}

fn parse_bytes(source: &[u8], source_id: SourceId) -> Result<Vec<SourceEntry>, ParseError> {
    let records = logical_records(source, source_id);
    let mut entries = Vec::with_capacity(records.len());
    for record in &records {
        entries.push(parse_record(record)?);
    }
    if entries.is_empty() && !source.trim_ascii().is_empty() {
        return Err(error(
            "TIK1001",
            "source contains no terminal entries",
            Span::new(source_id, TextRange::new(0, 0)),
        ));
    }
    Ok(entries)
}

#[derive(Debug)]
struct LogicalSegment {
    logical: TextRange,
    physical: TextRange,
}

#[derive(Debug)]
struct LogicalRecord {
    text: Vec<u8>,
    span: Span,
    segments: Vec<LogicalSegment>,
}

impl LogicalRecord {
    fn physical_start(&self, logical: usize) -> usize {
        self.segments
            .iter()
            .find(|segment| segment.logical.start() <= logical && logical < segment.logical.end())
            .map_or(self.span.range().start(), |segment| {
                segment
                    .physical
                    .start()
                    .saturating_add(logical.saturating_sub(segment.logical.start()))
            })
    }

    fn physical_end(&self, logical: usize) -> usize {
        self.segments
            .iter()
            .rev()
            .find(|segment| segment.logical.start() < logical && logical <= segment.logical.end())
            .map_or(self.span.range().end(), |segment| {
                segment
                    .physical
                    .start()
                    .saturating_add(logical.saturating_sub(segment.logical.start()))
            })
    }
}

fn logical_records(source: &[u8], source_id: SourceId) -> Vec<LogicalRecord> {
    let mut records = Vec::new();
    let mut text = Vec::new();
    let mut segments = Vec::new();
    let mut offset: usize = 0;
    for raw in source.split_inclusive(|byte| *byte == b'\n') {
        let line = raw.strip_suffix(b"\n").unwrap_or(raw);
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let trimmed = line.trim_ascii();
        let comment = trimmed.starts_with(b"#");
        let starts_entry = !line.is_empty() && !line[0].is_ascii_whitespace() && !comment;
        if starts_entry && !text.is_empty() {
            records.push(finish_record(
                core::mem::take(&mut text),
                core::mem::take(&mut segments),
                source_id,
            ));
        }
        if !comment && !trimmed.is_empty() {
            let mut piece = trimmed;
            if ends_with_continuation(piece) {
                piece = &piece[..piece.len() - 1];
            }
            let logical_start = text.len();
            let leading = line.len().saturating_sub(line.trim_ascii_start().len());
            let physical_start = offset.saturating_add(leading);
            text.extend_from_slice(piece);
            segments.push(LogicalSegment {
                logical: TextRange::new(logical_start, text.len()),
                physical: TextRange::new(
                    physical_start,
                    physical_start.saturating_add(piece.len()),
                ),
            });
        }
        offset += raw.len();
    }
    if !text.is_empty() {
        records.push(finish_record(text, segments, source_id));
    }
    records
}

fn finish_record(
    text: Vec<u8>,
    segments: Vec<LogicalSegment>,
    source_id: SourceId,
) -> LogicalRecord {
    let start = segments
        .first()
        .map_or(0, |segment| segment.physical.start());
    let end = segments
        .last()
        .map_or(start, |segment| segment.physical.end());
    LogicalRecord {
        text,
        span: Span::new(source_id, TextRange::new(start, end)),
        segments,
    }
}

fn ends_with_continuation(value: &[u8]) -> bool {
    let mut index = 0;
    while index < value.len() {
        if index + 1 == value.len() {
            return value[index] == b'\\';
        }
        index += if is_escape_introducer(value, index) {
            2
        } else {
            1
        };
    }
    false
}

fn parse_record(record: &LogicalRecord) -> Result<SourceEntry, ParseError> {
    let fields = split_unescaped(&record.text, b',');
    let names_field = fields.first().map_or(&[][..], |field| field.trim_ascii());
    let mut names = Vec::new();
    for bytes in names_field.split(|byte| *byte == b'|') {
        let bytes = bytes.trim_ascii();
        if bytes.is_empty() {
            continue;
        }
        let name = core::str::from_utf8(bytes)
            .map_err(|_| error("TIK1009", "terminal names must be valid UTF-8", record.span))?;
        names.push(String::from(name));
    }
    if names.is_empty() {
        return Err(error("TIK1002", "entry has no primary name", record.span));
    }
    let mut capabilities = Vec::new();
    let mut search_from = names_field.len().saturating_add(1);
    for raw in fields.iter().skip(1) {
        let field = raw.trim_ascii();
        if field.is_empty() {
            continue;
        }
        let relative = record
            .text
            .get(search_from..)
            .and_then(|tail| find_bytes(tail, field))
            .map_or(search_from, |value| search_from + value);
        let cap_span = Span::new(
            record.span.source_id(),
            TextRange::new(
                record.physical_start(relative),
                record.physical_end(relative.saturating_add(field.len())),
            ),
        );
        search_from = relative.saturating_add(field.len()).saturating_add(1);
        let (commented, field) = field
            .strip_prefix(b".")
            .map_or((false, field), |field| (true, field));
        let value = parse_capability(field, cap_span)?;
        capabilities.push(SourceCapability {
            value,
            span: cap_span,
            commented,
        });
    }
    Ok(SourceEntry {
        names,
        capabilities,
        span: record.span,
    })
}

fn parse_capability(field: &[u8], span: Span) -> Result<Capability, ParseError> {
    // The first non-escaped value separator determines the capability kind.
    // In particular, an '@' at the end of a string value such as ich1=\E[@
    // is data rather than a cancellation marker.
    if let Some((index, separator)) = find_first_unescaped(field, b"#=") {
        let name = validate_cap_name(&field[..index], span)?;
        let literal = &field[index + 1..];
        if separator == b'#' {
            let value = parse_number(literal)
                .ok_or_else(|| error("TIK1004", "invalid numeric capability value", span))?;
            return Ok(Capability::Numeric {
                name: String::from(name),
                value,
            });
        }
        if name == "use" {
            let name = validate_name(literal, span)?;
            return Ok(Capability::Use {
                name: String::from(name),
            });
        }
        let value = decode_escapes(literal, span)?;
        return Ok(Capability::String {
            name: String::from(name),
            value,
        });
    }
    if let Some(name) = field.strip_suffix(b"@") {
        let name = validate_cap_name(name, span)?;
        return Ok(Capability::Cancel {
            name: String::from(name),
        });
    }
    let name = validate_cap_name(field, span)?;
    Ok(Capability::Boolean {
        name: String::from(name),
    })
}

fn parse_number(value: &[u8]) -> Option<i32> {
    let value = core::str::from_utf8(value).ok()?;
    let (negative, digits) = value
        .strip_prefix('-')
        .map_or((false, value), |rest| (true, rest));
    let (radix, digits) = if let Some(rest) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, rest)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (8, &digits[1..])
    } else {
        (10, digits)
    };
    if digits.is_empty() {
        return if radix == 8 { Some(0) } else { None };
    }
    let number = i64::from_str_radix(digits, radix).ok()?;
    let number = if negative { -number } else { number };
    i32::try_from(number).ok()
}

fn decode_escapes(input: &[u8], span: Span) -> Result<Vec<u8>, ParseError> {
    let mut output = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'^' if output.last() != Some(&b'%') => {
                index += 1;
                let Some(&byte) = input.get(index) else {
                    return Err(error("TIK1005", "trailing caret escape", span));
                };
                output.push(if byte == b'?' { 0x7f } else { byte & 0x1f });
                index += 1;
            }
            b'\\' => {
                index += 1;
                let Some(&escaped) = input.get(index) else {
                    return Err(error("TIK1006", "trailing backslash escape", span));
                };
                match escaped {
                    b'E' | b'e' => {
                        output.push(0x1b);
                        index += 1;
                    }
                    b'n' | b'l' => {
                        output.push(b'\n');
                        index += 1;
                    }
                    b'r' => {
                        output.push(b'\r');
                        index += 1;
                    }
                    b't' => {
                        output.push(b'\t');
                        index += 1;
                    }
                    b'b' => {
                        output.push(0x08);
                        index += 1;
                    }
                    b'f' => {
                        output.push(0x0c);
                        index += 1;
                    }
                    b's' => {
                        output.push(b' ');
                        index += 1;
                    }
                    b',' | b':' | b'^' | b'\\' => {
                        output.push(escaped);
                        index += 1;
                    }
                    b'0'..=b'7' => {
                        let mut number = 0u16;
                        let mut count = 0;
                        while count < 3
                            && index < input.len()
                            && matches!(input[index], b'0'..=b'7')
                        {
                            number = number * 8 + u16::from(input[index] - b'0');
                            index += 1;
                            count += 1;
                        }
                        output.push(if number == 0 {
                            0x80
                        } else {
                            (number & 0xff) as u8
                        });
                    }
                    _ => {
                        output.push(escaped);
                        index += 1;
                    }
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    Ok(output)
}

fn split_unescaped(value: &[u8], separator: u8) -> Vec<&[u8]> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < value.len() {
        if value[index] == separator {
            result.push(&value[start..index]);
            start = index + 1;
        }
        index += if is_escape_introducer(value, index) {
            2
        } else {
            1
        };
    }
    result.push(&value[start..]);
    result
}
fn find_first_unescaped(value: &[u8], needles: &[u8]) -> Option<(usize, u8)> {
    let mut index = 0;
    while index < value.len() {
        let byte = value[index];
        if needles.contains(&byte) {
            return Some((index, byte));
        }
        index += if is_escape_introducer(value, index) {
            2
        } else {
            1
        };
    }
    None
}

fn is_escape_introducer(value: &[u8], index: usize) -> bool {
    value[index] == b'\\' || (value[index] == b'^' && (index == 0 || value[index - 1] != b'%'))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        Some(0)
    } else {
        haystack
            .windows(needle.len())
            .position(|window| window == needle)
    }
}
fn validate_name(name: &[u8], span: Span) -> Result<&str, ParseError> {
    if name.is_empty()
        || name
            .iter()
            .copied()
            .any(|b| b.is_ascii_whitespace() || matches!(b, b'/' | b'\\' | b',' | b'|'))
    {
        Err(error("TIK1007", "invalid use= name", span))
    } else {
        core::str::from_utf8(name)
            .map_err(|_| error("TIK1007", "use= name is not valid UTF-8", span))
    }
}
fn validate_cap_name(name: &[u8], span: Span) -> Result<&str, ParseError> {
    if name.is_empty()
        || name
            .iter()
            .copied()
            .any(|b| b.is_ascii_whitespace() || matches!(b, b',' | b'=' | b'#' | b'@'))
    {
        Err(error("TIK1003", "invalid capability name", span))
    } else {
        core::str::from_utf8(name)
            .map_err(|_| error("TIK1003", "capability name is not valid UTF-8", span))
    }
}
fn error(code: &'static str, message: &'static str, span: Span) -> ParseError {
    ParseError {
        diagnostic: Box::new(Diagnostic::error(code, message, Some(span))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_escapes_numbers_and_comments() {
        let input = "# comment\nbase|base terminal, am, cols#0x50, pairs#010, cup=\\E[%i%p1%d\\,%p2%dH,\n\tuse=ansi, clear@,\n";
        let document = parse(input.as_bytes()).unwrap();
        let entries = document.entries();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].names, ["base", "base terminal"]);
        assert!(matches!(
            &entries[0].capabilities[2].value,
            Capability::Numeric { value: 8, .. }
        ));
        assert!(
            matches!(&entries[0].capabilities[3].value, Capability::String { value, .. } if value.starts_with(b"\x1b["))
        );
    }

    #[test]
    fn separators_precede_cancel_and_string_payload_is_preserved() {
        let document = parse(b"demo,ich1=\\E[@,note=left#right,\n").unwrap();
        let capabilities = document.entries()[0].capabilities();
        assert!(matches!(
            capabilities[0].value(),
            Capability::String { name, value } if name == "ich1" && value == b"\x1b[@"
        ));
        assert!(matches!(
            capabilities[1].value(),
            Capability::String { name, value } if name == "note" && value == b"left#right"
        ));
    }

    #[test]
    fn forward_entries_are_split_without_blank_lines() {
        let document = parse(b"a,am,\nb,cols#80,\n").unwrap();
        let entries = document.entries();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn lossless_edit_changes_only_a_continued_token() {
        let input =
            b"# heading\r\nsample|sample terminal,\r\n\tam, cols#80, \\\r\n\tclear=\\E[H,\r\n# tail\r\n";
        let mut document = parse(input).unwrap();
        assert_eq!(document.to_bytes_preserve(), input);
        document
            .edit()
            .replace_capability(
                0,
                1,
                Capability::Numeric {
                    name: "cols".into(),
                    value: 100,
                },
            )
            .unwrap();
        let expected =
            b"# heading\r\nsample|sample terminal,\r\n\tam, cols#100, \\\r\n\tclear=\\E[H,\r\n# tail\r\n";
        assert_eq!(document.to_bytes_preserve(), expected);
        assert_eq!(document.line_column(45), (3, 10));
    }

    #[test]
    fn zero_escape_is_promoted_to_non_nul_byte() {
        let document = parse(b"zero,test=\\0,\n").unwrap();
        assert!(matches!(
            document.entries()[0].capabilities()[0].value(),
            Capability::String { value, .. } if value == &[0x80]
        ));
    }

    #[test]
    fn preserves_non_utf8_comments_and_string_bytes() {
        let input = b"# arbitrary byte: \xff\nraw,foo=\xff,\n";
        let document = parse(input).unwrap();
        assert_eq!(document.to_bytes_preserve(), input);
        assert!(matches!(
            document.entries()[0].capabilities()[0].value(),
            Capability::String { value, .. } if value == b"\xff"
        ));
        assert!(
            document
                .to_bytes_canonical()
                .windows(4)
                .any(|window| window == b"\\377")
        );
    }

    #[test]
    fn caret_backslash_does_not_escape_the_following_comma() {
        let document = parse(b"demo,cuf1=^\\,cuu1=^^,escaped=left\\,right,\n").unwrap();
        let capabilities = document.entries()[0].capabilities();
        assert_eq!(capabilities.len(), 3);
        assert!(matches!(
            capabilities[0].value(),
            Capability::String { name, value }
                if name == "cuf1" && value == &[0x1c]
        ));
        assert!(matches!(
            capabilities[1].value(),
            Capability::String { name, value }
                if name == "cuu1" && value == &[0x1e]
        ));
        assert!(matches!(
            capabilities[2].value(),
            Capability::String { name, value }
                if name == "escaped" && value == b"left,right"
        ));
    }

    #[test]
    fn caret_after_percent_is_preserved_as_a_parameter_operator() {
        let document = parse(b"demo,cup=%p1%{96}%^%c,kf21=^B%^M,\n").unwrap();
        let capabilities = document.entries()[0].capabilities();
        assert!(matches!(
            capabilities[0].value(),
            Capability::String { name, value }
                if name == "cup" && value == b"%p1%{96}%^%c"
        ));
        assert!(matches!(
            capabilities[1].value(),
            Capability::String { name, value }
                if name == "kf21" && value == b"\x02%^M"
        ));

        let delimited = parse(b"demo,first=%^,second=value,\n").unwrap();
        assert_eq!(delimited.entries()[0].capabilities().len(), 2);
    }

    #[test]
    fn commented_capabilities_are_logical_tokens_and_round_trip() {
        let input = b"sample,.am,.cols#132,.use=base,\nbase,cols#80,\n";
        let document = parse(input).unwrap();
        let capabilities = document.entries()[0].capabilities();
        assert!(capabilities.iter().all(SourceCapability::is_commented));
        assert!(matches!(
            capabilities[0].value(),
            Capability::Boolean { name } if name == "am"
        ));
        assert_eq!(document.to_bytes_preserve(), input);
        let canonical = document.to_bytes_canonical();
        assert!(canonical.windows(4).any(|value| value == b".am,"));
        assert!(canonical.windows(10).any(|value| value == b".use=base,"));
    }
}
