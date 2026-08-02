//! Editable logical terminfo entries.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::binary::{self, EncodeOptions};
use crate::caps::{BooleanCap, CapabilityId, NumericCap, StringCap};
use crate::error::{BuildError, EncodeError};

/// Presence state preserved by source and binary round trips.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CapabilityState<T> {
    /// Capability has no stored value.
    #[default]
    Absent,
    /// Capability is explicitly cancelled.
    Cancelled,
    /// Capability has a concrete value.
    Value(T),
}

impl<T> CapabilityState<T> {
    /// Reports whether the capability is absent.
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Reports whether the capability is cancelled.
    pub const fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled)
    }

    /// Borrows a concrete value while preserving presence state.
    pub fn as_ref(&self) -> CapabilityState<&T> {
        match self {
            Self::Absent => CapabilityState::Absent,
            Self::Cancelled => CapabilityState::Cancelled,
            Self::Value(value) => CapabilityState::Value(value),
        }
    }
}

/// State of a boolean capability. Terminfo has no stored false value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BooleanState {
    /// Boolean is not present.
    #[default]
    Absent,
    /// Boolean is explicitly cancelled.
    Cancelled,
    /// Boolean is present.
    Set,
}

impl BooleanState {
    /// Reports whether the boolean is absent.
    pub const fn is_absent(self) -> bool {
        matches!(self, Self::Absent)
    }

    /// Reports whether the boolean is cancelled.
    pub const fn is_cancelled(self) -> bool {
        matches!(self, Self::Cancelled)
    }
}

/// A non-negative number accepted by the compiled terminfo formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Number(i32);

impl Number {
    /// Largest number representable by the extended compiled format.
    pub const MAX: Self = Self(i32::MAX);

    /// Validates and constructs a non-negative compiled number.
    pub fn new(value: i64) -> Result<Self, BuildError> {
        i32::try_from(value)
            .ok()
            .filter(|value| *value >= 0)
            .map(Self)
            .ok_or(BuildError::InvalidNumber(value))
    }

    /// Returns the stored integer.
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl TryFrom<i32> for Number {
    type Error = BuildError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Self::new(i64::from(value))
    }
}

impl From<Number> for i32 {
    fn from(value: Number) -> Self {
        value.get()
    }
}

impl fmt::Display for Number {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A value belonging to a user-defined capability.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExtendedValue {
    /// Present boolean extension.
    Boolean,
    /// Numeric extension.
    Number(Number),
    /// Binary-safe string extension.
    String(Vec<u8>),
}

/// Type of an extended capability, required when cancelling an unknown name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExtendedKind {
    /// Boolean extension type.
    Boolean,
    /// Numeric extension type.
    Number,
    /// String extension type.
    String,
}

/// A user-defined capability, including cancellation state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedCapability {
    pub(crate) name: String,
    pub(crate) kind: ExtendedKind,
    pub(crate) state: CapabilityState<ExtendedValue>,
}

impl ExtendedCapability {
    /// Returns the user-defined capability name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the stable type used by the extended binary section.
    pub fn kind(&self) -> ExtendedKind {
        self.kind
    }

    /// Returns the borrowed presence state.
    pub fn state(&self) -> CapabilityState<&ExtendedValue> {
        self.state.as_ref()
    }
}

/// Borrowed state of a standard capability selected through CapabilityId.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilityValueRef<'a> {
    /// Borrowed standard boolean state.
    Boolean(BooleanState),
    /// Borrowed standard numeric state.
    Number(CapabilityState<Number>),
    /// Borrowed standard string state.
    String(CapabilityState<&'a [u8]>),
}

/// Primary name, lookup aliases, and the optional final verbose name.
///
/// Ncurses treats a final names-field without whitespace as both an alias and
/// the verbose name. The source_fields method emits that shared value only
/// once, so decoding and re-encoding does not duplicate it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryNames {
    primary: String,
    aliases: Vec<String>,
    verbose_name: Option<String>,
}

impl EntryNames {
    /// Creates a names model with one validated primary name.
    pub fn new(primary: impl Into<String>) -> Result<Self, BuildError> {
        let primary = primary.into();
        validate_terminal_name(&primary)?;
        Ok(Self {
            primary,
            aliases: Vec::new(),
            verbose_name: None,
        })
    }

