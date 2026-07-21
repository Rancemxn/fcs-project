//! Lossless external-source parsing and profile-bound semantic boundaries for I6.
//!
//! Profile selection, repair, and canonical lowering remain separate later
//! stages and do not occur implicitly in these APIs.

use std::fmt;

use fcs_model::LogicalSourceLocator;
use serde::Deserializer;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};

mod exact;
mod pgr;
mod pgr_canonical;

pub use exact::{DecimalLimits, ExactDecimal, ExactNumberError, ExactRational};
pub use pgr::*;
pub use pgr_canonical::*;

/// Source format family. The parser dialect and semantic profile are separate
/// later-stage values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceFormat {
    Pgr,
    Rpe,
    Pec,
}

/// Source artifact role from Conversion §4.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArtifactRole {
    Chart,
    Manifest,
    Metadata,
    Audio,
    Image,
    Font,
    Other,
}

/// One exact source artifact owned by the active importer workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArtifact {
    logical_id: LogicalSourceLocator,
    role: ArtifactRole,
    bytes: Box<[u8]>,
    content_sha256: [u8; 32],
}

impl SourceArtifact {
    pub fn new(
        logical_id: impl Into<String>,
        role: ArtifactRole,
        bytes: impl Into<Vec<u8>>,
    ) -> Result<Self, ImportError> {
        Self::new_with_limits(logical_id, role, bytes, ImportLimits::default())
    }

    pub fn new_with_limits(
        logical_id: impl Into<String>,
        role: ArtifactRole,
        bytes: impl Into<Vec<u8>>,
        limits: ImportLimits,
    ) -> Result<Self, ImportError> {
        let logical_id = LogicalSourceLocator::new(logical_id).map_err(ImportError::Locator)?;
        let bytes = bytes.into();
        if bytes.len() > limits.max_single_artifact_bytes {
            return Err(ImportError::LimitExceeded {
                kind: "max_single_artifact_bytes",
                limit: limits.max_single_artifact_bytes,
                observed: bytes.len(),
            });
        }
        let bytes = bytes.into_boxed_slice();
        let mut content_sha256 = [0; 32];
        content_sha256.copy_from_slice(&Sha256::digest(&bytes));
        Ok(Self {
            logical_id,
            role,
            bytes,
            content_sha256,
        })
    }

    pub fn logical_id(&self) -> &LogicalSourceLocator {
        &self.logical_id
    }

    pub const fn role(&self) -> ArtifactRole {
        self.role
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn byte_length(&self) -> usize {
        self.bytes.len()
    }

    pub const fn content_sha256(&self) -> [u8; 32] {
        self.content_sha256
    }
}

/// Resource bounds applied before artifacts enter an importer workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportLimits {
    pub max_artifacts: usize,
    pub max_single_artifact_bytes: usize,
    pub max_total_artifact_bytes: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_artifacts: 1024,
            max_single_artifact_bytes: 64 * 1024 * 1024,
            max_total_artifact_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Ordered artifact closure with unique logical IDs.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceArtifactSet {
    artifacts: Vec<SourceArtifact>,
}

impl SourceArtifactSet {
    pub fn new(artifacts: impl IntoIterator<Item = SourceArtifact>) -> Result<Self, ImportError> {
        Self::new_with_limits(artifacts, ImportLimits::default())
    }

    pub fn new_with_limits(
        artifacts: impl IntoIterator<Item = SourceArtifact>,
        limits: ImportLimits,
    ) -> Result<Self, ImportError> {
        let mut output = Vec::new();
        let mut total_bytes = 0usize;
        for artifact in artifacts {
            let observed_count = output.len().saturating_add(1);
            if observed_count > limits.max_artifacts {
                return Err(ImportError::LimitExceeded {
                    kind: "max_artifacts",
                    limit: limits.max_artifacts,
                    observed: observed_count,
                });
            }
            if artifact.byte_length() > limits.max_single_artifact_bytes {
                return Err(ImportError::LimitExceeded {
                    kind: "max_single_artifact_bytes",
                    limit: limits.max_single_artifact_bytes,
                    observed: artifact.byte_length(),
                });
            }
            let observed_total = total_bytes.saturating_add(artifact.byte_length());
            if observed_total > limits.max_total_artifact_bytes {
                return Err(ImportError::LimitExceeded {
                    kind: "max_total_artifact_bytes",
                    limit: limits.max_total_artifact_bytes,
                    observed: observed_total,
                });
            }
            if output
                .iter()
                .any(|existing: &SourceArtifact| existing.logical_id() == artifact.logical_id())
            {
                return Err(ImportError::DuplicateArtifactId(
                    artifact.logical_id().as_str().to_owned(),
                ));
            }
            total_bytes = observed_total;
            output.push(artifact);
        }
        Ok(Self { artifacts: output })
    }

