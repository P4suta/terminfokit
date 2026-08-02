//! Source compilation and ncurses-style `use=` resolution.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::binary::{EncodeOptions, NumberFormat};
use crate::caps::{
    BooleanCap, CapabilityId, CapabilityKind, Lookup, NameNamespace, NumericCap, StringCap,
};
use crate::error::{BuildError, CompileError, CompileErrorKind, Diagnostic};
use crate::model::{BooleanState, CapabilityState, Entry, ExtendedKind, ExtendedValue, Number};
use crate::source::{self, Capability, ParseOptions, SourceDocument, SourceEntry, SourceLimits};

/// Typed failure returned by an external entry provider.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ProviderError {
    message: String,
}

impl ProviderError {
    /// Creates a provider failure from a displayable explanation.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the provider-supplied explanation.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Supplies already-resolved entries referenced outside the input source.
pub trait EntryProvider {
    /// Retrieves a resolved entry by primary name or alias.
    fn get(&self, name: &str) -> Result<Option<Entry>, ProviderError>;
}

impl EntryProvider for () {
    fn get(&self, _name: &str) -> Result<Option<Entry>, ProviderError> {
        Ok(None)
    }
}

/// Source resolution, extension retention, and binary encoding options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct CompilerOptions {
    extended: bool,
    retain_commented: bool,
    number_format: NumberFormat,
    source_limits: SourceLimits,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            extended: true,
            retain_commented: false,
            number_format: NumberFormat::Auto,
            source_limits: SourceLimits::standard(),
        }
    }
}

impl CompilerOptions {
    /// Creates options matching normal extended-aware library compilation.
    pub const fn new() -> Self {
        Self {
            extended: true,
            retain_commented: false,
            number_format: NumberFormat::Auto,
            source_limits: SourceLimits::standard(),
        }
    }

    /// Reports whether user-defined capabilities are retained.
    pub const fn extended(self) -> bool {
        self.extended
    }

    /// Returns the compiled numeric-format policy.
    pub const fn number_format(self) -> NumberFormat {
        self.number_format
    }

    /// Reports whether dot-prefixed capabilities are activated.
    pub const fn retain_commented(self) -> bool {
        self.retain_commented
    }

    /// Returns source parser resource limits.
    pub const fn source_limits(self) -> SourceLimits {
        self.source_limits
    }

    /// Enables or disables user-defined capabilities.
    pub const fn with_extended(mut self, value: bool) -> Self {
        self.extended = value;
        self
    }

    /// Include capabilities disabled with a leading `.` as active values.
    /// Enabling this also enables user-defined capabilities, matching `tic -a`.
    pub const fn with_retain_commented(mut self, value: bool) -> Self {
        self.retain_commented = value;
        if value {
            self.extended = true;
        }
        self
    }

    /// Replaces the compiled numeric-format policy.
    pub const fn with_number_format(mut self, value: NumberFormat) -> Self {
        self.number_format = value;
        self
    }

    /// Replaces source parser resource limits.
    pub const fn with_source_limits(mut self, value: SourceLimits) -> Self {
        self.source_limits = value;
        self
    }
}

/// A reusable compiler, optionally backed by an installed-entry provider.
pub struct Compiler<'a> {
    options: CompilerOptions,
    provider: Option<&'a dyn EntryProvider>,
}

