//! Filesystem databases, ncurses search paths, and portable single-entry transport.

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
#[cfg(any(unix, test))]
use std::sync::atomic::{AtomicU64, Ordering};

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use atomic_write_file::AtomicWriteFile;

use crate::binary::{self, EncodeOptions};
use crate::error::DatabaseError;
use crate::model::{Entry, validate_terminal_name};
use crate::resolve::{EntryProvider, ProviderError};

#[cfg(any(unix, test))]
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// On-disk subdirectory naming convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DirectoryLayout {
    /// Select the native convention for the target platform.
    Auto,
    /// Use the first byte as a one-character directory.
    Letter,
    /// Use the first byte as a two-digit hexadecimal directory.
    Hex,
}

/// Options controlling installation of a compiled entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct InstallOptions {
    layout: DirectoryLayout,
    aliases: bool,
    encode_options: EncodeOptions,
}

impl InstallOptions {
    /// Creates options with automatic layout and alias installation.
    pub const fn new() -> Self {
        Self {
            layout: DirectoryLayout::Auto,
            aliases: true,
            encode_options: EncodeOptions::new(),
        }
    }

    /// Returns the requested directory layout.
    pub const fn layout(self) -> DirectoryLayout {
        self.layout
    }

    /// Reports whether aliases will be installed.
    pub const fn aliases(self) -> bool {
        self.aliases
    }

    /// Returns the options used to encode the installed entry.
    pub const fn encode_options(self) -> EncodeOptions {
        self.encode_options
    }

    /// Replaces the directory layout.
    pub const fn with_layout(mut self, value: DirectoryLayout) -> Self {
        self.layout = value;
        self
    }

    /// Enables or disables alias installation.
    pub const fn with_aliases(mut self, value: bool) -> Self {
        self.aliases = value;
        self
    }

    /// Replaces the options used to encode the installed entry.
    pub const fn with_encode_options(mut self, value: EncodeOptions) -> Self {
        self.encode_options = value;
        self
    }
}

/// Method used to materialize an alias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AliasMethod {
    /// Alias shares the primary file's inode.
    HardLink,
    /// Alias was written as an atomic independent copy.
    AtomicCopy,
}

/// One installed alias and its resulting path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasInstall {
    name: String,
    path: PathBuf,
    method: AliasMethod,
}

impl AliasInstall {
    /// Returns the alias name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the installed alias path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns how the alias was materialized.
    pub const fn method(&self) -> AliasMethod {
        self.method
    }
}

/// Result of installing a primary entry and its aliases.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallReport {
    primary: PathBuf,
    aliases: Vec<AliasInstall>,
    changed: bool,
}

impl InstallReport {
    /// Returns the primary compiled-entry path.
    pub fn primary(&self) -> &Path {
        &self.primary
    }

    /// Returns installed alias records.
    pub fn aliases(&self) -> &[AliasInstall] {
        &self.aliases
    }

    /// Reports whether the primary file's bytes changed.
    pub const fn changed(&self) -> bool {
        self.changed
    }
}

/// A decoded entry together with its exact load provenance.
#[derive(Debug, Clone)]
pub struct LoadReport {
    entry: Entry,
    magic: binary::Magic,
    origin: LoadOrigin,
}

/// Provenance of a database load.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadOrigin {
    /// An entry embedded directly in TERMINFO.
    Inline {
        /// Encoding used by the inline transport.
        encoding: TransportEncoding,
    },
    /// A file selected from an ncurses-style directory database.
    Directory {
        /// Exact compiled-entry path that was read.
        path: PathBuf,
        /// Directory naming layout that matched.
        layout: DirectoryLayout,
    },
}

impl LoadReport {
    /// Returns the decoded entry.
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// Consumes the report and returns the decoded entry.
    pub fn into_entry(self) -> Entry {
        self.entry
    }

    /// Returns the compiled numeric format that was decoded.
    pub const fn magic(&self) -> binary::Magic {
        self.magic
    }

    /// Returns the selected transport or directory origin.
    pub fn origin(&self) -> &LoadOrigin {
        &self.origin
    }

    /// Returns the selected path for a directory load.
    pub fn path(&self) -> Option<&Path> {
        match &self.origin {
            LoadOrigin::Directory { path, .. } => Some(path),
            LoadOrigin::Inline { .. } => None,
        }
    }

