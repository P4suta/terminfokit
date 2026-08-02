// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Termcap parsing, writing, inheritance, and conversion.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::caps::{BooleanCap, NumericCap, StringCap};
use crate::error::{ConvertError, Diagnostic, ParseError, Span};
use crate::format::escape;
use crate::model::{BooleanState, CapabilityState, Entry};
use crate::resolve::{Compilation, Compiler, CompilerOptions, EntryProvider};

/// Loss and compatibility policy for terminfo-to-termcap conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TermcapProfile {
    /// Reject every capability or expression that cannot round-trip.
    Strict,
    /// Follow ncurses 6.6 lossy conversion behavior.
    Ncurses66,
    /// Use historical BSD termcap compatibility.
    Bsd,
}

/// Rendering and loss policy for termcap output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ConvertOptions {
    profile: TermcapProfile,
    max_entry_bytes: Option<usize>,
    include_extended_comments: bool,
    width: usize,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            profile: TermcapProfile::Strict,
            max_entry_bytes: Some(1023),
            include_extended_comments: false,
            width: 76,
        }
    }
}
impl ConvertOptions {
    /// Creates a lossless conversion profile.
    pub const fn strict() -> Self {
        Self {
            profile: TermcapProfile::Strict,
            max_entry_bytes: Some(1023),
            include_extended_comments: false,
            width: 76,
        }
    }

    /// Creates the ncurses 6.6 compatibility profile.
    pub const fn ncurses_6_6() -> Self {
        Self {
            profile: TermcapProfile::Ncurses66,
            max_entry_bytes: Some(1023),
            include_extended_comments: true,
            width: 76,
        }
    }

    /// Creates the conservative BSD compatibility profile.
    pub const fn bsd() -> Self {
        Self {
            profile: TermcapProfile::Bsd,
            max_entry_bytes: Some(1023),
            include_extended_comments: true,
            width: 76,
        }
    }

    /// Returns the selected compatibility profile.
    pub const fn profile(self) -> TermcapProfile {
        self.profile
    }

    /// Removes the historical rendered entry-size limit.
    pub const fn unlimited(mut self) -> Self {
        self.max_entry_bytes = None;
        self
    }

    /// Returns the wrapping width.
    pub const fn width(self) -> usize {
        self.width
    }

    /// Replaces the wrapping width.
    pub const fn with_width(mut self, width: usize) -> Self {
        self.width = width;
        self
    }
}

/// A lossy termcap conversion warning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionWarning {
    capability: String,
    message: String,
}

impl ConversionWarning {
    /// Returns the affected capability name.
    pub fn capability(&self) -> &str {
        &self.capability
    }

    /// Returns the explanation of information loss.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Rendered termcap source plus loss warnings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermcapConversion {
    source: String,
    warnings: Vec<ConversionWarning>,
}
impl TermcapConversion {
    /// Returns rendered termcap source.
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Returns lossy conversion warnings.
    pub fn warnings(&self) -> &[ConversionWarning] {
        &self.warnings
    }
}

/// Termcap syntax and its unresolved terminfo document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TermcapDocument {
    original: Vec<u8>,
    source: crate::source::SourceDocument,
}

impl TermcapDocument {
    /// Returns the untouched termcap input.
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// Returns the original termcap bytes.
    pub fn to_bytes_preserve(&self) -> Vec<u8> {
        self.original.clone()
    }

    /// Returns the translated unresolved terminfo document.
    pub fn source_document(&self) -> &crate::source::SourceDocument {
        &self.source
    }

    /// Renders translated terminfo with the default width.
    pub fn to_terminfo_source(&self) -> Vec<u8> {
        self.source.to_bytes_canonical()
    }

    /// Renders translated terminfo with an explicit wrapping width.
    pub fn to_terminfo_source_with_width(&self, width: usize) -> Vec<u8> {
        self.source.to_bytes_canonical_with_width(width)
    }
}

