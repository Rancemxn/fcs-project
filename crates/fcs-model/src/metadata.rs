//! Immutable canonical metadata, logical resource, and sync values.
//!
//! This module deliberately contains no source-AST or filesystem concerns.  A
//! source adapter may validate an authoring path before constructing a resource,
//! but the canonical descriptor carries only the logical resource identity and
//! declared metadata.  Input bytes and computed hashes belong to I5.

use std::collections::BTreeMap;
use std::fmt;

use crate::{AudioOffset, Beat};

/// An sRGB color value carried by canonical typed custom data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanonicalColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl CanonicalColor {
    pub const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub const fn red(self) -> u8 {
        self.red
    }

    pub const fn green(self) -> u8 {
        self.green
    }

    pub const fn blue(self) -> u8 {
        self.blue
    }

    pub const fn alpha(self) -> u8 {
        self.alpha
    }
}

/// The type tag used to validate homogeneous canonical arrays.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalValueType {
    Null,
    Bool,
    Int,
    Float,
    String,
    Time,
    Beat,
    Color,
    ResourceReference,
    ContributorReference,
    Array(Box<Self>),
    Object,
}

/// An ordered object used by typed custom data.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalObject {
    entries: Vec<CanonicalObjectEntry>,
}

impl CanonicalObject {
    pub fn new(entries: Vec<CanonicalObjectEntry>) -> Result<Self, CanonicalObjectError> {
        let mut keys = std::collections::BTreeSet::new();
        for entry in &entries {
            if !keys.insert(entry.key.as_str()) {
                return Err(CanonicalObjectError::DuplicateKey(entry.key.clone()));
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[CanonicalObjectEntry] {
        &self.entries
    }

    pub fn get(&self, key: &str) -> Option<&CanonicalValue> {
        self.entries
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.value)
    }
}

/// One source-order-preserving custom object entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalObjectEntry {
    key: String,
    value: CanonicalValue,
}

impl CanonicalObjectEntry {
    pub fn new(key: impl Into<String>, value: CanonicalValue) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &CanonicalValue {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalObjectError {
    DuplicateKey(String),
}

impl fmt::Display for CanonicalObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateKey(key) => write!(formatter, "duplicate canonical object key {key}"),
        }
    }
}

impl std::error::Error for CanonicalObjectError {}

/// A value permitted in FCS typed custom data.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Time(f64),
    Beat(Beat),
    Color(CanonicalColor),
    ResourceReference(String),
    ContributorReference(String),
    Array {
        element_type: CanonicalValueType,
        values: Vec<Self>,
    },
    Object(CanonicalObject),
}

impl CanonicalValue {
    pub fn value_type(&self) -> CanonicalValueType {
        match self {
            Self::Null => CanonicalValueType::Null,
            Self::Bool(_) => CanonicalValueType::Bool,
            Self::Int(_) => CanonicalValueType::Int,
            Self::Float(_) => CanonicalValueType::Float,
            Self::String(_) => CanonicalValueType::String,
            Self::Time(_) => CanonicalValueType::Time,
            Self::Beat(_) => CanonicalValueType::Beat,
            Self::Color(_) => CanonicalValueType::Color,
            Self::ResourceReference(_) => CanonicalValueType::ResourceReference,
            Self::ContributorReference(_) => CanonicalValueType::ContributorReference,
            Self::Array { element_type, .. } => {
                CanonicalValueType::Array(Box::new(element_type.clone()))
            }
            Self::Object(_) => CanonicalValueType::Object,
        }
    }

    pub fn typed_array(
        element_type: CanonicalValueType,
        values: Vec<Self>,
    ) -> Result<Self, CanonicalArrayError> {
        if values
            .iter()
            .any(|value| value.value_type() != element_type)
        {
            return Err(CanonicalArrayError::TypeMismatch);
        }
        Ok(Self::Array {
            element_type,
            values,
        })
    }

    pub fn array(values: Vec<Self>) -> Result<Self, CanonicalArrayError> {
        let Some(first) = values.first() else {
            return Err(CanonicalArrayError::MissingElementType);
        };
        Self::typed_array(first.value_type(), values)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalArrayError {
    MissingElementType,
    TypeMismatch,
}

/// A validated contributor declaration. Field order is not semantic and is
/// therefore stored in a deterministic key order.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalContributor {
    id: String,
    fields: BTreeMap<String, CanonicalValue>,
}

impl CanonicalContributor {
    pub fn new(id: impl Into<String>, fields: BTreeMap<String, CanonicalValue>) -> Self {
        Self {
            id: id.into(),
            fields,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn fields(&self) -> &BTreeMap<String, CanonicalValue> {
        &self.fields
    }

    pub fn field(&self, name: &str) -> Option<&CanonicalValue> {
        self.fields.get(name)
    }
}

/// An ordered display credit.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCredit {
    role: String,
    label: Option<String>,
    contributors: Vec<String>,
}

impl CanonicalCredit {
    pub fn new(role: impl Into<String>, label: Option<String>, contributors: Vec<String>) -> Self {
        Self {
            role: role.into(),
            label,
            contributors,
        }
    }