impl<'a> Default for Compiler<'a> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> Compiler<'a> {
    /// Creates a compiler with no external provider.
    pub const fn new() -> Self {
        Self {
            options: CompilerOptions::new(),
            provider: None,
        }
    }
    /// Replaces all compiler options.
    pub const fn options(mut self, options: CompilerOptions) -> Self {
        self.options = options;
        self
    }
    /// Adds a provider for use targets outside the source document.
    pub fn provider(mut self, provider: &'a dyn EntryProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Parses, resolves, validates, and encodes a source buffer.
    pub fn compile(&self, source_bytes: &[u8]) -> Result<Compilation, CompileError> {
        let document = source::parse_with(
            source_bytes,
            ParseOptions::new().with_limits(self.options.source_limits()),
        )
        .map_err(|error| CompileError {
            kind: CompileErrorKind::Parse,
            diagnostics: vec![*error.diagnostic],
        })?;
        let resolution = self.resolve(&document)?;
        self.encode(&resolution)
    }

    /// Resolves a parsed document without serializing entries.
    pub fn resolve(&self, document: &SourceDocument) -> Result<Resolution, CompileError> {
        let mut resolver = Resolver::new(document.entries().to_vec(), self.options, self.provider)?;
        let mut entries = Vec::with_capacity(resolver.entries.len());
        for index in 0..resolver.entries.len() {
            entries.push(resolver.resolve(index)?);
        }
        Ok(Resolution {
            entries,
            diagnostics: resolver.diagnostics,
        })
    }

    /// Serializes an existing resolution with this compiler's options.
    pub fn encode(&self, resolution: &Resolution) -> Result<Compilation, CompileError> {
        let mut output = Vec::with_capacity(resolution.entries.len());
        for entry in &resolution.entries {
            let entry = entry.clone();
            let bytes = entry
                .to_bytes_with(
                    EncodeOptions::new()
                        .with_number_format(self.options.number_format())
                        .with_extended(self.options.extended()),
                )
                .map_err(|error| CompileError {
                    kind: CompileErrorKind::Encode(error),
                    diagnostics: resolution.diagnostics.clone(),
                })?;
            output.push(CompiledEntry { entry, bytes });
        }
        Ok(Compilation {
            entries: output,
            diagnostics: resolution.diagnostics.clone(),
        })
    }
}

/// Resolved logical entries plus non-fatal diagnostics.
#[derive(Debug, Clone)]
pub struct Resolution {
    entries: Vec<Entry>,
    diagnostics: Vec<Diagnostic>,
}

impl Resolution {
    /// Returns resolved entries in source order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Returns non-fatal compilation diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Finds a resolved primary name or alias.
    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| {
            entry.names().primary() == name
                || entry.names().aliases().iter().any(|alias| alias == name)
        })
    }
}

/// One resolved entry paired with encoded bytes.
#[derive(Debug, Clone)]
pub struct CompiledEntry {
    entry: Entry,
    bytes: Vec<u8>,
}

impl CompiledEntry {
    /// Returns the resolved logical entry.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }
    /// Returns the compiled representation.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Consumes the item into logical entry and compiled bytes.
    pub fn into_parts(self) -> (Entry, Vec<u8>) {
        (self.entry, self.bytes)
    }
}

/// Complete ordered compilation output and diagnostics.
#[derive(Debug, Clone)]
pub struct Compilation {
    entries: Vec<CompiledEntry>,
    diagnostics: Vec<Diagnostic>,
}

impl Compilation {
    /// Returns compiled entries in source order.
    pub fn entries(&self) -> &[CompiledEntry] {
        &self.entries
    }
    /// Returns non-fatal compilation diagnostics.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    /// Finds a compiled primary name or alias.
    pub fn get(&self, name: &str) -> Option<&CompiledEntry> {
        self.entries.iter().find(|item| {
            item.entry.names().primary() == name
                || item
                    .entry
                    .names()
                    .aliases()
                    .iter()
                    .any(|alias| alias == name)
        })
    }
}