/// Parses binary-safe termcap source and translates it to terminfo syntax.
pub fn parse(termcap: &[u8]) -> Result<TermcapDocument, ParseError> {
    let converted = convert_to_terminfo_source(termcap)?;
    let source = crate::source::parse(converted.as_bytes())?;
    Ok(TermcapDocument {
        original: termcap.to_vec(),
        source,
    })
}

fn convert_to_terminfo_source(termcap: &[u8]) -> Result<String, ParseError> {
    let mut output = String::new();
    for (record, line) in records(termcap) {
        let fields = split_unescaped(&record, b':');
        let Some(names) = fields.first() else {
            continue;
        };
        let names = core::str::from_utf8(names.trim_ascii())
            .map_err(|_| parse_error(line, "termcap names are not valid UTF-8"))?;
        output.push_str(names);
        output.push_str(",\n");
        for field in fields
            .iter()
            .skip(1)
            .map(|field| field.trim_ascii())
            .filter(|field| !field.is_empty())
        {
            output.push('\t');
            output.push_str(&convert_field(field, line)?);
            output.push_str(",\n");
        }
    }
    Ok(output)
}

/// Resolves a parsed termcap document through the terminfo compiler.
pub fn compile(
    document: &TermcapDocument,
    options: CompilerOptions,
    provider: Option<&dyn EntryProvider>,
) -> Result<Compilation, ConvertError> {
    let compiler = if let Some(provider) = provider {
        Compiler::new().options(options).provider(provider)
    } else {
        Compiler::new().options(options)
    };
    let resolution = compiler
        .resolve(document.source_document())
        .map_err(ConvertError::Compile)?;
    compiler.encode(&resolution).map_err(ConvertError::Compile)
}

/// Converts one resolved entry to termcap under an explicit loss profile.
pub fn from_entry(
    entry: &Entry,
    options: ConvertOptions,
) -> Result<TermcapConversion, ConvertError> {
    let mut fields = Vec::new();
    let mut warnings = Vec::new();
    for cap in BooleanCap::ALL {
        match entry.boolean(*cap) {
            BooleanState::Absent => {}
            state => match cap.termcap_name() {
                Some(name) => fields.push(if matches!(state, BooleanState::Cancelled) {
                    format!("{name}@")
                } else {
                    name.into()
                }),
                None => loss(cap.short_name(), &mut warnings, options)?,
            },
        }
    }
    for cap in NumericCap::ALL {
        match entry.number(*cap) {
            CapabilityState::Absent => {}
            state => match cap.termcap_name() {
                Some(name) => fields.push(match state {
                    CapabilityState::Cancelled => format!("{name}@"),
                    CapabilityState::Value(value) => format!("{name}#{value}"),
                    CapabilityState::Absent => unreachable!(),
                }),
                None => loss(cap.short_name(), &mut warnings, options)?,
            },
        }
    }
    for cap in StringCap::ALL {
        match entry.string(*cap) {
            CapabilityState::Absent => {}
            state => match cap.termcap_name() {
                Some(name) => match state {
                    CapabilityState::Cancelled => fields.push(format!("{name}@")),
                    CapabilityState::Value(value) => match terminfo_expression_to_termcap(value) {
                        Some(value) => fields.push(format!("{name}={}", escape_termcap(&value))),
                        None => expression_loss(cap.short_name(), &mut warnings, options)?,
                    },
                    CapabilityState::Absent => unreachable!(),
                },
                None => loss(cap.short_name(), &mut warnings, options)?,
            },
        }
    }
    for cap in entry.extended() {
        if !cap.state().is_absent() {
            loss(cap.name(), &mut warnings, options)?;
        }
    }

    let mut source = entry.names().source_fields().join("|");
    source.push_str(":\\\n\t:");
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            source.push(':');
        }
        if source.lines().last().map_or(0, str::len) + field.len() + 2 > options.width.max(1) {
            source.push_str("\\\n\t:");
        }
        source.push_str(field);
    }
    source.push_str(":\n");
    if options.include_extended_comments {
        for warning in &warnings {
            source.push_str(&format!(
                "# terminfokit: {} ({})\n",
                warning.message, warning.capability
            ));
        }
    }
    if let Some(limit) = options.max_entry_bytes {
        let length = source.replace("\\\n\t", "").len();
        if length > limit {
            return Err(ConvertError::EntryTooLong { length, limit });
        }
    }
    Ok(TermcapConversion { source, warnings })
}