    pub fn artifacts(&self) -> &[SourceArtifact] {
        &self.artifacts
    }

    pub fn get(&self, logical_id: &str) -> Option<&SourceArtifact> {
        self.artifacts
            .iter()
            .find(|artifact| artifact.logical_id().as_str() == logical_id)
    }

    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }
}

/// A source string retaining both decoded value and exact JSON spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LosslessJsonString {
    value: String,
    raw: String,
}

impl LosslessJsonString {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
}

/// JSON value preserving source order, duplicate members, and number lexemes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LosslessJsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(LosslessJsonString),
    Array(Vec<Self>),
    Object(Vec<LosslessJsonMember>),
}

impl LosslessJsonValue {
    pub fn as_number_lexeme(&self) -> Option<&str> {
        match self {
            Self::Number(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.value()),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&[LosslessJsonMember]> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Self]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }
}

/// One ordered JSON object member; duplicate keys are intentionally retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LosslessJsonMember {
    key: LosslessJsonString,
    value: LosslessJsonValue,
}

impl LosslessJsonMember {
    pub fn key(&self) -> &str {
        self.key.value()
    }

    pub fn raw_key(&self) -> &str {
        self.key.raw()
    }

    pub fn value(&self) -> &LosslessJsonValue {
        &self.value
    }
}

/// Parsed source document. It contains no profile binding or FCS semantic IR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSourceDocument {
    artifact_id: LogicalSourceLocator,
    format: SourceFormat,
    body: LosslessJsonValue,
}

impl ParsedSourceDocument {
    pub fn artifact_id(&self) -> &LogicalSourceLocator {
        &self.artifact_id
    }

    pub const fn format(&self) -> SourceFormat {
        self.format
    }

    pub fn body(&self) -> &LosslessJsonValue {
        &self.body
    }
}

/// Parses one PGR/RPE JSON artifact without interpreting its fields.
pub fn parse_json_document(
    format: SourceFormat,
    artifact: &SourceArtifact,
) -> Result<ParsedSourceDocument, ImportError> {
    if format == SourceFormat::Pec {
        return Err(ImportError::JsonFormatUnsupported);
    }
    std::str::from_utf8(artifact.bytes()).map_err(|_| ImportError::InvalidUtf8)?;
    let raw: Box<RawValue> = serde_json::from_slice(artifact.bytes())
        .map_err(|error| ImportError::Json(error.to_string()))?;
    let body = parse_raw_json(raw.get())?;
    Ok(ParsedSourceDocument {
        artifact_id: artifact.logical_id().clone(),
        format,
        body,
    })
}

fn parse_raw_json(raw: &str) -> Result<LosslessJsonValue, ImportError> {
    let raw = raw.trim();
    let first = raw
        .as_bytes()
        .first()
        .copied()
        .ok_or(ImportError::Json("JSON value must not be empty".into()))?;
    match first {
        b'{' => {
            let mut deserializer = serde_json::Deserializer::from_str(raw);
            let value = deserializer
                .deserialize_map(ObjectVisitor)
                .map_err(|error| ImportError::Json(error.to_string()))?;
            deserializer
                .end()
                .map_err(|error| ImportError::Json(error.to_string()))?;
            Ok(LosslessJsonValue::Object(value))
        }
        b'[' => {
            let mut deserializer = serde_json::Deserializer::from_str(raw);
            let value = deserializer
                .deserialize_seq(ArrayVisitor)
                .map_err(|error| ImportError::Json(error.to_string()))?;
            deserializer
                .end()
                .map_err(|error| ImportError::Json(error.to_string()))?;
            Ok(LosslessJsonValue::Array(value))
        }
        b'"' => Ok(LosslessJsonValue::String(parse_json_string(raw)?)),
        b't' if raw == "true" => Ok(LosslessJsonValue::Bool(true)),
        b'f' if raw == "false" => Ok(LosslessJsonValue::Bool(false)),
        b'n' if raw == "null" => Ok(LosslessJsonValue::Null),
        _ => {
            let _: Box<RawValue> =
                serde_json::from_str(raw).map_err(|error| ImportError::Json(error.to_string()))?;
            Ok(LosslessJsonValue::Number(raw.to_owned()))
        }
    }
}