    /// Returns the matched layout for a directory load.
    pub const fn layout(&self) -> Option<DirectoryLayout> {
        match self.origin {
            LoadOrigin::Directory { layout, .. } => Some(layout),
            LoadOrigin::Inline { .. } => None,
        }
    }
}

/// Common read-only interface implemented by database backends.
pub trait DatabaseBackend {
    /// Loads a named entry and includes backend provenance.
    fn load_report(&self, name: &str) -> Result<LoadReport, DatabaseError>;
    /// Lists validated entry filenames known to the backend.
    fn names(&self) -> Result<Vec<String>, DatabaseError>;
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            layout: DirectoryLayout::Auto,
            aliases: true,
            encode_options: EncodeOptions::new(),
        }
    }
}

/// Filesystem-backed ncurses-style terminfo database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryDatabase {
    root: PathBuf,
}

impl DirectoryDatabase {
    /// Creates a database rooted at the supplied directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    /// Returns the database root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Loads a primary name or alias.
    pub fn load(&self, name: &str) -> Result<Entry, DatabaseError> {
        self.load_report(name).map(LoadReport::into_entry)
    }

    /// Loads an entry and reports its exact path and matched layout.
    pub fn load_report(&self, name: &str) -> Result<LoadReport, DatabaseError> {
        let (path, bytes, layout) = self.load_raw(name)?;
        let document = binary::decode(&bytes).map_err(DatabaseError::Decode)?;
        let magic = document.magic();
        let entry = document.into_entry();
        if entry.names().primary() != name
            && !entry.names().aliases().iter().any(|alias| alias == name)
        {
            return Err(DatabaseError::NameMismatch {
                requested: name.to_string(),
                decoded: entry.names().primary().to_string(),
            });
        }
        Ok(LoadReport {
            entry,
            magic,
            origin: LoadOrigin::Directory { path, layout },
        })
    }

    /// Loads the compiled bytes without decoding them.
    pub fn load_bytes(&self, name: &str) -> Result<Vec<u8>, DatabaseError> {
        self.load_raw(name).map(|(_, bytes, _)| bytes)
    }

    fn load_raw(&self, name: &str) -> Result<(PathBuf, Vec<u8>, DirectoryLayout), DatabaseError> {
        validate_db_name(name)?;
        for (path, layout) in candidate_paths(&self.root, name) {
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(DatabaseError::UntrustedSymlink(path));
                }
                Ok(metadata) if !metadata.is_file() => continue,
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(DatabaseError::Io(error)),
            }
            match fs::read(&path) {
                Ok(bytes) => return Ok((path, bytes, layout)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(DatabaseError::Io(error)),
            }
        }
        Err(DatabaseError::NotFound(name.to_string()))
    }

    /// Atomically installs an entry and optionally its aliases.
    pub fn install(
        &self,
        entry: &Entry,
        options: InstallOptions,
    ) -> Result<InstallReport, DatabaseError> {
        let bytes = entry
            .to_bytes_with(options.encode_options())
            .map_err(|error| DatabaseError::UnsupportedBackend(error.to_string()))?;
        let primary = entry.names().primary();
        validate_db_name(primary)?;
        let layout = resolved_layout(options.layout);
        let path = entry_path(&self.root, primary, layout);
        let parent = path
            .parent()
            .ok_or_else(|| DatabaseError::InvalidName(primary.to_string()))?;
        ensure_entry_directory(&self.root, parent)?;
        reject_symlink(&path)?;
        let changed = !fs::read(&path).is_ok_and(|current| current == bytes);
        if changed {
            atomic_write(&path, &bytes)?;
        }

        let mut installed_aliases = Vec::new();
        if options.aliases {
            for alias in entry.names().aliases() {
                validate_db_name(alias)?;
                let alias_path = entry_path(&self.root, alias, layout);
                if let Some(parent) = alias_path.parent() {
                    ensure_entry_directory(&self.root, parent)?;
                }
                let method = install_alias(&path, &alias_path, &bytes)?;
                installed_aliases.push(AliasInstall {
                    name: alias.to_string(),
                    path: alias_path,
                    method,
                });
            }
        }
        Ok(InstallReport {
            primary: path,
            aliases: installed_aliases,
            changed,
        })
    }
}