fn loss(
    name: &str,
    warnings: &mut Vec<ConversionWarning>,
    options: ConvertOptions,
) -> Result<(), ConvertError> {
    if options.profile == TermcapProfile::Strict {
        Err(ConvertError::LossyCapability(name.into()))
    } else {
        warnings.push(ConversionWarning {
            capability: name.into(),
            message: "no termcap code; omitted".into(),
        });
        Ok(())
    }
}
fn expression_loss(
    name: &str,
    warnings: &mut Vec<ConversionWarning>,
    options: ConvertOptions,
) -> Result<(), ConvertError> {
    if options.profile == TermcapProfile::Strict {
        Err(ConvertError::UnsupportedExpression(name.into()))
    } else {
        warnings.push(ConversionWarning {
            capability: name.into(),
            message: "parameter expression unsupported by termcap; omitted".into(),
        });
        Ok(())
    }
}

fn records(source: &[u8]) -> Vec<(Vec<u8>, usize)> {
    let mut result = Vec::new();
    let mut current = Vec::new();
    let mut start_line = 1;
    for (index, raw) in source.split(|byte| *byte == b'\n').enumerate() {
        let line = raw.trim_ascii_end();
        let trimmed = line.trim_ascii_start();
        if trimmed.is_empty() || trimmed.starts_with(b"#") {
            continue;
        }
        if current.is_empty() {
            start_line = index + 1;
        }
        if let Some(prefix) = line.strip_suffix(b"\\") {
            current.extend_from_slice(prefix.trim_ascii());
        } else {
            current.extend_from_slice(line.trim_ascii());
            result.push((core::mem::take(&mut current), start_line));
        }
    }
    if !current.is_empty() {
        result.push((current, start_line));
    }
    result
}

fn convert_field(field: &[u8], line: usize) -> Result<String, ParseError> {
    if let Some(name) = field.strip_prefix(b"tc=") {
        let name = core::str::from_utf8(name)
            .map_err(|_| parse_error(line, "tc= name is not valid UTF-8"))?;
        return Ok(format!("use={name}"));
    }
    let (code, suffix) = field
        .iter()
        .position(|byte| matches!(*byte, b'=' | b'#' | b'@'))
        .map_or((field, &[][..]), |index| (&field[..index], &field[index..]));
    if code.is_empty() {
        return Err(parse_error(line, "empty termcap capability"));
    }
    let code = core::str::from_utf8(code)
        .map_err(|_| parse_error(line, "termcap capability name is not valid UTF-8"))?;
    let short = if suffix.starts_with(b"=") {
        StringCap::ALL
            .iter()
            .find(|cap| cap.termcap_name() == Some(code))
            .map(|cap| cap.short_name())
    } else if suffix.starts_with(b"#") {
        NumericCap::ALL
            .iter()
            .find(|cap| cap.termcap_name() == Some(code))
            .map(|cap| cap.short_name())
    } else {
        BooleanCap::ALL
            .iter()
            .find(|cap| cap.termcap_name() == Some(code))
            .map(|cap| cap.short_name())
    }
    .unwrap_or(code);
    if let Some(value) = suffix.strip_prefix(b"=") {
        Ok(format!("{short}={}", termcap_expression_to_terminfo(value)))
    } else {
        let suffix = core::str::from_utf8(suffix)
            .map_err(|_| parse_error(line, "termcap capability value is not valid ASCII"))?;
        Ok(format!("{short}{suffix}"))
    }
}

