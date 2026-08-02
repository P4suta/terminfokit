// SPDX-FileCopyrightText: 2026 Yasunobu Sakashita
//
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Terminfo formatting and logical diffs.

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::Write as _;

use crate::caps::{BooleanCap, CapabilityMetadata, NumericCap, StringCap};
use crate::model::{BooleanState, CapabilityState, Entry, ExtendedValue};

/// Naming namespace used while rendering fixed capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NameStyle {
    /// Compact terminfo source names.
    Short,
    /// Descriptive terminfo names.
    Long,
}

/// Physical source layout policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Layout {
    /// Emit the complete entry on one line.
    Compact,
    /// Wrap fields to a target display width.
    Wrapped {
        /// Target width, clamped to a usable minimum.
        width: usize,
    },
    /// Emit each capability on its own indented line.
    OnePerLine,
}

/// Ordering policy for rendered capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CapabilitySort {
    /// Preserve compiled section and slot order.
    Storage,
    /// Sort each type by compact terminfo name.
    Short,
    /// Sort each type by descriptive terminfo name.
    Long,
    /// Sort by termcap code with short-name fallback.
    Termcap,
}

/// Options for deterministic terminfo source rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct FormatOptions {
    names: NameStyle,
    layout: Layout,
    extended: bool,
    sort: CapabilitySort,
}

impl FormatOptions {
    /// Creates the default short-name, wrapped formatter options.
    pub const fn new() -> Self {
        Self {
            names: NameStyle::Short,
            layout: Layout::Wrapped { width: 60 },
            extended: true,
            sort: CapabilitySort::Short,
        }
    }

    /// Returns the selected capability-name style.
    pub const fn names(self) -> NameStyle {
        self.names
    }

    /// Returns the physical layout.
    pub const fn layout(self) -> Layout {
        self.layout
    }

    /// Reports whether extended capabilities are included.
    pub const fn extended(self) -> bool {
        self.extended
    }

    /// Returns the capability ordering policy.
    pub const fn sort(self) -> CapabilitySort {
        self.sort
    }

    /// Replaces the capability-name style.
    pub const fn with_names(mut self, value: NameStyle) -> Self {
        self.names = value;
        self
    }

    /// Replaces the physical layout.
    pub const fn with_layout(mut self, value: Layout) -> Self {
        self.layout = value;
        self
    }

    /// Enables or disables extended capabilities.
    pub const fn with_extended(mut self, value: bool) -> Self {
        self.extended = value;
        self
    }

    /// Replaces the capability ordering policy.
    pub const fn with_sort(mut self, value: CapabilitySort) -> Self {
        self.sort = value;
        self
    }
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            names: NameStyle::Short,
            layout: Layout::Wrapped { width: 60 },
            extended: true,
            sort: CapabilitySort::Short,
        }
    }
}

/// Terminfo source formatter.
#[derive(Debug, Clone, Copy, Default)]
pub struct SourceFormatter {
    options: FormatOptions,
}

impl SourceFormatter {
    /// Creates a formatter from explicit options.
    pub const fn new(options: FormatOptions) -> Self {
        Self { options }
    }

    /// Renders one resolved logical entry.
    pub fn format(&self, entry: &Entry) -> String {
        let mut capabilities = Vec::new();
        let mut fixed = Vec::new();
        for cap in BooleanCap::ALL {
            let name = cap_name(cap.short_name(), cap.long_name(), self.options.names);
            match entry.boolean(*cap) {
                BooleanState::Absent => {}
                BooleanState::Cancelled => fixed.push((cap.metadata(), format!("{name}@"))),
                BooleanState::Set => fixed.push((cap.metadata(), name.into())),
            }
        }
        append_sorted(&mut capabilities, &mut fixed, self.options.sort);
        for cap in NumericCap::ALL {
            let name = cap_name(cap.short_name(), cap.long_name(), self.options.names);
            match entry.number(*cap) {
                CapabilityState::Absent => {}
                CapabilityState::Cancelled => fixed.push((cap.metadata(), format!("{name}@"))),
                CapabilityState::Value(value) => {
                    fixed.push((cap.metadata(), format!("{name}#{value}")))
                }
            }
        }
        append_sorted(&mut capabilities, &mut fixed, self.options.sort);
        for cap in StringCap::ALL {
            let name = cap_name(cap.short_name(), cap.long_name(), self.options.names);
            match entry.string(*cap) {
                CapabilityState::Absent => {}
                CapabilityState::Cancelled => fixed.push((cap.metadata(), format!("{name}@"))),
                CapabilityState::Value(value) => {
                    fixed.push((cap.metadata(), format!("{name}={}", escape(value))))
                }
            }
        }
        append_sorted(&mut capabilities, &mut fixed, self.options.sort);
        if self.options.extended {
            let mut extended = Vec::new();
            for cap in entry.extended() {
                let value = match cap.state() {
                    CapabilityState::Absent => continue,
                    CapabilityState::Cancelled => format!("{}@", cap.name()),
                    CapabilityState::Value(ExtendedValue::Boolean) => cap.name().into(),
                    CapabilityState::Value(ExtendedValue::Number(value)) => {
                        format!("{}#{value}", cap.name())
                    }
                    CapabilityState::Value(ExtendedValue::String(value)) => {
                        format!("{}={}", cap.name(), escape(value))
                    }
                };
                extended.push((cap.name(), value));
            }
            if self.options.sort != CapabilitySort::Storage {
                extended.sort_by(|left, right| left.0.cmp(right.0));
            }
            capabilities.extend(extended.into_iter().map(|(_, value)| value));
        }
        render(
            entry.names().source_fields().join("|"),
            &capabilities,
            self.options.layout,
        )
    }
}