impl DatabaseBackend for DirectoryDatabase {
    fn load_report(&self, name: &str) -> Result<LoadReport, DatabaseError> {
        DirectoryDatabase::load_report(self, name)
    }

    fn names(&self) -> Result<Vec<String>, DatabaseError> {
        let mut names = Vec::new();
        if !self.root.exists() {
            return Ok(names);
        }
        for directory in fs::read_dir(&self.root)? {
            let directory = directory?;
            if directory.file_type()?.is_symlink() || !directory.file_type()?.is_dir() {
                continue;
            }
            for item in fs::read_dir(directory.path())? {
                let item = item?;
                if item.file_type()?.is_symlink() || !item.file_type()?.is_file() {
                    continue;
                }
                if let Some(name) = item.file_name().to_str()
                    && validate_db_name(name).is_ok()
                    && !names.iter().any(|candidate| candidate == name)
                {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        Ok(names)
    }
}

impl EntryProvider for DirectoryDatabase {
    fn get(&self, name: &str) -> Result<Option<Entry>, ProviderError> {
        match self.load(name) {
            Ok(entry) => Ok(Some(entry)),
            Err(DatabaseError::NotFound(_)) => Ok(None),
            Err(error) => Err(ProviderError::new(error.to_string())),
        }
    }
}

/// Ordered, duplicate-free set of terminfo directory roots.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchPath {
    roots: Vec<PathBuf>,
}

impl SearchPath {
    /// Creates an empty search path.
    pub fn new() -> Self {
        Self::default()
    }
    /// Appends a root unless it is already present.
    pub fn push(&mut self, root: impl Into<PathBuf>) {
        let root = root.into();
        if !self.roots.contains(&root) {
            self.roots.push(root);
        }
    }
    /// Returns roots in lookup priority order.
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Constructs the ncurses-style directory order from the environment.
    pub fn from_env() -> Self {
        let mut result = Self::new();
        if let Some(value) = env::var_os("TERMINFO") {
            let text = value.to_string_lossy();
            if !text.starts_with("hex:") && !text.starts_with("b64:") && !text.is_empty() {
                result.push(PathBuf::from(value));
            }
        }
        if !cfg!(windows)
            && let Some(home) = env::var_os("HOME")
        {
            result.push(PathBuf::from(home).join(".terminfo"));
        }
        let defaults = default_roots();
        if let Some(value) = env::var_os("TERMINFO_DIRS") {
            for root in env::split_paths(&value) {
                if root.as_os_str().is_empty() {
                    for item in &defaults {
                        result.push(item);
                    }
                } else {
                    result.push(root);
                }
            }
        }
        for root in defaults {
            result.push(root);
        }
        result
    }

    /// Loads the first matching entry.
    pub fn load(&self, name: &str) -> Result<Entry, DatabaseError> {
        self.load_report(name).map(LoadReport::into_entry)
    }

    /// Loads an entry and reports the selected root, path, and layout.
    pub fn load_report(&self, name: &str) -> Result<LoadReport, DatabaseError> {
        validate_db_name(name)?;
        for root in &self.roots {
            match DirectoryDatabase::new(root).load_report(name) {
                Ok(report) => return Ok(report),
                Err(DatabaseError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        Err(DatabaseError::NotFound(name.to_string()))
    }
}

impl EntryProvider for SearchPath {
    fn get(&self, name: &str) -> Result<Option<Entry>, ProviderError> {
        match self.load(name) {
            Ok(entry) => Ok(Some(entry)),
            Err(DatabaseError::NotFound(_)) => Ok(None),
            Err(error) => Err(ProviderError::new(error.to_string())),
        }
    }
}

/// Load an entry using `TERMINFO=hex:...` / `b64:...` or the ncurses directory search path.
pub fn load_from_env(name: &str) -> Result<Entry, DatabaseError> {
    load_from_env_report(name).map(LoadReport::into_entry)
}

/// Loads an entry using the environment and reports its exact transport or
/// directory origin.
pub fn load_from_env_report(name: &str) -> Result<LoadReport, DatabaseError> {
    if let Ok(value) = env::var("TERMINFO") {
        if let Some(encoded) = value.strip_prefix("hex:") {
            let document = binary::decode(&decode_hex(encoded)?).map_err(DatabaseError::Decode)?;
            let magic = document.magic();
            let entry = validate_loaded_name(name, document.into_entry())?;
            return Ok(LoadReport {
                entry,
                magic,
                origin: LoadOrigin::Inline {
                    encoding: TransportEncoding::Hex,
                },
            });
        }
        if let Some(encoded) = value.strip_prefix("b64:") {
            let document =
                binary::decode(&decode_base64(encoded)?).map_err(DatabaseError::Decode)?;
            let magic = document.magic();
            let entry = validate_loaded_name(name, document.into_entry())?;
            return Ok(LoadReport {
                entry,
                magic,
                origin: LoadOrigin::Inline {
                    encoding: TransportEncoding::Base64,
                },
            });
        }
    }
    SearchPath::from_env().load_report(name)
}

/// Portable inline representation used in TERMINFO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportEncoding {
    /// Lowercase hexadecimal.
    Hex,
    /// RFC 4648 base64.
    Base64,
}

/// Encodes one entry as a prefixed inline transport value.
pub fn encode_transport(
    entry: &Entry,
    encoding: TransportEncoding,
) -> Result<String, DatabaseError> {
    encode_transport_with(entry, encoding, EncodeOptions::new())
}

/// Encodes one entry as a prefixed inline transport value with explicit
/// compiled-output options.
pub fn encode_transport_with(
    entry: &Entry,
    encoding: TransportEncoding,
    options: EncodeOptions,
) -> Result<String, DatabaseError> {
    let bytes = entry
        .to_bytes_with(options)
        .map_err(|error| DatabaseError::UnsupportedBackend(error.to_string()))?;
    Ok(match encoding {
        TransportEncoding::Hex => format!("hex:{}", encode_hex(&bytes)),
        TransportEncoding::Base64 => format!("b64:{}", encode_base64(&bytes)),
    })
}

/// Returns the default writable per-user installation root, when available.
pub fn default_install_root() -> Option<PathBuf> {
    if let Some(root) = env::var_os("TERMINFO").filter(|value| !value.is_empty()) {
        let text = root.to_string_lossy();
        if !text.starts_with("hex:") && !text.starts_with("b64:") {
            return Some(root.into());
        }
    }
    if cfg!(windows) {
        None
    } else {
        env::var_os("HOME").map(|home| PathBuf::from(home).join(".terminfo"))
    }
}

fn default_roots() -> Vec<PathBuf> {
    if cfg!(windows) {
        Vec::new()
    } else {
        [
            "/etc/terminfo",
            "/lib/terminfo",
            "/usr/share/terminfo",
            "/usr/lib/terminfo",
        ]
        .iter()
        .map(PathBuf::from)
        .collect()
    }
}
fn validate_db_name(name: &str) -> Result<(), DatabaseError> {
    validate_terminal_name(name).map_err(|_| DatabaseError::InvalidName(name.to_string()))
}

fn validate_loaded_name(name: &str, entry: Entry) -> Result<Entry, DatabaseError> {
    if entry.names().primary() == name || entry.names().aliases().iter().any(|alias| alias == name)
    {
        Ok(entry)
    } else {
        Err(DatabaseError::NameMismatch {
            requested: name.to_string(),
            decoded: entry.names().primary().to_string(),
        })
    }
}
fn resolved_layout(layout: DirectoryLayout) -> DirectoryLayout {
    match layout {
        DirectoryLayout::Auto if cfg!(any(windows, target_os = "macos")) => DirectoryLayout::Hex,
        DirectoryLayout::Auto => DirectoryLayout::Letter,
        other => other,
    }
}
fn entry_path(root: &Path, name: &str, layout: DirectoryLayout) -> PathBuf {
    let first = name.as_bytes()[0];
    let directory = match layout {
        DirectoryLayout::Letter => char::from(first).to_string(),
        DirectoryLayout::Hex | DirectoryLayout::Auto => format!("{first:02x}"),
    };
    root.join(directory).join(name)
}
fn candidate_paths(root: &Path, name: &str) -> [(PathBuf, DirectoryLayout); 3] {
    let first = name.as_bytes()[0];
    [
        (
            entry_path(root, name, DirectoryLayout::Letter),
            DirectoryLayout::Letter,
        ),
        (
            root.join(format!("{first:02x}")).join(name),
            DirectoryLayout::Hex,
        ),
        (
            root.join(format!("{first:02X}")).join(name),
            DirectoryLayout::Hex,
        ),
    ]
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), DatabaseError> {
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.commit()?;
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), DatabaseError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(DatabaseError::UntrustedSymlink(path.to_path_buf()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DatabaseError::Io(error)),
    }
}

fn ensure_entry_directory(root: &Path, directory: &Path) -> Result<(), DatabaseError> {
    fs::create_dir_all(root)?;
    match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(DatabaseError::UntrustedSymlink(directory.to_path_buf()))
        }
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(DatabaseError::InvalidName(directory.display().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(directory).map_err(DatabaseError::Io)
        }
        Err(error) => Err(DatabaseError::Io(error)),
    }
}

fn install_alias(primary: &Path, alias: &Path, bytes: &[u8]) -> Result<AliasMethod, DatabaseError> {
    if let Ok(metadata) = fs::symlink_metadata(alias)
        && metadata.file_type().is_symlink()
    {
        return Err(DatabaseError::UntrustedSymlink(alias.to_path_buf()));
    }
    if !alias.exists() && fs::hard_link(primary, alias).is_ok() {
        return Ok(AliasMethod::HardLink);
    }
    #[cfg(unix)]
    {
        let parent = alias
            .parent()
            .ok_or_else(|| DatabaseError::InvalidName(alias.display().to_string()))?;
        let suffix = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".terminfokit-alias-{}-{suffix}.tmp",
            std::process::id()
        ));
        if fs::hard_link(primary, &temporary).is_ok() {
            match fs::rename(&temporary, alias) {
                Ok(()) => return Ok(AliasMethod::HardLink),
                Err(_) => {
                    let _ = fs::remove_file(&temporary);
                }
            }
        }
    }
    atomic_write(alias, bytes)?;
    Ok(AliasMethod::AtomicCopy)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 15)]));
    }
    output
}
fn decode_hex(value: &str) -> Result<Vec<u8>, DatabaseError> {
    if !value.len().is_multiple_of(2) {
        return Err(DatabaseError::UnsupportedBackend(
            "odd-length hex transport".into(),
        ));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_digit(pair[0])
                .ok_or_else(|| DatabaseError::UnsupportedBackend("invalid hex transport".into()))?;
            let low = hex_digit(pair[1])
                .ok_or_else(|| DatabaseError::UnsupportedBackend("invalid hex transport".into()))?;
            Ok((high << 4) | low)
        })
        .collect()
}
fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
fn encode_base64(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(char::from(BASE64[((value >> 18) & 63) as usize]));
        output.push(char::from(BASE64[((value >> 12) & 63) as usize]));
        output.push(if chunk.len() > 1 {
            char::from(BASE64[((value >> 6) & 63) as usize])
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            char::from(BASE64[(value & 63) as usize])
        } else {
            '='
        });
    }
    output
}
fn decode_base64(value: &str) -> Result<Vec<u8>, DatabaseError> {
    if !value.len().is_multiple_of(4) {
        return Err(DatabaseError::UnsupportedBackend(
            "invalid base64 transport length".into(),
        ));
    }
    let mut output = Vec::with_capacity(value.len() / 4 * 3);
    let chunks = value.len() / 4;
    for (index, chunk) in value.as_bytes().chunks_exact(4).enumerate() {
        let final_chunk = index + 1 == chunks;
        let a = b64_digit(chunk[0])?;
        let b = b64_digit(chunk[1])?;
        let c = if chunk[2] == b'=' {
            0
        } else {
            b64_digit(chunk[2])?
        };
        let d = if chunk[3] == b'=' {
            0
        } else {
            b64_digit(chunk[3])?
        };
        if (chunk[2] == b'=' || chunk[3] == b'=') && !final_chunk {
            return Err(DatabaseError::UnsupportedBackend(
                "base64 padding is only valid in the final quartet".into(),
            ));
        }
        if chunk[2] == b'=' && (chunk[3] != b'=' || b & 0x0f != 0) {
            return Err(DatabaseError::UnsupportedBackend(
                "non-canonical base64 padding".into(),
            ));
        }
        if chunk[3] == b'=' && chunk[2] != b'=' && c & 0x03 != 0 {
            return Err(DatabaseError::UnsupportedBackend(
                "non-canonical base64 padding".into(),
            ));
        }
        let bits = (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        output.push((bits >> 16) as u8);
        if chunk[2] != b'=' {
            output.push((bits >> 8) as u8);
        } else if chunk[3] != b'=' {
            return Err(DatabaseError::UnsupportedBackend(
                "invalid base64 padding".into(),
            ));
        }
        if chunk[3] != b'=' {
            output.push(bits as u8);
        }
    }
    Ok(output)
}
fn b64_digit(byte: u8) -> Result<u8, DatabaseError> {
    BASE64
        .iter()
        .position(|candidate| *candidate == byte)
        .map(|value| value as u8)
        .ok_or_else(|| DatabaseError::UnsupportedBackend("invalid base64 transport".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EntryBuilder;

    #[test]
    fn transport_round_trip() {
        let entry = EntryBuilder::new("portable").unwrap().build();
        let bytes = entry.to_bytes().unwrap();
        assert_eq!(decode_hex(&encode_hex(&bytes)).unwrap(), bytes);
        assert_eq!(decode_base64(&encode_base64(&bytes)).unwrap(), bytes);
        assert_eq!(decode_base64("AA==").unwrap(), [0]);
        assert_eq!(decode_base64("AAA=").unwrap(), [0, 0]);
        for invalid in ["AA==AAAA", "AB==", "AAB=", "AA=A"] {
            assert!(
                decode_base64(invalid).is_err(),
                "{invalid:?} must be rejected"
            );
        }
    }

    #[test]
    fn directory_install_loads_hex_and_alias_and_rejects_traversal() {
        let root = std::env::temp_dir().join(format!(
            "terminfokit-database-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let entry = EntryBuilder::new("portable")
            .unwrap()
            .alias("portable-alias")
            .unwrap()
            .build();
        let database = DirectoryDatabase::new(&root);
        database
            .install(
                &entry,
                InstallOptions::new().with_layout(DirectoryLayout::Hex),
            )
            .unwrap();
        database
            .install(
                &entry,
                InstallOptions::new().with_layout(DirectoryLayout::Hex),
            )
            .unwrap();
        assert_eq!(
            database.load("portable").unwrap().names().primary(),
            "portable"
        );
        assert_eq!(
            database.load("portable-alias").unwrap().names().primary(),
            "portable"
        );
        assert!(matches!(
            database.load("../portable"),
            Err(DatabaseError::InvalidName(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn search_report_preserves_priority_and_origin() {
        let suffix = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "terminfokit-search-report-{}-{suffix}",
            std::process::id()
        ));
        let first = base.join("first");
        let second = base.join("second");
        let first_entry = EntryBuilder::new("priority")
            .unwrap()
            .number(crate::caps::NumericCap::COLUMNS, 80)
            .unwrap()
            .build();
        let second_entry = EntryBuilder::new("priority")
            .unwrap()
            .number(crate::caps::NumericCap::COLUMNS, 132)
            .unwrap()
            .build();
        DirectoryDatabase::new(&first)
            .install(&first_entry, InstallOptions::new())
            .unwrap();
        DirectoryDatabase::new(&second)
            .install(&second_entry, InstallOptions::new())
            .unwrap();
        let mut search = SearchPath::new();
        search.push(&first);
        search.push(&second);
        let report = search.load_report("priority").unwrap();
        assert!(report.path().unwrap().starts_with(&first));
        assert!(matches!(report.origin(), LoadOrigin::Directory { .. }));
        assert_eq!(
            report.entry().number(crate::caps::NumericCap::COLUMNS),
            crate::CapabilityState::Value(crate::Number::new(80).unwrap())
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn installation_rejects_symlinked_entry_directories() {
        use std::os::unix::fs::symlink;

        let suffix = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "terminfokit-database-symlink-{}-{suffix}",
            std::process::id()
        ));
        let outside = std::env::temp_dir().join(format!(
            "terminfokit-database-outside-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::create_dir_all(&outside).unwrap();
        symlink(&outside, root.join("p")).unwrap();

        let entry = EntryBuilder::new("portable").unwrap().build();
        let error = DirectoryDatabase::new(&root)
            .install(
                &entry,
                InstallOptions::new().with_layout(DirectoryLayout::Letter),
            )
            .unwrap_err();
        assert!(matches!(error, DatabaseError::UntrustedSymlink(_)));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
}