fn termcap_expression_to_terminfo(value: &[u8]) -> String {
    let mut output = String::new();
    let mut parameters: Vec<String> = (1..=9).map(|index| format!("%p{index}")).collect();
    let mut index = 0;
    let mut parameter = 0usize;
    while index < value.len() {
        if value[index] != b'%' {
            push_source_byte(&mut output, value[index]);
            index += 1;
            continue;
        }
        let Some(&operator) = value.get(index + 1) else {
            output.push('%');
            break;
        };
        match operator {
            b'%' => {
                output.push_str("%%");
                index += 2;
            }
            b'r' => {
                parameters.swap(0, 1);
                index += 2;
            }
            b'i' => {
                for expression in &mut parameters[..2] {
                    *expression = format!("{expression}%{{1}}%+");
                }
                index += 2;
            }
            b'n' | b'm' => {
                let mask = if operator == b'n' { 0o140 } else { 0o177 };
                for expression in &mut parameters[..2] {
                    *expression = format!("{expression}%{{{mask}}}%^");
                }
                index += 2;
            }
            b'B' | b'6' => {
                let expression = parameters[parameter].clone();
                parameters[parameter] =
                    format!("{expression}%{{10}}%/%{{16}}%*{expression}%{{10}}%m%+");
                index += 2;
            }
            b'D' | b'8' => {
                let expression = parameters[parameter].clone();
                parameters[parameter] = format!("{expression}{expression}%{{16}}%m%{{2}}%*%-");
                index += 2;
            }
            b'>' => {
                let Some((threshold, threshold_length)) =
                    decode_legacy_character(&value[index + 2..])
                else {
                    output.push_str("%>");
                    index += 2;
                    continue;
                };
                let Some((increment, increment_length)) =
                    decode_legacy_character(&value[index + 2 + threshold_length..])
                else {
                    output.push_str("%>");
                    index += 2;
                    continue;
                };
                let expression = parameters[parameter].clone();
                parameters[parameter] = format!(
                    "%?{expression}%{{{threshold}}}%>%t{expression}%{{{increment}}}%+%e{expression}%;"
                );
                index += 2 + threshold_length + increment_length;
            }
            b'a' => {
                let rest = &value[index + 2..];
                if rest.len() >= 3
                    && matches!(rest[0], b'=' | b'+' | b'-' | b'*' | b'/')
                    && matches!(rest[1], b'p' | b'c')
                {
                    let operator = rest[0];
                    let (operand, consumed) = if rest[1] == b'p' {
                        let target = parameter
                            .checked_add(usize::from(rest[2].saturating_sub(b'@')))
                            .filter(|target| *target < parameters.len());
                        let Some(target) = target else {
                            output.push_str("%a");
                            index += 2;
                            continue;
                        };
                        (parameters[target].clone(), 3)
                    } else {
                        let Some((character, length)) = decode_legacy_character(&rest[2..]) else {
                            output.push_str("%a");
                            index += 2;
                            continue;
                        };
                        (format!("%{{{character}}}"), 2 + length)
                    };
                    parameters[parameter] = if operator == b'=' {
                        operand
                    } else {
                        format!(
                            "{}{operand}%{}",
                            parameters[parameter],
                            char::from(operator)
                        )
                    };
                    index += 2 + consumed;
                } else if let Some((character, length)) = decode_legacy_character(rest) {
                    parameters[parameter] = format!("{}%{{{character}}}%+", parameters[parameter]);
                    index += 2 + length;
                } else {
                    output.push_str("%a");
                    index += 2;
                }
            }
            b'f' => {
                parameter = (parameter + 1).min(parameters.len() - 1);
                index += 2;
            }
            b'b' => {
                parameter = parameter.saturating_sub(1);
                index += 2;
            }
            b'+' | b'-' => {
                let Some((character, length)) = decode_legacy_character(&value[index + 2..]) else {
                    output.push('%');
                    output.push(char::from(operator));
                    index += 2;
                    continue;
                };
                if operator == b'+' {
                    output.push_str(&format!("{}%{{{character}}}%+%c", parameters[parameter]));
                } else {
                    output.push_str(&format!("%{{{character}}}{}%-%c", parameters[parameter]));
                }
                parameter = (parameter + 1).min(parameters.len() - 1);
                index += 2 + length;
            }
            b'd' | b'.' | b's' => {
                output.push_str(&parameters[parameter]);
                output.push('%');
                output.push(if operator == b'.' {
                    'c'
                } else {
                    char::from(operator)
                });
                parameter = (parameter + 1).min(parameters.len() - 1);
                index += 2;
            }
            b'2' | b'3' => {
                output.push_str(&parameters[parameter]);
                output.push('%');
                output.push(char::from(operator));
                output.push('d');
                parameter = (parameter + 1).min(parameters.len() - 1);
                index += 2;
            }
            b'0' if matches!(value.get(index + 2), Some(b'2' | b'3')) => {
                output.push_str(&parameters[parameter]);
                output.push_str("%0");
                output.push(char::from(value[index + 2]));
                output.push('d');
                parameter = (parameter + 1).min(parameters.len() - 1);
                index += 3;
            }
            _ => {
                output.push('%');
                output.push(char::from(operator));
                index += 2;
            }
        }
    }
    output
}