struct Resolver<'a> {
    entries: Vec<SourceEntry>,
    aliases: BTreeMap<String, usize>,
    extended_kinds: BTreeMap<String, (ExtendedKind, crate::error::Span)>,
    states: Vec<u8>,
    cache: Vec<Option<Entry>>,
    stack: Vec<usize>,
    options: CompilerOptions,
    provider: Option<&'a dyn EntryProvider>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Resolver<'a> {
    fn new(
        entries: Vec<SourceEntry>,
        options: CompilerOptions,
        provider: Option<&'a dyn EntryProvider>,
    ) -> Result<Self, CompileError> {
        let mut aliases = BTreeMap::new();
        for (index, entry) in entries.iter().enumerate() {
            let names = entry.entry_names().map_err(|error| CompileError {
                kind: CompileErrorKind::Build(error.clone()),
                diagnostics: vec![Diagnostic::error(
                    "TIK2002",
                    error.to_string(),
                    Some(entry.span),
                )],
            })?;
            for name in
                core::iter::once(names.primary()).chain(names.aliases().iter().map(String::as_str))
            {
                if let Some(previous) = aliases.insert(name.to_string(), index) {
                    let diagnostic = Diagnostic::error(
                        "TIK2001",
                        alloc::format!("duplicate terminal name {name:?}"),
                        Some(entry.span),
                    );
                    return Err(CompileError {
                        kind: CompileErrorKind::Build(BuildError::DuplicateAlias(
                            entries[previous].primary_name().to_string(),
                        )),
                        diagnostics: vec![diagnostic],
                    });
                }
            }
        }
        let extended_kinds = collect_extended_kinds(&entries, options)?;
        let len = entries.len();
        Ok(Self {
            entries,
            aliases,
            extended_kinds,
            states: vec![0; len],
            cache: vec![None; len],
            stack: Vec::new(),
            options,
            provider,
            diagnostics: Vec::new(),
        })
    }

    fn resolve(&mut self, index: usize) -> Result<Entry, CompileError> {
        if let Some(entry) = &self.cache[index] {
            return Ok(entry.clone());
        }
        self.states[index] = 1;
        self.stack.clear();
        self.stack.push(index);
        while let Some(&current) = self.stack.last() {
            let unresolved = self.entries[current]
                .capabilities
                .iter()
                .filter(|capability| !capability.commented || self.options.retain_commented)
                .filter_map(|capability| match &capability.value {
                    Capability::Use { name } => self.aliases.get(name).copied(),
                    _ => None,
                })
                .find(|target| self.states[*target] != 2);
            if let Some(target) = unresolved {
                if self.states[target] == 1 {
                    return Err(self.cycle_error(target));
                }
                self.states[target] = 1;
                self.stack.push(target);
                continue;
            }
            let entry = self.finish(current)?;
            self.cache[current] = Some(entry);
            self.states[current] = 2;
            self.stack.pop();
        }
        self.cache[index].clone().ok_or_else(|| {
            self.missing_use(self.entries[index].primary_name(), self.entries[index].span)
        })
    }

    fn finish(&mut self, index: usize) -> Result<Entry, CompileError> {
        let source = self.entries[index].clone();
        let names = source
            .entry_names()
            .map_err(|error| self.build_error(error, source.span))?;
        let mut entry = Entry::empty(names);

        // ncurses processes uses in reverse order.  Each non-absent value then
        // replaces one imported earlier, so the left-most conflicting use wins.
        // Direct capabilities are applied afterwards and override every use.
        for capability in source.capabilities.iter().rev() {
            if capability.commented && !self.options.retain_commented {
                continue;
            }
            if let Capability::Use { name } = &capability.value {
                let inherited = if let Some(target) = self.aliases.get(name).copied() {
                    self.cache[target]
                        .clone()
                        .ok_or_else(|| self.missing_use(name, capability.span))?
                } else if let Some(provider) = self.provider {
                    match provider.get(name) {
                        Ok(Some(entry)) => entry,
                        Ok(None) => return Err(self.missing_use(name, capability.span)),
                        Err(error) => {
                            return Err(CompileError {
                                kind: CompileErrorKind::Provider {
                                    name: name.clone(),
                                    message: error.message().to_string(),
                                },
                                diagnostics: vec![Diagnostic::error(
                                    "TIK2004",
                                    error.message(),
                                    Some(capability.span),
                                )],
                            });
                        }
                    }
                } else {
                    return Err(self.missing_use(name, capability.span));
                };
                merge_inherited(&mut entry, &inherited);
            }
        }
        for capability in &source.capabilities {
            if (!capability.commented || self.options.retain_commented)
                && !matches!(capability.value, Capability::Use { .. })
            {
                self.apply_direct(&mut entry, capability)?;
            }
        }
        Ok(entry)
    }

    fn cycle_error(&self, target: usize) -> CompileError {
        let first = self
            .stack
            .iter()
            .position(|candidate| *candidate == target)
            .unwrap_or(0);
        let mut chain: Vec<String> = self.stack[first..]
            .iter()
            .map(|value| self.entries[*value].primary_name().to_string())
            .collect();
        chain.push(self.entries[target].primary_name().to_string());
        CompileError {
            kind: CompileErrorKind::UseCycle {
                chain: chain.clone(),
            },
            diagnostics: vec![Diagnostic::error(
                "TIK2003",
                alloc::format!("use= cycle: {}", chain.join(" -> ")),
                Some(self.entries[target].span),
            )],
        }
    }

    fn apply_direct(
        &mut self,
        entry: &mut Entry,
        capability: &source::SourceCapability,
    ) -> Result<(), CompileError> {
        match &capability.value {
            Capability::Boolean { name } => {
                if let Some(cap) = source_boolean(name) {
                    entry.set_boolean(cap);
                } else if source_standard(name).is_some() {
                    return Err(self.wrong_type(name, CapabilityKind::Boolean, capability.span));
                } else {
                    self.extended(entry, name, ExtendedValue::Boolean, capability.span)?;
                }
            }
            Capability::Numeric { name, value } => {
                if *value < 0 {
                    return Err(self.build_error(
                        BuildError::InvalidNumber(i64::from(*value)),
                        capability.span,
                    ));
                }
                if let Some(cap) = source_number(name) {
                    entry
                        .set_number(cap, *value)
                        .map_err(|error| self.build_error(error, capability.span))?;
                } else if source_standard(name).is_some() {
                    return Err(self.wrong_type(name, CapabilityKind::Number, capability.span));
                } else {
                    self.extended(
                        entry,
                        name,
                        ExtendedValue::Number(
                            Number::new(i64::from(*value))
                                .map_err(|error| self.build_error(error, capability.span))?,
                        ),
                        capability.span,
                    )?;
                }
            }
            Capability::String { name, value } => {
                if let Some(cap) = source_string(name) {
                    entry
                        .set_string(cap, value.clone())
                        .map_err(|error| self.build_error(error, capability.span))?;
                } else if source_standard(name).is_some() {
                    return Err(self.wrong_type(name, CapabilityKind::String, capability.span));
                } else {
                    self.extended(
                        entry,
                        name,
                        ExtendedValue::String(value.clone()),
                        capability.span,
                    )?;
                }
            }
            Capability::Cancel { name } => {
                if let Some(id) = source_standard(name) {
                    cancel_standard(entry, id);
                } else if self.options.extended {
                    let kind = self
                        .extended_kinds
                        .get(name)
                        .map(|(kind, _)| *kind)
                        .ok_or_else(|| {
                            self.build_error(
                                BuildError::ExtendedKindRequired(name.clone()),
                                capability.span,
                            )
                        })?;
                    entry
                        .cancel_extended(name, kind)
                        .map_err(|error| self.build_error(error, capability.span))?;
                } else {
                    self.unknown_warning(name, capability.span);
                }
            }
            Capability::Use { .. } => {}
        }
        Ok(())
    }

    fn extended(
        &mut self,
        entry: &mut Entry,
        name: &str,
        value: ExtendedValue,
        span: crate::error::Span,
    ) -> Result<(), CompileError> {
        if self.options.extended {
            entry
                .set_extended(name, value)
                .map_err(|error| self.build_error(error, span))?;
        } else {
            self.unknown_warning(name, span);
        }
        Ok(())
    }
    fn unknown_warning(&mut self, name: &str, span: crate::error::Span) {
        self.diagnostics.push(Diagnostic::warning("TIK2005", alloc::format!("unknown capability {name:?} was omitted; enable extended capabilities to preserve it"), Some(span)));
    }
    fn wrong_type(
        &self,
        name: &str,
        actual: CapabilityKind,
        span: crate::error::Span,
    ) -> CompileError {
        CompileError {
            kind: CompileErrorKind::Build(BuildError::InvalidCapabilityName(name.to_string())),
            diagnostics: vec![Diagnostic::error(
                "TIK2006",
                alloc::format!("capability {name:?} has the wrong type; parsed as {actual:?}"),
                Some(span),
            )],
        }
    }
    fn build_error(&self, error: BuildError, span: crate::error::Span) -> CompileError {
        CompileError {
            kind: CompileErrorKind::Build(error.clone()),
            diagnostics: vec![Diagnostic::error("TIK2002", error.to_string(), Some(span))],
        }
    }
    fn missing_use(&self, name: &str, span: crate::error::Span) -> CompileError {
        CompileError {
            kind: CompileErrorKind::MissingUse {
                name: name.to_string(),
            },
            diagnostics: vec![Diagnostic::error(
                "TIK2007",
                alloc::format!("unknown use= entry {name:?}"),
                Some(span),
            )],
        }
    }
}