    /// Interprets raw names fields using ncurses alias/verbose rules.
    pub fn from_source_fields(fields: &[String]) -> Result<Self, BuildError> {
        let Some(primary) = fields.first() else {
            return Err(BuildError::EmptyPrimaryName);
        };
        let mut result = Self::new(primary.clone())?;
        let verbose_index = fields.len().checked_sub(1).filter(|&index| index > 0);
        for (index, field) in fields.iter().enumerate().skip(1) {
            if Some(index) == verbose_index {
                result.set_verbose_name(Some(field.clone()))?;
                continue;
            }
            result.push_alias(field.clone())?;
        }
        Ok(result)
    }

    /// Returns the primary lookup name.
    pub fn primary(&self) -> &str {
        &self.primary
    }
    /// Returns lookup aliases in source order.
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }
    /// Returns the final human-readable names field.
    pub fn verbose_name(&self) -> Option<&str> {
        self.verbose_name.as_deref()
    }

    /// Compatibility synonym for verbose_name.
    pub fn description(&self) -> Option<&str> {
        self.verbose_name()
    }

    /// Adds a validated, unique lookup alias.
    pub fn push_alias(&mut self, alias: impl Into<String>) -> Result<(), BuildError> {
        let alias = alias.into();
        validate_terminal_name(&alias)?;
        if alias == self.primary || self.aliases.iter().any(|item| item == &alias) {
            return Err(BuildError::DuplicateAlias(alias));
        }
        self.aliases.push(alias);
        Ok(())
    }

    /// Replaces the final verbose names field.
    pub fn set_verbose_name(&mut self, verbose_name: Option<String>) -> Result<(), BuildError> {
        if let Some(value) = &verbose_name {
            validate_verbose_name(value)?;
            if value.bytes().all(|byte| !byte.is_ascii_whitespace()) && value == &self.primary {
                return Err(BuildError::DuplicateAlias(value.clone()));
            }
        }
        if let Some(previous) = self.verbose_name.take()
            && previous.bytes().all(|byte| !byte.is_ascii_whitespace())
        {
            self.aliases.retain(|alias| alias != &previous);
        }
        if let Some(value) = &verbose_name
            && value.bytes().all(|byte| !byte.is_ascii_whitespace())
            && !self.aliases.iter().any(|alias| alias == value)
        {
            self.push_alias(value.clone())?;
        }
        self.verbose_name = verbose_name;
        Ok(())
    }

    /// Compatibility synonym for set_verbose_name.
    pub fn set_description(&mut self, description: Option<String>) -> Result<(), BuildError> {
        self.set_verbose_name(description)
    }

    /// Reconstructs the original names column without duplicating a value that
    /// is both an alias and the verbose name.
    pub fn source_fields(&self) -> Vec<&str> {
        let mut fields =
            Vec::with_capacity(1 + self.aliases.len() + usize::from(self.verbose_name.is_some()));
        fields.push(self.primary.as_str());
        fields.extend(
            self.aliases
                .iter()
                .filter(|alias| Some(alias.as_str()) != self.verbose_name.as_deref())
                .map(String::as_str),
        );
        if let Some(verbose_name) = &self.verbose_name {
            fields.push(verbose_name);
        }
        fields
    }

    pub(crate) fn packed(&self) -> String {
        self.source_fields().join("|")
    }

    pub(crate) fn unpack(value: &str) -> Result<Self, BuildError> {
        let fields: Vec<String> = value.split('|').map(ToString::to_string).collect();
        Self::from_source_fields(&fields)
    }
}

pub(crate) fn validate_terminal_name(name: &str) -> Result<(), BuildError> {
    if name.is_empty() {
        return Err(BuildError::EmptyPrimaryName);
    }
    if name == "."
        || name == ".."
        || name.bytes().any(|b| {
            b == 0 || b.is_ascii_whitespace() || b == b'/' || b == b'\\' || b == b'|' || b == b','
        })
    {
        return Err(BuildError::InvalidName(name.to_string()));
    }
    Ok(())
}

fn validate_verbose_name(name: &str) -> Result<(), BuildError> {
    if name.is_empty() || name.bytes().any(|byte| matches!(byte, 0 | b'|' | b',')) {
        return Err(BuildError::InvalidName(name.to_string()));
    }
    Ok(())
}