fn append_sorted(
    output: &mut Vec<String>,
    capabilities: &mut Vec<(&'static CapabilityMetadata, String)>,
    sort: CapabilitySort,
) {
    match sort {
        CapabilitySort::Storage => {}
        CapabilitySort::Short => {
            capabilities.sort_by(|left, right| left.0.short_name().cmp(right.0.short_name()));
        }
        CapabilitySort::Long => {
            capabilities.sort_by(|left, right| left.0.long_name().cmp(right.0.long_name()));
        }
        CapabilitySort::Termcap => {
            capabilities.sort_by(|left, right| {
                left.0
                    .termcap_name()
                    .unwrap_or(left.0.short_name())
                    .cmp(right.0.termcap_name().unwrap_or(right.0.short_name()))
            });
        }
    }
    output.extend(capabilities.drain(..).map(|(_, value)| value));
}

fn cap_name<'a>(short: &'a str, long: &'a str, style: NameStyle) -> &'a str {
    match style {
        NameStyle::Short => short,
        NameStyle::Long => long,
    }
}

pub(crate) fn render(names: String, capabilities: &[String], layout: Layout) -> String {
    match layout {
        Layout::Compact => {
            let mut output = names;
            output.push(',');
            for cap in capabilities {
                output.push_str(cap);
                output.push(',');
            }
            output.push('\n');
            output
        }
        Layout::OnePerLine => {
            let mut output = names;
            output.push_str(",\n");
            for cap in capabilities {
                output.push('\t');
                output.push_str(cap);
                output.push_str(",\n");
            }
            output
        }
        Layout::Wrapped { width } => {
            let width = width.max(20);
            let mut output = names;
            output.push_str(",\n\t");
            let mut column = 8;
            for cap in capabilities {
                if column > 8 && column + cap.len() + 2 > width {
                    output.push_str("\n\t");
                    column = 8;
                }
                output.push_str(cap);
                output.push(',');
                column += cap.len() + 1;
                if column < width {
                    output.push(' ');
                    column += 1;
                }
            }
            if output.ends_with(' ') {
                output.pop();
            }
            output.push('\n');
            output
        }
    }
}

/// Escapes arbitrary capability bytes for terminfo source.
pub fn escape(value: &[u8]) -> String {
    let mut output = String::new();
    for &byte in value {
        match byte {
            0x1b => output.push_str("\\E"),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            b'\x08' => output.push_str("\\b"),
            b'\x0c' => output.push_str("\\f"),
            b'\\' => output.push_str("\\\\"),
            b',' => output.push_str("\\,"),
            b'^' => output.push_str("\\^"),
            0x20..=0x7e => output.push(char::from(byte)),
            0..=31 => {
                output.push('^');
                output.push(char::from(byte + 0x40));
            }
            127 => output.push_str("^?"),
            _ => {
                let _ = write!(output, "\\{:03o}", byte);
            }
        }
    }
    output
}

/// One typed logical capability difference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    name: String,
    left: String,
    right: String,
}

impl Difference {
    /// Returns the short capability name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the left rendered state.
    pub fn left(&self) -> &str {
        &self.left
    }

    /// Returns the right rendered state.
    pub fn right(&self) -> &str {
        &self.right
    }
}

/// Complete logical difference between two entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryDiff {
    differences: Vec<Difference>,
}

impl EntryDiff {
    /// Returns differences in fixed-type and capability order.
    pub fn differences(&self) -> &[Difference] {
        &self.differences
    }
    /// Reports whether the entries are logically equal.
    pub fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }
}

