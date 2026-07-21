//! Immutable canonical metadata, resource, and sync values.
//!
//! This module deliberately contains no source-AST or filesystem concerns.  A
//! source adapter may validate an authoring path before constructing a resource,
//! but canonical values carry only logical resource identity, declared metadata,
//! computed hashes, and exact opaque bytes. They never retain workspace paths.

use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::{AudioOffset, Beat, TempoError};

/// A linear RGBA color value carried by canonical typed custom data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalColor {
    red: f64,
    green: f64,
    blue: f64,
    alpha: f64,
}

impl CanonicalColor {
    /// Constructs a canonical color from linear RGBA Float64 components.
    pub fn from_linear(components: [f64; 4]) -> Result<Self, CanonicalColorError> {
        if components
            .iter()
            .all(|component| component.is_finite() && (0.0..=1.0).contains(component))
        {
            Ok(Self {
                red: components[0],
                green: components[1],
                blue: components[2],
                alpha: components[3],
            })
        } else {
            Err(CanonicalColorError::InvalidComponent)
        }
    }

    /// Compatibility constructor for encoded 8-bit sRGB input. The stored
    /// components are still canonical linear values.
    pub fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self::from_linear([
            srgb_to_linear(red),
            srgb_to_linear(green),
            srgb_to_linear(blue),
            f64::from(alpha) / 255.0,
        ])
        .expect("8-bit sRGB conversion produces valid canonical components")
    }

    pub const fn red(self) -> f64 {
        self.red
    }

    pub const fn green(self) -> f64 {
        self.green
    }

    pub const fn blue(self) -> f64 {
        self.blue
    }

    pub const fn alpha(self) -> f64 {
        self.alpha
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalColorError {
    InvalidComponent,
}

fn srgb_to_linear(value: u8) -> f64 {
    let encoded = f64::from(value) / 255.0;
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
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

/// A validated contributor declaration with FCBC-ready typed fields.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalContributor {
    id: String,
    name: String,
    aliases: Vec<String>,
    identifiers: CanonicalObject,
}

impl CanonicalContributor {
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        aliases: Vec<String>,
        identifiers: CanonicalObject,
    ) -> Result<Self, CanonicalContributorError> {
        let id = id.into();
        if id.is_empty() {
            return Err(CanonicalContributorError::EmptyId);
        }
        let name = name.into();
        if name.is_empty() {
            return Err(CanonicalContributorError::EmptyName);
        }
        if let Some(entry) = identifiers
            .entries()
            .iter()
            .find(|entry| !matches!(entry.value(), CanonicalValue::String(_)))
        {
            return Err(CanonicalContributorError::NonStringIdentifier {
                key: entry.key().to_owned(),
            });
        }
        Ok(Self {
            id,
            name,
            aliases,
            identifiers,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    pub const fn identifiers(&self) -> &CanonicalObject {
        &self.identifiers
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalContributorError {
    EmptyId,
    EmptyName,
    NonStringIdentifier { key: String },
}

/// One of the twelve standard FCS 5 credit roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalStandardCreditRole {
    Composer,
    Arranger,
    Lyricist,
    Vocalist,
    Instrumentalist,
    Mixer,
    Mastering,
    Charter,
    Illustrator,
    Designer,
    Programmer,
    Publisher,
}

impl CanonicalStandardCreditRole {
    pub const ALL: [Self; 12] = [
        Self::Composer,
        Self::Arranger,
        Self::Lyricist,
        Self::Vocalist,
        Self::Instrumentalist,
        Self::Mixer,
        Self::Mastering,
        Self::Charter,
        Self::Illustrator,
        Self::Designer,
        Self::Programmer,
        Self::Publisher,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Composer => "composer",
            Self::Arranger => "arranger",
            Self::Lyricist => "lyricist",
            Self::Vocalist => "vocalist",
            Self::Instrumentalist => "instrumentalist",
            Self::Mixer => "mixer",
            Self::Mastering => "mastering",
            Self::Charter => "charter",
            Self::Illustrator => "illustrator",
            Self::Designer => "designer",
            Self::Programmer => "programmer",
            Self::Publisher => "publisher",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == value)
    }
}

/// A classified standard role or an exact custom ASCII role ID.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalCreditRole {
    Standard(CanonicalStandardCreditRole),
    Custom(String),
}

impl CanonicalCreditRole {
    pub fn parse(value: impl Into<String>) -> Result<Self, CanonicalCreditRoleError> {
        let value = value.into();
        if let Some(role) = CanonicalStandardCreditRole::from_name(&value) {
            return Ok(Self::Standard(role));
        }
        if value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic())
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            Ok(Self::Custom(value))
        } else {
            Err(CanonicalCreditRoleError::InvalidCustomId(value))
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Standard(role) => role.as_str(),
            Self::Custom(role) => role,
        }
    }

    pub const fn standard(&self) -> Option<CanonicalStandardCreditRole> {
        match self {
            Self::Standard(role) => Some(*role),
            Self::Custom(_) => None,
        }
    }

    pub fn custom(&self) -> Option<&str> {
        match self {
            Self::Standard(_) => None,
            Self::Custom(role) => Some(role),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalCreditRoleError {
    InvalidCustomId(String),
}

/// An ordered display credit.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalCredit {
    role: CanonicalCreditRole,
    label: Option<String>,
    contributors: Vec<String>,
}

impl CanonicalCredit {
    pub fn new(
        role: CanonicalCreditRole,
        label: Option<String>,
        contributors: Vec<String>,
    ) -> Result<Self, CanonicalCreditError> {
        let mut unique = std::collections::BTreeSet::new();
        if let Some(duplicate) = contributors
            .iter()
            .find(|contributor| !unique.insert(contributor.as_str()))
        {
            return Err(CanonicalCreditError::DuplicateContributor(
                duplicate.clone(),
            ));
        }
        Ok(Self {
            role,
            label,
            contributors,
        })
    }

    pub fn role(&self) -> &str {
        self.role.as_str()
    }

    pub const fn role_kind(&self) -> &CanonicalCreditRole {
        &self.role
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn contributors(&self) -> &[String] {
        &self.contributors
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalCreditError {
    DuplicateContributor(String),
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
    metadata: CanonicalObject,
}

impl CanonicalResource {
    pub fn new(
        id: impl Into<String>,
        kind: CanonicalResourceKind,
        media_type: impl Into<String>,
        declared_sha256: Option<DeclaredSha256>,
        metadata: CanonicalObject,
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

    pub fn metadata(&self) -> &CanonicalObject {
        &self.metadata
    }
}

/// A SHA-256 digest computed over exact opaque resource bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalContentSha256([u8; 32]);

impl CanonicalContentSha256 {
    pub fn digest(bytes: &[u8]) -> Self {
        let digest = Sha256::digest(bytes);
        let mut output = [0; 32];
        output.copy_from_slice(&digest);
        Self(output)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// One canonical resource descriptor paired with its exact workspace payload.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalBundledResource {
    resource: CanonicalResource,
    content_sha256: CanonicalContentSha256,
    bytes: Box<[u8]>,
}

impl CanonicalBundledResource {
    pub fn new(
        resource: CanonicalResource,
        bytes: Vec<u8>,
    ) -> Result<Self, CanonicalBundledResourceError> {
        let content_sha256 = CanonicalContentSha256::digest(&bytes);
        if let Some(declared) = resource.declared_sha256()
            && declared.as_bytes() != content_sha256.as_bytes()
        {
            return Err(CanonicalBundledResourceError::HashMismatch {
                declared,
                computed: content_sha256,
            });
        }
        Ok(Self {
            resource,
            content_sha256,
            bytes: bytes.into_boxed_slice(),
        })
    }

    pub fn resource(&self) -> &CanonicalResource {
        &self.resource
    }

    pub fn id(&self) -> &str {
        self.resource.id()
    }

    pub const fn content_sha256(&self) -> CanonicalContentSha256 {
        self.content_sha256
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalBundledResourceError {
    HashMismatch {
        declared: DeclaredSha256,
        computed: CanonicalContentSha256,
    },
}

impl CanonicalBundledResourceError {
    pub const fn declared(self) -> DeclaredSha256 {
        match self {
            Self::HashMismatch { declared, .. } => declared,
        }
    }

    pub const fn computed(self) -> CanonicalContentSha256 {
        match self {
            Self::HashMismatch { computed, .. } => computed,
        }
    }
}

/// The deterministic, logical-resource-ID keyed opaque payload closure.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalResourceBundle {
    resources: BTreeMap<String, CanonicalBundledResource>,
}

impl CanonicalResourceBundle {
    pub fn new(
        resources: Vec<CanonicalBundledResource>,
    ) -> Result<Self, CanonicalResourceBundleError> {
        let mut by_id = BTreeMap::new();
        for resource in resources {
            let id = resource.id().to_owned();
            if by_id.insert(id.clone(), resource).is_some() {
                return Err(CanonicalResourceBundleError::DuplicateId(id));
            }
        }
        Ok(Self { resources: by_id })
    }

    pub fn len(&self) -> usize {
        self.resources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&CanonicalBundledResource> {
        self.resources.get(id)
    }

    pub fn resources(&self) -> &BTreeMap<String, CanonicalBundledResource> {
        &self.resources
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalResourceBundleError {
    DuplicateId(String),
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

    /// Returns whether `audio_time` lies in the half-open audio-domain interval
    /// `[start, end)`. Chart-time values must be converted through [`AudioOffset`]
    /// before membership is tested.
    pub fn contains_audio_time(self, audio_time: f64) -> bool {
        audio_time.is_finite() && audio_time >= self.start_seconds && audio_time < self.end_seconds
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
    ) -> Result<Self, CanonicalSyncError> {
        if preview.is_some() && primary_audio.is_none() {
            return Err(CanonicalSyncError::PreviewRequiresPrimaryAudio);
        }
        Ok(Self {
            primary_audio,
            audio_offset,
            preview,
        })
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

    /// Shared player/converter forward map: `audioTime = chartTime + audioOffset`.
    pub fn audio_time(&self, chart_time: f64) -> Result<f64, TempoError> {
        self.audio_offset.audio_time(chart_time)
    }

    /// Shared player/converter inverse map: `chartTime = audioTime - audioOffset`.
    pub fn chart_time(&self, audio_time: f64) -> Result<f64, TempoError> {
        self.audio_offset.chart_time(audio_time)
    }

    /// Preview membership after converting `chart_time` into the audio domain.
    pub fn preview_contains_chart_time(&self, chart_time: f64) -> Result<bool, TempoError> {
        let Some(preview) = self.preview else {
            return Ok(false);
        };
        Ok(preview.contains_audio_time(self.audio_time(chart_time)?))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalSyncError {
    PreviewRequiresPrimaryAudio,
}

impl fmt::Display for CanonicalSyncError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PreviewRequiresPrimaryAudio => {
                write!(formatter, "sync preview requires primaryAudio")
            }
        }
    }
}

impl std::error::Error for CanonicalSyncError {}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credit_roles_classify_every_standard_name_and_exact_custom_ids() {
        for expected in CanonicalStandardCreditRole::ALL {
            let role = CanonicalCreditRole::parse(expected.as_str()).unwrap();
            assert_eq!(role.standard(), Some(expected));
            assert_eq!(role.as_str(), expected.as_str());
        }

        let artist = CanonicalCreditRole::parse("artist").unwrap();
        assert_eq!(artist.custom(), Some("artist"));
        for invalid in ["", "1artist", "chart effects", "custom(role)", "特效"] {
            assert!(CanonicalCreditRole::parse(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn contributor_and_credit_constructors_defend_canonical_invariants() {
        let empty = CanonicalObject::new(Vec::new()).unwrap();
        assert_eq!(
            CanonicalContributor::new("alice", "", Vec::new(), empty.clone()),
            Err(CanonicalContributorError::EmptyName)
        );

        let non_string = CanonicalObject::new(vec![CanonicalObjectEntry::new(
            "provider",
            CanonicalValue::Int(1),
        )])
        .unwrap();
        assert_eq!(
            CanonicalContributor::new("alice", "Alice", Vec::new(), non_string),
            Err(CanonicalContributorError::NonStringIdentifier {
                key: "provider".into()
            })
        );

        let role = CanonicalCreditRole::parse("charter").unwrap();
        assert_eq!(
            CanonicalCredit::new(role, None, vec!["alice".into(), "alice".into()]),
            Err(CanonicalCreditError::DuplicateContributor("alice".into()))
        );
    }

    #[test]
    fn resource_bundle_constructors_defend_hash_and_logical_id_invariants() {
        let metadata = CanonicalObject::new(Vec::new()).unwrap();
        let declared = DeclaredSha256::from_lower_hex(&"0".repeat(64)).unwrap();
        let mismatched = CanonicalResource::new(
            "payload",
            CanonicalResourceKind::Binary,
            "application/octet-stream",
            Some(declared),
            metadata.clone(),
        );
        assert!(matches!(
            CanonicalBundledResource::new(mismatched, b"not empty".to_vec()),
            Err(CanonicalBundledResourceError::HashMismatch { .. })
        ));

        let resource = CanonicalResource::new(
            "payload",
            CanonicalResourceKind::Binary,
            "application/octet-stream",
            None,
            metadata,
        );
        let bundled = CanonicalBundledResource::new(resource, b"opaque".to_vec()).unwrap();
        assert_eq!(
            CanonicalResourceBundle::new(vec![bundled.clone(), bundled]),
            Err(CanonicalResourceBundleError::DuplicateId("payload".into()))
        );
    }

    #[test]
    fn preview_is_half_open_on_audio_time() {
        let preview = CanonicalPreview::new(30.0, 45.0).unwrap();
        assert!(preview.contains_audio_time(30.0));
        assert!(preview.contains_audio_time(44.999));
        assert!(!preview.contains_audio_time(45.0));
        assert!(!preview.contains_audio_time(29.999));
        assert!(!preview.contains_audio_time(f64::NAN));
        assert!(CanonicalPreview::new(-0.0, 1.0).is_some());
        assert!(CanonicalPreview::new(1.0, 1.0).is_none());
        assert!(CanonicalPreview::new(2.0, 1.0).is_none());
        assert!(CanonicalPreview::new(-1.0, 1.0).is_none());
        assert!(CanonicalPreview::new(0.0, f64::INFINITY).is_none());
    }

    #[test]
    fn sync_shares_offset_formula_and_requires_primary_audio_for_preview() {
        let offset = AudioOffset::new(0.1).unwrap();
        let preview = CanonicalPreview::new(30.0, 45.0);
        let sync = CanonicalSync::new(Some("song".into()), offset, preview).unwrap();
        assert_eq!(sync.audio_time(1.0).unwrap(), 1.1);
        assert_eq!(sync.chart_time(1.1).unwrap(), 1.0);
        // chartTime 29.9 + 0.1 = audioTime 30.0, which is preview-inclusive.
        assert!(sync.preview_contains_chart_time(29.9).unwrap());
        // chartTime 44.9 + 0.1 = audioTime 45.0, which is preview-exclusive.
        assert!(!sync.preview_contains_chart_time(44.9).unwrap());
        assert_eq!(
            CanonicalSync::new(None, offset, preview),
            Err(CanonicalSyncError::PreviewRequiresPrimaryAudio)
        );
        let no_preview = CanonicalSync::new(None, offset, None).unwrap();
        assert!(!no_preview.preview_contains_chart_time(0.0).unwrap());
    }
}