pub(crate) fn validate_capability_name(name: &str) -> Result<(), BuildError> {
    if name.is_empty()
        || name.bytes().any(|b| {
            b == 0
                || b.is_ascii_whitespace()
                || matches!(b, b',' | b'=' | b'#' | b'@' | b'|' | b'/' | b'\\')
        })
    {
        return Err(BuildError::InvalidCapabilityName(name.to_string()));
    }
    Ok(())
}

/// A logical, editable terminfo entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub(crate) names: EntryNames,
    pub(crate) booleans: Vec<BooleanState>,
    pub(crate) numbers: Vec<CapabilityState<Number>>,
    pub(crate) strings: Vec<CapabilityState<Vec<u8>>>,
    pub(crate) extended: Vec<ExtendedCapability>,
}

impl Entry {
    /// Starts checked construction with a primary name.
    pub fn builder(primary: impl Into<String>) -> Result<EntryBuilder, BuildError> {
        EntryBuilder::new(primary)
    }

    /// Returns the structured names model.
    pub fn names(&self) -> &EntryNames {
        &self.names
    }
    /// Returns a standard boolean's presence state.
    pub fn boolean(&self, cap: BooleanCap) -> BooleanState {
        self.booleans.get(cap.index()).cloned().unwrap_or_default()
    }
    /// Returns a standard numeric capability state.
    pub fn number(&self, cap: NumericCap) -> CapabilityState<Number> {
        self.numbers.get(cap.index()).cloned().unwrap_or_default()
    }
    /// Returns a borrowed standard string capability state.
    pub fn string(&self, cap: StringCap) -> CapabilityState<&[u8]> {
        match self.strings.get(cap.index()) {
            Some(CapabilityState::Value(value)) => CapabilityState::Value(value),
            Some(CapabilityState::Cancelled) => CapabilityState::Cancelled,
            _ => CapabilityState::Absent,
        }
    }
    /// Returns user-defined capabilities in binary order.
    pub fn extended(&self) -> &[ExtendedCapability] {
        &self.extended
    }