fn source_boolean(name: &str) -> Option<BooleanCap> {
    BooleanCap::lookup(NameNamespace::Short, name)
        .or_else(|| BooleanCap::lookup(NameNamespace::Long, name))
}

fn source_number(name: &str) -> Option<NumericCap> {
    NumericCap::lookup(NameNamespace::Short, name)
        .or_else(|| NumericCap::lookup(NameNamespace::Long, name))
}

fn source_string(name: &str) -> Option<StringCap> {
    StringCap::lookup(NameNamespace::Short, name)
        .or_else(|| StringCap::lookup(NameNamespace::Long, name))
}

fn source_standard(name: &str) -> Option<CapabilityId> {
    for namespace in [NameNamespace::Short, NameNamespace::Long] {
        match crate::caps::lookup(namespace, name) {
            Lookup::Found(id) => return Some(id),
            Lookup::Ambiguous(ids) => return ids.first().copied(),
            Lookup::NotFound => {}
        }
    }
    None
}

fn cancel_standard(entry: &mut Entry, id: CapabilityId) {
    match id {
        CapabilityId::Boolean(cap) => entry.cancel_boolean(cap),
        CapabilityId::Number(cap) => entry.cancel_number(cap),
        CapabilityId::String(cap) => entry.cancel_string(cap),
    }
}

