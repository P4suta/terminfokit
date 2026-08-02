//! The generated ncurses capability vocabulary.
use alloc::vec::Vec;

/// Namespace used for an exact capability-name lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NameNamespace {
    /// Compact terminfo source names such as cup.
    Short,
    /// Descriptive terminfo names such as cursor_address.
    Long,
    /// Historical two-character termcap codes.
    Termcap,
}

/// Historical vocabulary generation in which a capability first appeared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CapabilityVersion {
    /// Capability belongs to the System V fixed vocabulary.
    SystemV,
}

/// Names and type information associated with one fixed-index capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityMetadata {
    long: &'static str,
    short: &'static str,
    termcap: Option<&'static str>,
    parameters: &'static [ParameterType],
    introduced: CapabilityVersion,
}

/// Parameter types used by `tput` and documented parameterized capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterType {
    /// Signed numeric parameter.
    Number,
    /// Uninterpreted byte-string parameter.
    Bytes,
}

impl CapabilityMetadata {
    /// Returns the compact terminfo name.
    pub const fn short_name(&self) -> &'static str {
        self.short
    }
    /// Returns the descriptive terminfo name.
    pub const fn long_name(&self) -> &'static str {
        self.long
    }
    /// Returns the termcap code when one exists.
    pub const fn termcap_name(&self) -> Option<&'static str> {
        self.termcap
    }
    /// Returns the expected parameter signature.
    pub const fn parameters(&self) -> &'static [ParameterType] {
        self.parameters
    }
    /// Returns the vocabulary generation that introduced the capability.
    pub const fn introduced(&self) -> CapabilityVersion {
        self.introduced
    }
    /// Returns the exact name in a selected namespace.
    pub const fn name(&self, namespace: NameNamespace) -> Option<&'static str> {
        match namespace {
            NameNamespace::Short => Some(self.short),
            NameNamespace::Long => Some(self.long),
            NameNamespace::Termcap => self.termcap,
        }
    }
    pub(crate) fn matches_any(&self, name: &str) -> bool {
        self.short == name || self.long == name || self.termcap == Some(name)
    }
}

include!(concat!(env!("OUT_DIR"), "/caps_generated.rs"));

/// The type of a known capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityKind {
    /// Boolean flag.
    Boolean,
    /// Non-negative integer.
    Number,
    /// Binary-safe byte string.
    String,
}

/// Type-preserving identifier for any standard capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CapabilityId {
    /// Identifier in the fixed boolean section.
    Boolean(BooleanCap),
    /// Identifier in the fixed numeric section.
    Number(NumericCap),
    /// Identifier in the fixed string section.
    String(StringCap),
}

impl CapabilityId {
    /// Returns the value type associated with this identifier.
    pub const fn kind(self) -> CapabilityKind {
        match self {
            Self::Boolean(_) => CapabilityKind::Boolean,
            Self::Number(_) => CapabilityKind::Number,
            Self::String(_) => CapabilityKind::String,
        }
    }

    /// Returns generated names and parameter metadata.
    pub fn metadata(self) -> &'static CapabilityMetadata {
        match self {
            Self::Boolean(cap) => cap.metadata(),
            Self::Number(cap) => cap.metadata(),
            Self::String(cap) => cap.metadata(),
        }
    }
}

/// Result of an exact, single-namespace lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lookup {
    /// No exact name exists.
    NotFound,
    /// Exactly one capability has the name.
    Found(CapabilityId),
    /// More than one capability has the exact name.
    Ambiguous(Vec<CapabilityId>),
}

/// Returns every exact match in one namespace, including same-type aliases.
pub fn lookup_all(namespace: NameNamespace, name: &str) -> Vec<CapabilityId> {
    let mut found = Vec::new();
    found.extend(
        BooleanCap::ALL
            .iter()
            .copied()
            .filter(|cap| cap.metadata().name(namespace) == Some(name))
            .map(CapabilityId::Boolean),
    );
    found.extend(
        NumericCap::ALL
            .iter()
            .copied()
            .filter(|cap| cap.metadata().name(namespace) == Some(name))
            .map(CapabilityId::Number),
    );
    found.extend(
        StringCap::ALL
            .iter()
            .copied()
            .filter(|cap| cap.metadata().name(namespace) == Some(name))
            .map(CapabilityId::String),
    );
    found
}

/// Look up a name in exactly one namespace.
pub fn lookup(namespace: NameNamespace, name: &str) -> Lookup {
    let found = lookup_all(namespace, name);
    match found.len() {
        0 => Lookup::NotFound,
        1 => Lookup::Found(found[0]),
        _ => Lookup::Ambiguous(found),
    }
}

/// Iterate over all 497 standard capabilities in binary-slot order by type.
pub fn all_capabilities() -> impl Iterator<Item = CapabilityId> + Clone {
    BooleanCap::ALL
        .iter()
        .copied()
        .map(CapabilityId::Boolean)
        .chain(NumericCap::ALL.iter().copied().map(CapabilityId::Number))
        .chain(StringCap::ALL.iter().copied().map(CapabilityId::String))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_table_matches_ncurses_fixed_counts() {
        assert_eq!(BooleanCap::COUNT, 44);
        assert_eq!(NumericCap::COUNT, 39);
        assert_eq!(StringCap::COUNT, 414);
        assert_eq!(
            StringCap::CURSOR_ADDRESS.metadata().parameters(),
            &[ParameterType::Number, ParameterType::Number]
        );
        assert_eq!(all_capabilities().count(), 497);
        assert!(matches!(
            lookup(NameNamespace::Short, "cup"),
            Lookup::Found(CapabilityId::String(StringCap::CURSOR_ADDRESS))
        ));
        assert_eq!(lookup_all(NameNamespace::Termcap, "ML").len(), 2);
        assert!(matches!(
            lookup(NameNamespace::Termcap, "ML"),
            Lookup::Ambiguous(candidates) if candidates.len() == 2
        ));
    }
}