struct ObjectVisitor;

impl<'de> Visitor<'de> for ObjectVisitor {
    type Value = Vec<LosslessJsonMember>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut members = Vec::new();
        while let Some(raw_key) = map.next_key::<Box<RawValue>>()? {
            let key = parse_json_string(raw_key.get()).map_err(de::Error::custom)?;
            let raw: Box<RawValue> = map.next_value()?;
            let value = parse_raw_json(raw.get()).map_err(de::Error::custom)?;
            members.push(LosslessJsonMember { key, value });
        }
        Ok(members)
    }
}

fn parse_json_string(raw: &str) -> Result<LosslessJsonString, ImportError> {
    let value = serde_json::from_str(raw).map_err(|error| ImportError::Json(error.to_string()))?;
    Ok(LosslessJsonString {
        value,
        raw: raw.to_owned(),
    })
}

struct ArrayVisitor;

impl<'de> Visitor<'de> for ArrayVisitor {
    type Value = Vec<LosslessJsonValue>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON array")
    }

    fn visit_seq<S>(self, mut sequence: S) -> Result<Self::Value, S::Error>
    where
        S: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(raw) = sequence.next_element::<Box<RawValue>>()? {
            values.push(parse_raw_json(raw.get()).map_err(de::Error::custom)?);
        }
        Ok(values)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    Locator(fcs_model::ProvenanceError),
    DuplicateArtifactId(String),
    LimitExceeded {
        kind: &'static str,
        limit: usize,
        observed: usize,
    },
    InvalidUtf8,
    Json(String),
    JsonFormatUnsupported,
}

impl fmt::Display for ImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Locator(error) => error.fmt(formatter),
            Self::DuplicateArtifactId(id) => {
                write!(formatter, "duplicate source artifact id: {id}")
            }
            Self::LimitExceeded {
                kind,
                limit,
                observed,
            } => write!(
                formatter,
                "import limit {kind} exceeded: limit {limit}, observed {observed}"
            ),
            Self::InvalidUtf8 => formatter.write_str("source artifact is not valid UTF-8"),
            Self::Json(message) => write!(formatter, "invalid source JSON: {message}"),
            Self::JsonFormatUnsupported => {
                formatter.write_str("JSON parser only accepts PGR and RPE source formats")
            }
        }
    }
}