/// Overrides plus ordered use targets representing a relative entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeEntry {
    bases: Vec<String>,
    overrides: Entry,
}

impl RelativeEntry {
    /// Returns the first base name, retained for single-base callers.
    pub fn base(&self) -> &str {
        self.bases.first().map_or("", String::as_str)
    }

    /// Returns all use targets in merge order.
    pub fn bases(&self) -> &[String] {
        &self.bases
    }

    /// Returns the direct override entry.
    pub fn overrides(&self) -> &Entry {
        &self.overrides
    }

    /// Renders overrides followed by ordered use fields.
    pub fn format(&self, formatter: &SourceFormatter) -> String {
        let mut source = formatter.format(&self.overrides);
        for base in &self.bases {
            source.push_str(&alloc::format!("\tuse={base},\n"));
        }
        source
    }
}

impl Entry {
    /// Computes a typed logical difference against another entry.
    pub fn diff(&self, other: &Self) -> EntryDiff {
        let mut differences = Vec::new();
        for cap in BooleanCap::ALL {
            push_diff(
                &mut differences,
                cap.short_name(),
                &self.boolean(*cap),
                &other.boolean(*cap),
            );
        }
        for cap in NumericCap::ALL {
            push_diff(
                &mut differences,
                cap.short_name(),
                &self.number(*cap),
                &other.number(*cap),
            );
        }
        for cap in StringCap::ALL {
            let left = self.string(*cap).map(escape);
            let right = other.string(*cap).map(escape);
            push_diff(&mut differences, cap.short_name(), &left, &right);
        }
        for cap in self.extended().iter().chain(other.extended()) {
            if differences
                .iter()
                .any(|item: &Difference| item.name == cap.name())
            {
                continue;
            }
            let left = self
                .extended()
                .iter()
                .find(|item| item.name() == cap.name())
                .map(|item| format!("{:?}", item.state()))
                .unwrap_or_else(|| "Absent".into());
            let right = other
                .extended()
                .iter()
                .find(|item| item.name() == cap.name())
                .map(|item| format!("{:?}", item.state()))
                .unwrap_or_else(|| "Absent".into());
            if left != right {
                differences.push(Difference {
                    name: cap.name().into(),
                    left,
                    right,
                });
            }
        }
        EntryDiff { differences }
    }

    /// Computes direct overrides relative to one base entry.
    pub fn relative_to(&self, base: &Self) -> RelativeEntry {
        let mut overrides = Entry::empty(self.names.clone());
        for cap in BooleanCap::ALL {
            let target = self.boolean(*cap);
            if target == base.boolean(*cap) {
                continue;
            }
            match target {
                BooleanState::Set => overrides.set_boolean(*cap),
                BooleanState::Cancelled => overrides.cancel_boolean(*cap),
                BooleanState::Absent => overrides.cancel_boolean(*cap),
            }
        }
        for cap in NumericCap::ALL {
            let target = self.number(*cap);
            if target == base.number(*cap) {
                continue;
            }
            match target {
                CapabilityState::Value(value) => {
                    let _ = overrides.set_number(*cap, value.get());
                }
                CapabilityState::Cancelled | CapabilityState::Absent => {
                    overrides.cancel_number(*cap);
                }
            }
        }
        for cap in StringCap::ALL {
            let target = self.string(*cap);
            if target == base.string(*cap) {
                continue;
            }
            match target {
                CapabilityState::Value(value) => {
                    let _ = overrides.set_string(*cap, value);
                }
                CapabilityState::Cancelled | CapabilityState::Absent => {
                    overrides.cancel_string(*cap);
                }
            }
        }
        for capability in self.extended().iter().chain(base.extended()) {
            if overrides
                .extended
                .iter()
                .any(|item| item.name == capability.name)
            {
                continue;
            }
            let target = self
                .extended
                .iter()
                .find(|item| item.name == capability.name);
            let inherited = base
                .extended
                .iter()
                .find(|item| item.name == capability.name);
            if target == inherited {
                continue;
            }
            match target {
                Some(value) => overrides.extended.push(value.clone()),
                None => overrides.extended.push(crate::model::ExtendedCapability {
                    name: capability.name.clone(),
                    kind: capability.kind,
                    state: CapabilityState::Cancelled,
                }),
            }
        }
        RelativeEntry {
            bases: vec![base.names().primary().into()],
            overrides,
        }
    }