fn collect_extended_kinds(
    entries: &[SourceEntry],
    options: CompilerOptions,
) -> Result<BTreeMap<String, (ExtendedKind, crate::error::Span)>, CompileError> {
    let mut kinds = BTreeMap::new();
    if !options.extended {
        return Ok(kinds);
    }
    for entry in entries {
        for capability in entry
            .capabilities()
            .iter()
            .filter(|capability| !capability.is_commented() || options.retain_commented)
        {
            let definition = match capability.value() {
                Capability::Boolean { name }
                    if source_boolean(name).is_none() && source_standard(name).is_none() =>
                {
                    Some((name, ExtendedKind::Boolean))
                }
                Capability::Numeric { name, .. }
                    if source_number(name).is_none() && source_standard(name).is_none() =>
                {
                    Some((name, ExtendedKind::Number))
                }
                Capability::String { name, .. }
                    if source_string(name).is_none() && source_standard(name).is_none() =>
                {
                    Some((name, ExtendedKind::String))
                }
                _ => None,
            };
            let Some((name, kind)) = definition else {
                continue;
            };
            if let Some((previous_kind, previous_span)) = kinds.get(name).copied() {
                if previous_kind != kind {
                    return Err(CompileError {
                        kind: CompileErrorKind::ExtendedTypeConflict { name: name.clone() },
                        diagnostics: vec![
                            Diagnostic::error(
                                "TIK2008",
                                alloc::format!(
                                    "extended capability {name:?} is defined with conflicting types"
                                ),
                                Some(capability.span()),
                            )
                            .with_primary_message(alloc::format!("defined here as {kind:?}"))
                            .with_secondary(
                                previous_span,
                                alloc::format!("previously defined as {previous_kind:?}"),
                            ),
                        ],
                    });
                }
            } else {
                kinds.insert(name.clone(), (kind, capability.span()));
            }
        }
    }
    Ok(kinds)
}