    /// Returns any standard capability through a type-preserving identifier.
    pub fn capability(&self, id: CapabilityId) -> CapabilityValueRef<'_> {
        match id {
            CapabilityId::Boolean(cap) => CapabilityValueRef::Boolean(self.boolean(cap)),
            CapabilityId::Number(cap) => CapabilityValueRef::Number(self.number(cap)),
            CapabilityId::String(cap) => CapabilityValueRef::String(self.string(cap)),
        }
    }

    /// Marks a standard boolean as present.
    pub fn set_boolean(&mut self, cap: BooleanCap) {
        set_slot(&mut self.booleans, cap.index(), BooleanState::Set);
    }
    /// Sets a validated standard numeric value.
    pub fn set_number(&mut self, cap: NumericCap, value: i32) -> Result<(), BuildError> {
        let value = Number::try_from(value)?;
        set_slot(
            &mut self.numbers,
            cap.index(),
            CapabilityState::Value(value),
        );
        Ok(())
    }
    /// Sets a NUL-free standard string value.
    pub fn set_string(
        &mut self,
        cap: StringCap,
        value: impl Into<Vec<u8>>,
    ) -> Result<(), BuildError> {
        let value = value.into();
        if value.contains(&0) {
            return Err(BuildError::StringContainsNul(cap.short_name().to_string()));
        }
        set_slot(
            &mut self.strings,
            cap.index(),
            CapabilityState::Value(value),
        );
        Ok(())
    }
    /// Cancels a standard boolean.
    pub fn cancel_boolean(&mut self, cap: BooleanCap) {
        set_slot(&mut self.booleans, cap.index(), BooleanState::Cancelled);
    }
    /// Cancels a standard number.
    pub fn cancel_number(&mut self, cap: NumericCap) {
        set_slot(&mut self.numbers, cap.index(), CapabilityState::Cancelled);
    }
    /// Cancels a standard string.
    pub fn cancel_string(&mut self, cap: StringCap) {
        set_slot(&mut self.strings, cap.index(), CapabilityState::Cancelled);
    }
    /// Removes a standard boolean's value and cancellation.
    pub fn remove_boolean(&mut self, cap: BooleanCap) {
        set_slot(&mut self.booleans, cap.index(), BooleanState::Absent);
    }
    /// Removes a standard number's value and cancellation.
    pub fn remove_number(&mut self, cap: NumericCap) {
        set_slot(&mut self.numbers, cap.index(), CapabilityState::Absent);
    }
    /// Removes a standard string's value and cancellation.
    pub fn remove_string(&mut self, cap: StringCap) {
        set_slot(&mut self.strings, cap.index(), CapabilityState::Absent);
    }

    /// Sets or replaces a typed extended capability.
    pub fn set_extended(
        &mut self,
        name: impl Into<String>,
        value: ExtendedValue,
    ) -> Result<(), BuildError> {
        let name = name.into();
        validate_capability_name(&name)?;
        let kind = match &value {
            ExtendedValue::Boolean => ExtendedKind::Boolean,
            ExtendedValue::Number(_) => ExtendedKind::Number,
            ExtendedValue::String(_) => ExtendedKind::String,
        };
        if matches!(&value, ExtendedValue::String(value) if value.contains(&0)) {
            return Err(BuildError::StringContainsNul(name));
        }
        if let Some(cap) = self.extended.iter_mut().find(|cap| cap.name == name) {
            cap.kind = kind;
            cap.state = CapabilityState::Value(value);
        } else {
            self.extended.push(ExtendedCapability {
                name,
                kind,
                state: CapabilityState::Value(value),
            });
        }
        Ok(())
    }

    /// Cancels a known standard or already-typed extended name.
    pub fn cancel(&mut self, name: &str) -> Result<(), BuildError> {
        if let Some(cap) = BooleanCap::find_any(name) {
            self.cancel_boolean(cap);
            return Ok(());
        }
        if let Some(cap) = NumericCap::find_any(name) {
            self.cancel_number(cap);
            return Ok(());
        }
        if let Some(cap) = StringCap::find_any(name) {
            self.cancel_string(cap);
            return Ok(());
        }
        if let Some(cap) = self.extended.iter_mut().find(|cap| cap.name == name) {
            cap.state = CapabilityState::Cancelled;
            return Ok(());
        }
        Err(BuildError::ExtendedKindRequired(name.to_string()))
    }

    /// Cancels an extended name with an explicit stable type.
    pub fn cancel_extended(
        &mut self,
        name: impl Into<String>,
        kind: ExtendedKind,
    ) -> Result<(), BuildError> {
        let name = name.into();
        validate_capability_name(&name)?;
        if let Some(cap) = self.extended.iter_mut().find(|cap| cap.name == name) {
            cap.kind = kind;
            cap.state = CapabilityState::Cancelled;
        } else {
            self.extended.push(ExtendedCapability {
                name,
                kind,
                state: CapabilityState::Cancelled,
            });
        }
        Ok(())
    }

    /// Removes a standard or extended capability by any known name.
    pub fn remove(&mut self, name: &str) -> bool {
        if let Some(cap) = BooleanCap::find_any(name) {
            self.remove_boolean(cap);
            return true;
        }
        if let Some(cap) = NumericCap::find_any(name) {
            self.remove_number(cap);
            return true;
        }
        if let Some(cap) = StringCap::find_any(name) {
            self.remove_string(cap);
            return true;
        }
        let before = self.extended.len();
        self.extended.retain(|cap| cap.name != name);
        before != self.extended.len()
    }

    /// Encodes the entry with automatic numeric format selection.
    pub fn to_bytes(&self) -> Result<Vec<u8>, EncodeError> {
        self.to_bytes_with(EncodeOptions::default())
    }
    /// Encodes the entry with explicit output options.
    pub fn to_bytes_with(&self, options: EncodeOptions) -> Result<Vec<u8>, EncodeError> {
        binary::encode(self, options)
    }

    pub(crate) fn empty(names: EntryNames) -> Self {
        Self {
            names,
            booleans: vec![BooleanState::Absent; BooleanCap::COUNT],
            numbers: vec![CapabilityState::Absent; NumericCap::COUNT],
            strings: vec![CapabilityState::Absent; StringCap::COUNT],
            extended: Vec::new(),
        }
    }
}