    /// Computes direct overrides relative to multiple ordered bases.
    pub fn relative_to_many(&self, bases: &[&Self]) -> RelativeEntry {
        if bases.is_empty() {
            return RelativeEntry {
                bases: Vec::new(),
                overrides: self.clone(),
            };
        }
        let mut combined = Entry::empty(bases[0].names.clone());
        for base in bases.iter().rev() {
            merge_relative_base(&mut combined, base);
        }
        let mut relative = self.relative_to(&combined);
        relative.bases = bases
            .iter()
            .map(|entry| entry.names().primary().into())
            .collect();
        relative
    }
}

fn merge_relative_base(target: &mut Entry, source: &Entry) {
    if target.booleans.len() < source.booleans.len() {
        target
            .booleans
            .resize(source.booleans.len(), BooleanState::Absent);
    }
    for (slot, inherited) in target.booleans.iter_mut().zip(&source.booleans) {
        if inherited.is_cancelled() {
            *slot = BooleanState::Absent;
        } else if !inherited.is_absent() {
            *slot = *inherited;
        }
    }
    merge_relative_slots(&mut target.numbers, &source.numbers);
    merge_relative_slots(&mut target.strings, &source.strings);
    for inherited in &source.extended {
        if let Some(existing) = target
            .extended
            .iter_mut()
            .find(|capability| capability.name == inherited.name)
        {
            if inherited.state.is_cancelled() {
                existing.state = CapabilityState::Absent;
            } else if !inherited.state.is_absent() {
                *existing = inherited.clone();
            }
        } else {
            let mut inherited = inherited.clone();
            if inherited.state.is_cancelled() {
                inherited.state = CapabilityState::Absent;
            }
            target.extended.push(inherited);
        }
    }
}

fn merge_relative_slots<T: Clone>(
    target: &mut Vec<CapabilityState<T>>,
    source: &[CapabilityState<T>],
) {
    if target.len() < source.len() {
        target.resize(source.len(), CapabilityState::Absent);
    }
    for (slot, inherited) in target.iter_mut().zip(source) {
        if inherited.is_cancelled() {
            *slot = CapabilityState::Absent;
        } else if !inherited.is_absent() {
            *slot = inherited.clone();
        }
    }
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
fn push_diff<T: core::fmt::Debug + PartialEq>(
    target: &mut Vec<Difference>,
    name: &str,
    left: &T,
    right: &T,
) {
    if left != right {
        target.push(Difference {
            name: name.into(),
            left: format!("{left:?}"),
            right: format!("{right:?}"),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{BooleanCap, StringCap};
    use crate::model::EntryBuilder;

    #[test]
    fn formatted_source_parses_again() {
        let entry = EntryBuilder::new("demo")
            .unwrap()
            .description("demo terminal")
            .unwrap()
            .string(StringCap::CLEAR_SCREEN, b"\x1b[H,\\".to_vec())
            .unwrap()
            .build();
        let text = SourceFormatter::default().format(&entry);
        assert!(crate::source::parse(text.as_bytes()).is_ok(), "{text}");
    }

    #[test]
    fn capability_sorting_is_explicit_and_stable_within_each_type() {
        let entry = EntryBuilder::new("sort")
            .unwrap()
            .boolean(BooleanCap::AUTO_LEFT_MARGIN)
            .boolean(BooleanCap::AUTO_RIGHT_MARGIN)
            .build();
        let storage = SourceFormatter::new(
            FormatOptions::new()
                .with_layout(Layout::Compact)
                .with_sort(CapabilitySort::Storage),
        )
        .format(&entry);
        let alphabetical = SourceFormatter::new(
            FormatOptions::new()
                .with_layout(Layout::Compact)
                .with_sort(CapabilitySort::Short),
        )
        .format(&entry);
        assert!(storage.contains(",bw,am,"));
        assert!(alphabetical.contains(",am,bw,"));
    }

    #[test]
    fn relative_to_many_round_trips_through_ordered_uses() {
        let compilation = crate::Compiler::new()
            .compile(
                b"target,cols#20,lines#31,clear@,\nleft,cols#10,lines#11,clear=left,\nright,cols#20,lines#30,clear=right,\n",
            )
            .unwrap();
        let target = compilation.get("target").unwrap().entry();
        let left = compilation.get("left").unwrap().entry();
        let right = compilation.get("right").unwrap().entry();
        let formatter = SourceFormatter::default();
        let mut source = target.relative_to_many(&[left, right]).format(&formatter);
        source.push_str(&formatter.format(left));
        source.push_str(&formatter.format(right));
        let reconstructed = crate::Compiler::new().compile(source.as_bytes()).unwrap();
        assert!(
            target
                .diff(reconstructed.get("target").unwrap().entry())
                .is_empty(),
            "{source}"
        );
    }
}