fn push_source_byte(output: &mut String, byte: u8) {
    match byte {
        b',' => output.push_str("\\,"),
        0 => output.push_str("\\200"),
        0x20..=0x7e => output.push(char::from(byte)),
        _ => output.push_str(&format!("\\{byte:03o}")),
    }
}

fn decode_legacy_character(value: &[u8]) -> Option<(u8, usize)> {
    match *value.first()? {
        b'^' => {
            let character = *value.get(1)?;
            Some((
                if character == b'?' {
                    0x7f
                } else {
                    character & 0x1f
                },
                2,
            ))
        }
        b'\\' => {
            let escaped = *value.get(1)?;
            if matches!(escaped, b'0'..=b'7') {
                let mut number = 0u16;
                let mut length = 1usize;
                while length <= 3
                    && value
                        .get(length)
                        .is_some_and(|byte| matches!(*byte, b'0'..=b'7'))
                {
                    number = number * 8 + u16::from(value[length] - b'0');
                    length += 1;
                }
                Some(((number & 0xff) as u8, length))
            } else {
                let character = match escaped {
                    b'E' | b'e' => 0x1b,
                    b'n' | b'l' => b'\n',
                    b'r' => b'\r',
                    b't' => b'\t',
                    b'b' => 0x08,
                    b'f' => 0x0c,
                    other => other,
                };
                Some((character, 2))
            }
        }
        character => Some((character, 1)),
    }
}

fn terminfo_expression_to_termcap(value: &[u8]) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut index = 0;
    let mut expected = 1;
    while index < value.len() {
        if value[index] != b'%' {
            output.push(value[index]);
            index += 1;
            continue;
        }
        if value.get(index + 1) == Some(&b'p') {
            let parameter = value.get(index + 2)?.checked_sub(b'0')?;
            if parameter != expected {
                return None;
            }
            expected = expected % 2 + 1;
            index += 3;
            if value.get(index) != Some(&b'%') {
                return None;
            }
            match *value.get(index + 1)? {
                b'd' => output.extend_from_slice(b"%d"),
                b'c' => output.extend_from_slice(b"%."),
                b'0' if value.get(index + 2) == Some(&b'2')
                    && value.get(index + 3) == Some(&b'd') =>
                {
                    output.extend_from_slice(b"%2");
                    index += 2;
                }
                b'0' if value.get(index + 2) == Some(&b'3')
                    && value.get(index + 3) == Some(&b'd') =>
                {
                    output.extend_from_slice(b"%3");
                    index += 2;
                }
                _ => return None,
            }
            index += 2;
        } else if matches!(value.get(index + 1), Some(b'i' | b'%')) {
            output.extend_from_slice(&value[index..index + 2]);
            index += 2;
        } else {
            return None;
        }
    }
    Some(output)
}