fn merge_inherited(target: &mut Entry, source: &Entry) {
    if target.booleans.len() < source.booleans.len() {
        target
            .booleans
            .resize(source.booleans.len(), BooleanState::Absent);
    }
    for (slot, inherited) in target.booleans.iter_mut().zip(&source.booleans) {
        if *inherited == BooleanState::Cancelled {
            *slot = BooleanState::Absent;
        } else if *inherited != BooleanState::Absent {
            *slot = *inherited;
        }
    }
    merge_slots(&mut target.numbers, &source.numbers);
    merge_slots(&mut target.strings, &source.strings);
    for inherited in &source.extended {
        if let Some(existing) = target
            .extended
            .iter_mut()
            .find(|cap| cap.name == inherited.name)
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
fn merge_slots<T: Clone>(target: &mut Vec<CapabilityState<T>>, source: &[CapabilityState<T>]) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_forward_alias_and_leftmost_use() {
        let source = "child,use=left,use=right,cols#90,clear@,\nleft|l,cols#10,lines#20,clear=left,\nright|r,cols#30,lines#40,clear=right,\n";
        let result = Compiler::new().compile(source.as_bytes()).unwrap();
        let child = result.get("child").unwrap().entry();
        assert_eq!(
            child.number(NumericCap::COLUMNS),
            CapabilityState::Value(Number::new(90).unwrap())
        );
        assert_eq!(
            child.number(NumericCap::LINES),
            CapabilityState::Value(Number::new(20).unwrap())
        );
        assert_eq!(
            child.string(StringCap::CLEAR_SCREEN),
            CapabilityState::Cancelled
        );
    }

    #[test]
    fn direct_and_inherited_cancels_follow_ncurses_use_order() {
        let source = b"base,cols#80,clear=base,\n\
before,cols#90,use=base,\n\
after,use=base,cols#91,\n\
cancel_before,clear@,use=base,\n\
cancel_after,use=base,clear@,\n\
cancel_parent,clear@,\n\
left_cancel,use=cancel_parent,use=base,\n\
right_cancel,use=base,use=cancel_parent,\n";
        let result = Compiler::new().compile(source).unwrap();
        for (name, columns) in [("before", 90), ("after", 91)] {
            assert_eq!(
                result
                    .get(name)
                    .unwrap()
                    .entry()
                    .number(NumericCap::COLUMNS),
                CapabilityState::Value(Number::new(columns).unwrap())
            );
        }
        for name in ["cancel_before", "cancel_after"] {
            assert_eq!(
                result
                    .get(name)
                    .unwrap()
                    .entry()
                    .string(StringCap::CLEAR_SCREEN),
                CapabilityState::Cancelled,
                "{name}"
            );
        }
        assert_eq!(
            result
                .get("left_cancel")
                .unwrap()
                .entry()
                .string(StringCap::CLEAR_SCREEN),
            CapabilityState::Absent
        );
        assert_eq!(
            result
                .get("right_cancel")
                .unwrap()
                .entry()
                .string(StringCap::CLEAR_SCREEN),
            CapabilityState::Value(b"base".as_slice())
        );
    }

    #[test]
    fn reports_complete_cycle() {
        let error = Compiler::new()
            .compile(b"a,use=b,\nb,use=c,\nc,use=a,\n")
            .unwrap_err();
        assert!(
            matches!(error.kind, CompileErrorKind::UseCycle { ref chain } if chain == &["a", "b", "c", "a"])
        );
    }

    #[test]
    fn resolves_a_deep_graph_without_recursion() {
        let mut source = String::new();
        source.push_str("entry0,cols#80,\n");
        for index in 1..2000 {
            source.push_str(&alloc::format!("entry{index},use=entry{},\n", index - 1));
        }
        let compilation = Compiler::new().compile(source.as_bytes()).unwrap();
        assert_eq!(
            compilation
                .get("entry1999")
                .unwrap()
                .entry()
                .number(NumericCap::COLUMNS),
            CapabilityState::Value(Number::new(80).unwrap())
        );
    }

    #[test]
    fn commented_capabilities_are_ignored_unless_retained() {
        let source = b"base,cols#80,\nchild,.cols#132,.am,.use=base,\n";
        let normal = Compiler::new().compile(source).unwrap();
        let child = normal.get("child").unwrap().entry();
        assert_eq!(child.number(NumericCap::COLUMNS), CapabilityState::Absent);
        assert_eq!(
            child.boolean(BooleanCap::AUTO_RIGHT_MARGIN),
            BooleanState::Absent
        );

        let retained = Compiler::new()
            .options(CompilerOptions::new().with_retain_commented(true))
            .compile(source)
            .unwrap();
        let child = retained.get("child").unwrap().entry();
        assert_eq!(
            child.number(NumericCap::COLUMNS),
            CapabilityState::Value(Number::new(132).unwrap())
        );
        assert_eq!(
            child.boolean(BooleanCap::AUTO_RIGHT_MARGIN),
            BooleanState::Set
        );
    }

    #[test]
    fn descriptions_are_not_lookup_aliases_but_compact_longnames_are() {
        let source = b"base|compact,cols#80,\n\
other|shared terminal,lines#24,\n\
third|shared terminal,lines#25,\n\
child,use=compact,\n";
        let result = Compiler::new().compile(source).unwrap();
        assert!(result.get("compact").is_some());
        assert!(result.get("shared terminal").is_none());
        assert_eq!(
            result
                .get("child")
                .unwrap()
                .entry()
                .number(NumericCap::COLUMNS),
            CapabilityState::Value(Number::new(80).unwrap())
        );
    }

    #[test]
    fn source_uses_only_short_and_long_standard_names() {
        let result = Compiler::new()
            .compile(b"demo,xr,XF=value,dl=delete,ed=erase,ma#7,ML=lr,MT=tb,\n")
            .unwrap();
        let entry = result.get("demo").unwrap().entry();
        assert!(entry.extended().iter().any(|cap| cap.name() == "xr"));
        assert!(entry.extended().iter().any(|cap| cap.name() == "XF"));
        assert!(entry.extended().iter().any(|cap| cap.name() == "ML"));
        assert!(entry.extended().iter().any(|cap| cap.name() == "MT"));
        assert_eq!(
            entry.string(StringCap::PARM_DELETE_LINE),
            CapabilityState::Value(b"delete".as_slice())
        );
        assert_eq!(
            entry.string(StringCap::CLR_EOS),
            CapabilityState::Value(b"erase".as_slice())
        );
        assert_eq!(
            entry.number(NumericCap::MAX_ATTRIBUTES),
            CapabilityState::Value(Number::new(7).unwrap())
        );
    }

    #[test]
    fn extended_cancel_types_are_collected_across_the_document() {
        let result = Compiler::new()
            .compile(b"cancelled,BD@,\ndefined,BD=value,\n")
            .unwrap();
        let cap = result
            .get("cancelled")
            .unwrap()
            .entry()
            .extended()
            .iter()
            .find(|cap| cap.name() == "BD")
            .unwrap();
        assert_eq!(cap.kind(), ExtendedKind::String);
        assert!(matches!(cap.state(), CapabilityState::Cancelled));

        let error = Compiler::new().compile(b"unknown,ZZ@,\n").unwrap_err();
        assert!(matches!(
            error.kind(),
            CompileErrorKind::Build(BuildError::ExtendedKindRequired(name)) if name == "ZZ"
        ));
    }

    #[test]
    fn conflicting_extended_types_report_both_definitions() {
        let error = Compiler::new()
            .compile(b"one,ZZ,\ntwo,ZZ#1,\n")
            .unwrap_err();
        assert!(matches!(
            error.kind(),
            CompileErrorKind::ExtendedTypeConflict { name } if name == "ZZ"
        ));
        assert_eq!(error.diagnostics()[0].secondary().len(), 1);
    }
}