    pub fn role(&self) -> &str {
        &self.role
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn contributors(&self) -> &[String] {
        &self.contributors
    }
}

/// The resource kinds defined by FCS Core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalResourceKind {
    Audio,
    Image,
    Font,
    Texture,
    Path,
    Shader,
    Binary,
}

/// A declared SHA-256 value. It is a declaration only; I5 computes and checks
/// the digest of workspace input bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeclaredSha256([u8; 32]);

impl DeclaredSha256 {
    pub fn from_lower_hex(hex: &str) -> Option<Self> {
        if hex.len() != 64 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let mut bytes = [0; 32];
        for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
            if pair.iter().any(u8::is_ascii_uppercase) {
                return None;
            }
            bytes[index] = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
        }
        Some(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// A canonical resource declaration without workspace path or payload bytes.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalResource {
    id: String,
    kind: CanonicalResourceKind,
    media_type: String,
    declared_sha256: Option<DeclaredSha256>,
    metadata: BTreeMap<String, CanonicalValue>,
}

impl CanonicalResource {
    pub fn new(
        id: impl Into<String>,
        kind: CanonicalResourceKind,
        media_type: impl Into<String>,
        declared_sha256: Option<DeclaredSha256>,
        metadata: BTreeMap<String, CanonicalValue>,
    ) -> Self {
        Self {
            id: id.into(),
            kind,
            media_type: media_type.into(),
            declared_sha256,
            metadata,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn kind(&self) -> CanonicalResourceKind {
        self.kind
    }

    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    pub const fn declared_sha256(&self) -> Option<DeclaredSha256> {
        self.declared_sha256
    }

    pub fn metadata(&self) -> &BTreeMap<String, CanonicalValue> {
        &self.metadata
    }
}

/// The artwork graph, currently consisting of the optional primary image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalArtwork {
    primary: Option<String>,
}

impl CanonicalArtwork {
    pub const fn new(primary: Option<String>) -> Self {
        Self { primary }
    }

    pub fn primary(&self) -> Option<&str> {
        self.primary.as_deref()
    }
}

/// An audio-time half-open preview interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalPreview {
    start_seconds: f64,
    end_seconds: f64,
}

impl CanonicalPreview {
    pub fn new(start_seconds: f64, end_seconds: f64) -> Option<Self> {
        (start_seconds.is_finite()
            && end_seconds.is_finite()
            && start_seconds >= 0.0
            && end_seconds > start_seconds)
            .then_some(Self {
                start_seconds,
                end_seconds,
            })
    }

    pub const fn start_seconds(self) -> f64 {
        self.start_seconds
    }

    pub const fn end_seconds(self) -> f64 {
        self.end_seconds
    }
}

/// The single-clock audio synchronization boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalSync {
    primary_audio: Option<String>,
    audio_offset: AudioOffset,
    preview: Option<CanonicalPreview>,
}

impl CanonicalSync {
    pub fn new(
        primary_audio: Option<String>,
        audio_offset: AudioOffset,
        preview: Option<CanonicalPreview>,
    ) -> Self {
        Self {
            primary_audio,
            audio_offset,
            preview,
        }
    }

    pub fn primary_audio(&self) -> Option<&str> {
        self.primary_audio.as_deref()
    }

    pub const fn audio_offset(&self) -> AudioOffset {
        self.audio_offset
    }

    pub const fn preview(&self) -> Option<CanonicalPreview> {
        self.preview
    }
}

/// The complete I3.3 canonical metadata graph.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalMetadata {
    meta: Option<BTreeMap<String, CanonicalValue>>,
    contributors: BTreeMap<String, CanonicalContributor>,
    credits: Vec<CanonicalCredit>,
    resources: BTreeMap<String, CanonicalResource>,
    artwork: Option<CanonicalArtwork>,
    sync: Option<CanonicalSync>,
}

impl CanonicalMetadata {
    pub fn new(
        meta: Option<BTreeMap<String, CanonicalValue>>,
        contributors: BTreeMap<String, CanonicalContributor>,
        credits: Vec<CanonicalCredit>,
        resources: BTreeMap<String, CanonicalResource>,
        artwork: Option<CanonicalArtwork>,
        sync: Option<CanonicalSync>,
    ) -> Self {
        Self {
            meta,
            contributors,
            credits,
            resources,
            artwork,
            sync,
        }
    }

    pub fn meta(&self) -> Option<&BTreeMap<String, CanonicalValue>> {
        self.meta.as_ref()
    }

    pub fn contributors(&self) -> &BTreeMap<String, CanonicalContributor> {
        &self.contributors
    }

    pub fn credits(&self) -> &[CanonicalCredit] {
        &self.credits
    }

    pub fn resources(&self) -> &BTreeMap<String, CanonicalResource> {
        &self.resources
    }

    pub fn artwork(&self) -> Option<&CanonicalArtwork> {
        self.artwork.as_ref()
    }

    pub fn sync(&self) -> Option<&CanonicalSync> {
        self.sync.as_ref()
    }
}