fn escape_termcap(value: &[u8]) -> String {
    escape(value).replace("\\,", ",").replace(':', "\\:")
}
fn split_unescaped(value: &[u8], separator: u8) -> Vec<&[u8]> {
    let mut fields = Vec::new();
    let mut start = 0;
    for (index, byte) in value.iter().copied().enumerate() {
        if byte == separator
            && value[..index]
                .iter()
                .rev()
                .take_while(|b| **b == b'\\')
                .count()
                .is_multiple_of(2)
        {
            fields.push(&value[start..index]);
            start = index + 1;
        }
    }
    fields.push(&value[start..]);
    fields
}
fn parse_error(line: usize, message: &str) -> ParseError {
    ParseError {
        diagnostic: Box::new(Diagnostic::error(
            "TIK5001",
            message,
            Some(Span::at(line.saturating_sub(1), 0)),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand_legacy(source: &[u8], parameters: &[crate::expand::Param<'_>]) -> Vec<u8> {
        let modern = termcap_expression_to_terminfo(source);
        crate::expand::expand(modern.as_bytes(), parameters).unwrap()
    }

    #[test]
    fn translates_standard_and_extended_legacy_parameter_operators() {
        use crate::expand::Param;

        assert_eq!(
            expand_legacy(b"%r%d:%d", &[Param::Number(1), Param::Number(2)]),
            b"2:1"
        );
        assert_eq!(
            expand_legacy(b"%n%i%d", &[Param::Number(1), Param::Number(2)]),
            b"98"
        );
        assert_eq!(
            expand_legacy(b"%m%d", &[Param::Number(1), Param::Number(2)]),
            b"126"
        );
        assert_eq!(expand_legacy(b"%B%d", &[Param::Number(42)]), b"66");
        assert_eq!(expand_legacy(b"%D%d", &[Param::Number(31)]), b"1");
        assert_eq!(expand_legacy(b"%>A\\001%d", &[Param::Number(66)]), b"67");
        assert_eq!(expand_legacy(b"%a+cb%d", &[Param::Number(1)]), b"99");
        assert_eq!(
            expand_legacy(b"%a+pA%d", &[Param::Number(1), Param::Number(2)]),
            b"3"
        );
        assert_eq!(
            expand_legacy(
                b"%f%d%b%d",
                &[Param::Number(1), Param::Number(2), Param::Number(3)]
            ),
            b"22"
        );
        assert_eq!(expand_legacy(b"%aA%d", &[Param::Number(1)]), b"66");
        assert_eq!(expand_legacy(b"%-d", &[Param::Number(1)]), b"c");
        assert_eq!(expand_legacy(b"%02", &[Param::Number(7)]), b"07");
        assert_eq!(expand_legacy(b"%s", &[Param::Bytes(b"ok")]), b"ok");
    }

    #[test]
    fn termcap_tc_and_cursor_expression_compile() {
        let document =
            parse(b"base|base terminal:am:co#80:\nchild|child terminal:cm=\\E[%i%d;%dH:tc=base:\n")
                .unwrap();
        let compiled = compile(&document, CompilerOptions::default(), None).unwrap();
        let child = compiled.get("child").unwrap().entry();
        assert_eq!(
            child.boolean(BooleanCap::AUTO_RIGHT_MARGIN),
            BooleanState::Set
        );
        assert!(
            matches!(child.string(StringCap::CURSOR_ADDRESS), CapabilityState::Value(value) if value.windows(3).any(|part| part == b"%p1"))
        );
    }
    #[test]
    fn simple_entry_writes_termcap() {
        let entry = crate::model::EntryBuilder::new("demo")
            .unwrap()
            .boolean(BooleanCap::AUTO_RIGHT_MARGIN)
            .number(NumericCap::COLUMNS, 80)
            .unwrap()
            .build();
        assert!(
            from_entry(&entry, ConvertOptions::default())
                .unwrap()
                .source()
                .contains(":am:co#80:")
        );
    }

    #[test]
    fn preserves_non_utf8_comments_and_values() {
        let input = b"# comment \xff\nraw|raw terminal:zz=\xff:\n";
        let document = parse(input).unwrap();
        assert_eq!(document.original(), input);
        assert!(
            document
                .to_terminfo_source()
                .windows(4)
                .any(|window| window == b"\\377")
        );
        let compilation = compile(&document, CompilerOptions::default(), None).unwrap();
        assert!(compilation.get("raw").unwrap().entry().extended().iter().any(
            |cap| matches!(cap.state(), CapabilityState::Value(crate::model::ExtendedValue::String(value)) if value == b"\xff")
        ));
    }
}