impl std::error::Error for ImportError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(json: &str) -> SourceArtifact {
        SourceArtifact::new("chart.json", ArtifactRole::Chart, json.as_bytes().to_vec()).unwrap()
    }

    #[test]
    fn json_parser_preserves_duplicate_order_and_number_lexemes() {
        let document = parse_json_document(
            SourceFormat::Pgr,
            &artifact(
                r#"{"bpmfactor":-0,"x":1e+03,"x":123456789012345678901234567890,"lineBpm":120}"#,
            ),
        )
        .unwrap();
        let members = document.body().as_object().unwrap();
        assert_eq!(
            members
                .iter()
                .map(LosslessJsonMember::key)
                .collect::<Vec<_>>(),
            ["bpmfactor", "x", "x", "lineBpm"]
        );
        assert_eq!(members[0].value().as_number_lexeme(), Some("-0"));
        assert_eq!(members[1].value().as_number_lexeme(), Some("1e+03"));
        assert_eq!(
            members[2].value().as_number_lexeme(),
            Some("123456789012345678901234567890")
        );
    }

    #[test]
    fn json_parser_preserves_string_escaping_and_array_order() {
        let document = parse_json_document(
            SourceFormat::Rpe,
            &artifact(r#"{"\u0074ext":"\u0061","events":[3,2,1]}"#),
        )
        .unwrap();
        let members = document.body().as_object().unwrap();
        assert_eq!(members[0].key(), "text");
        assert_eq!(members[0].raw_key(), r#""\u0074ext""#);
        let text = members[0].value().as_string().unwrap();
        assert_eq!(text, "a");
        assert_eq!(
            members[0].value(),
            &LosslessJsonValue::String(LosslessJsonString {
                value: "a".into(),
                raw: r#""\u0061""#.into()
            })
        );
        let events = members[1].value().as_array().unwrap();
        assert_eq!(
            events
                .iter()
                .map(LosslessJsonValue::as_number_lexeme)
                .collect::<Vec<_>>(),
            [Some("3"), Some("2"), Some("1")]
        );
    }

    #[test]
    fn artifacts_preserve_bytes_hash_and_order_and_reject_duplicates() {
        let first = SourceArtifact::new("chart.json", ArtifactRole::Chart, b"{}".to_vec()).unwrap();
        let second =
            SourceArtifact::new("manifest.json", ArtifactRole::Manifest, b"[]".to_vec()).unwrap();
        assert_eq!(first.byte_length(), 2);
        assert_eq!(first.content_sha256(), {
            let mut digest = [0; 32];
            digest.copy_from_slice(&Sha256::digest(b"{}"));
            digest
        });
        let set = SourceArtifactSet::new([first.clone(), second]).unwrap();
        assert_eq!(set.artifacts()[0], first);
        assert!(matches!(
            SourceArtifactSet::new([first.clone(), first]),
            Err(ImportError::DuplicateArtifactId(_))
        ));
    }

    #[test]
    fn invalid_utf8_json_and_pec_json_are_rejected_without_document() {
        let invalid = SourceArtifact::new("chart.json", ArtifactRole::Chart, vec![0xff]).unwrap();
        assert!(matches!(
            parse_json_document(SourceFormat::Pgr, &invalid),
            Err(ImportError::InvalidUtf8)
        ));
        assert!(matches!(
            parse_json_document(SourceFormat::Rpe, &artifact(r#"{"events":]"#)),
            Err(ImportError::Json(_))
        ));
        assert!(matches!(
            parse_json_document(SourceFormat::Pec, &artifact("{}")),
            Err(ImportError::JsonFormatUnsupported)
        ));
    }

    #[test]
    fn artifact_limits_reject_oversized_inputs_and_sets() {
        let limits = ImportLimits {
            max_artifacts: 1,
            max_single_artifact_bytes: 2,
            max_total_artifact_bytes: 2,
        };
        assert!(matches!(
            SourceArtifact::new_with_limits(
                "chart.json",
                ArtifactRole::Chart,
                b"123".to_vec(),
                limits
            ),
            Err(ImportError::LimitExceeded {
                kind: "max_single_artifact_bytes",
                limit: 2,
                observed: 3,
            })
        ));

        let first = SourceArtifact::new("chart.json", ArtifactRole::Chart, b"{}".to_vec()).unwrap();
        let second =
            SourceArtifact::new("manifest.json", ArtifactRole::Manifest, Vec::new()).unwrap();
        assert!(matches!(
            SourceArtifactSet::new_with_limits([first, second], limits),
            Err(ImportError::LimitExceeded {
                kind: "max_artifacts",
                limit: 1,
                observed: 2,
            })
        ));

        let first = SourceArtifact::new("chart.json", ArtifactRole::Chart, b"{}".to_vec()).unwrap();
        let second =
            SourceArtifact::new("manifest.json", ArtifactRole::Manifest, b"x".to_vec()).unwrap();
        let total_limits = ImportLimits {
            max_artifacts: 2,
            max_single_artifact_bytes: 2,
            max_total_artifact_bytes: 2,
        };
        assert!(matches!(
            SourceArtifactSet::new_with_limits([first, second], total_limits),
            Err(ImportError::LimitExceeded {
                kind: "max_total_artifact_bytes",
                limit: 2,
                observed: 3,
            })
        ));
    }
}