fn set_slot<T: Clone + Default>(slots: &mut Vec<T>, index: usize, value: T) {
    if slots.len() <= index {
        slots.resize(index + 1, T::default());
    }
    slots[index] = value;
}

/// Fluent checked construction of an [`Entry`].
/// Fluent checked builder for a logical entry.
pub struct EntryBuilder {
    entry: Entry,
}

impl EntryBuilder {
    /// Starts an entry with a validated primary name.
    pub fn new(primary: impl Into<String>) -> Result<Self, BuildError> {
        Ok(Self {
            entry: Entry::empty(EntryNames::new(primary)?),
        })
    }
    /// Adds a validated alias.
    pub fn alias(mut self, alias: impl Into<String>) -> Result<Self, BuildError> {
        self.entry.names.push_alias(alias)?;
        Ok(self)
    }
    /// Sets the final verbose names field.
    pub fn description(mut self, description: impl Into<String>) -> Result<Self, BuildError> {
        self.entry.names.set_description(Some(description.into()))?;
        Ok(self)
    }
    /// Adds a present boolean.
    pub fn boolean(mut self, cap: BooleanCap) -> Self {
        self.entry.set_boolean(cap);
        self
    }
    /// Adds a validated number.
    pub fn number(mut self, cap: NumericCap, value: i32) -> Result<Self, BuildError> {
        self.entry.set_number(cap, value)?;
        Ok(self)
    }
    /// Adds a NUL-free binary string.
    pub fn string(mut self, cap: StringCap, value: impl Into<Vec<u8>>) -> Result<Self, BuildError> {
        self.entry.set_string(cap, value)?;
        Ok(self)
    }
    /// Adds a typed user-defined capability.
    pub fn extended(
        mut self,
        name: impl Into<String>,
        value: ExtendedValue,
    ) -> Result<Self, BuildError> {
        self.entry.set_extended(name, value)?;
        Ok(self)
    }
    /// Finishes construction.
    pub fn build(self) -> Entry {
        self.entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{NumericCap, StringCap};

    #[test]
    fn final_compact_name_is_alias_and_verbose_name_without_duplication() {
        let names =
            EntryNames::from_source_fields(&["demo".into(), "alias".into(), "longname".into()])
                .unwrap();
        assert_eq!(names.primary(), "demo");
        assert_eq!(names.aliases(), ["alias", "longname"]);
        assert_eq!(names.verbose_name(), Some("longname"));
        assert_eq!(names.source_fields(), ["demo", "alias", "longname"]);

        let described = EntryNames::from_source_fields(&[
            "demo".into(),
            "alias".into(),
            "demo terminal".into(),
        ])
        .unwrap();
        assert_eq!(described.aliases(), ["alias"]);
        assert_eq!(described.verbose_name(), Some("demo terminal"));

        let mut assembled = EntryNames::new("demo").unwrap();
        assembled.push_alias("longname").unwrap();
        assembled.set_verbose_name(Some("longname".into())).unwrap();
        assert_eq!(assembled.aliases(), ["longname"]);
        assert_eq!(assembled.verbose_name(), Some("longname"));
        assert_eq!(assembled.source_fields(), ["demo", "longname"]);
        assembled.set_verbose_name(Some("longname".into())).unwrap();
        assert_eq!(assembled.source_fields(), ["demo", "longname"]);
    }

    #[test]
    fn construction_rejects_negative_numbers_nul_and_untyped_cancel() {
        let mut entry = EntryBuilder::new("checked").unwrap().build();
        assert!(matches!(
            entry.set_number(NumericCap::COLUMNS, -1),
            Err(BuildError::InvalidNumber(-1))
        ));
        assert!(matches!(
            entry.set_string(StringCap::CLEAR_SCREEN, b"a\0b".to_vec()),
            Err(BuildError::StringContainsNul(_))
        ));
        assert!(matches!(
            entry.cancel("Unknown"),
            Err(BuildError::ExtendedKindRequired(_))
        ));
        entry
            .cancel_extended("Unknown", ExtendedKind::String)
            .unwrap();
        assert!(matches!(
            entry.extended()[0].state(),
            CapabilityState::Cancelled
        ));
    }
}
