//! Immutable canonical-model foundations shared by later FCS lowering stages.
//!
//! I3.1 owns identity construction and I3.2 owns exact global chart-time
//! normalization. Graph validation, Track lowering, Note semantics, and runtime
//! descriptors remain later stages.

use std::collections::BTreeMap;
use std::fmt;

use sha2::{Digest, Sha256};

mod line;
mod metadata;
mod time;

pub use line::{
    CanonicalLine, CanonicalLineBase, CanonicalLineGraph, CanonicalLineInherit,
    CanonicalLineWorldState, CanonicalScrollTempo, CanonicalScrollTempoMap,
    CanonicalScrollTempoPoint, CanonicalVec2, LineBaseError, LineGraphError, ScrollTempoDomain,
    ScrollTempoError, ScrollTempoKey,
};
pub use metadata::{
    CanonicalArrayError, CanonicalArtwork, CanonicalColor, CanonicalColorError,
    CanonicalContributor, CanonicalCredit, CanonicalMetadata, CanonicalObject,
    CanonicalObjectEntry, CanonicalObjectError, CanonicalPreview, CanonicalResource,
    CanonicalResourceKind, CanonicalSync, CanonicalValue, CanonicalValueType, DeclaredSha256,
};
pub use time::{AudioOffset, Beat, CanonicalTime, ChartTimeMap, TempoError, TempoPoint};

/// The textual namespace reserved for compiler-generated identities.
pub const GENERATED_PREFIX: &str = "generated/";

/// The fixed FCBC namespace used when deriving a stable Line ID.
pub const LINE_NAMESPACE: &str = "fcs.line";

/// The fixed FCBC namespace used when deriving a stable Note ID.
pub const NOTE_NAMESPACE: &str = "fcs.note";

/// A canonical entity kind with a fixed lowercase textual spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    Line,
    Note,
}

impl EntityKind {
    pub const fn namespace(self) -> &'static str {
        match self {
            Self::Line => LINE_NAMESPACE,
            Self::Note => NOTE_NAMESPACE,
        }
    }

    pub const fn segment(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Note => "note",
        }
    }
}

/// A source/expansion path used to form a generated canonical textual ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpansionPath {
    collection: String,
    item_order: u64,
    template_calls: Vec<TemplateCall>,
    generator_indices: Vec<u64>,
}

impl ExpansionPath {
    /// Creates a direct collection item path. All order values are zero-based.
    pub fn new(collection: impl Into<String>, item_order: u64) -> Result<Self, IdError> {
        let path = Self {
            collection: collection.into(),
            item_order,
            template_calls: Vec::new(),
            generator_indices: Vec::new(),
        };
        path.validate()?;
        Ok(path)
    }

    /// Appends a template call ancestry segment.
    pub fn with_template_call(
        mut self,
        template: impl Into<String>,
        call_order: u64,
    ) -> Result<Self, IdError> {
        let call = TemplateCall {
            template: template.into(),
            call_order,
        };
        validate_ascii_segment(&call.template, "template name")?;
        self.template_calls.push(call);
        self.validate()?;
        Ok(self)
    }

    /// Appends a generator iteration index.
    pub fn with_generator_index(mut self, generator_index: u64) -> Result<Self, IdError> {
        self.generator_indices.push(generator_index);
        self.validate()?;
        Ok(self)
    }

    pub fn collection(&self) -> &str {
        &self.collection
    }

    pub const fn item_order(&self) -> u64 {
        self.item_order
    }

    pub fn template_calls(&self) -> impl Iterator<Item = (&str, u64)> {
        self.template_calls
            .iter()
            .map(|call| (call.template.as_str(), call.call_order))
    }

    pub fn generator_indices(&self) -> impl Iterator<Item = u64> + '_ {
        self.generator_indices.iter().copied()
    }

    fn validate(&self) -> Result<(), IdError> {
        validate_ascii_segment(&self.collection, "collection name")?;
        Ok(())
    }

    fn append_segments(&self, output: &mut String) {
        output.push_str("collection/");
        output.push_str(&self.collection);
        output.push_str("/item/");
        output.push_str(&self.item_order.to_string());
        for call in &self.template_calls {
            output.push_str("/template/");
            output.push_str(&call.template);
            output.push_str("/call/");
            output.push_str(&call.call_order.to_string());
        }
        for index in &self.generator_indices {
            output.push_str("/generate/");
            output.push_str(&index.to_string());
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TemplateCall {
    template: String,
    call_order: u64,
}

fn validate_ascii_segment(value: &str, field: &'static str) -> Result<(), IdError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(IdError::InvalidPathSegment {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// A canonical textual identity, preserving explicit source bytes exactly.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalTextualId(String);

impl CanonicalTextualId {
    /// Validates and retains an explicit source ID without normalization.
    pub fn explicit(value: impl Into<String>) -> Result<Self, IdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IdError::EmptyExplicitId);
        }
        if value.starts_with(GENERATED_PREFIX) {
            return Err(IdError::ReservedGeneratedPrefix { value });
        }
        Ok(Self(value))
    }

    /// Constructs the normative generated textual ID spelling.
    pub fn generated(kind: EntityKind, path: &ExpansionPath, order: u64) -> Self {
        let mut value = String::from(GENERATED_PREFIX);
        value.push_str(kind.segment());
        value.push('/');
        path.append_segments(&mut value);
        value.push_str("/order/");
        value.push_str(&order.to_string());
        Self(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for CanonicalTextualId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for CanonicalTextualId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The final typed stable ID and its canonical textual source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableId {
    namespace: EntityKind,
    value: u64,
    textual: CanonicalTextualId,
}

impl StableId {
    pub fn namespace(&self) -> EntityKind {
        self.namespace
    }

    pub const fn value(&self) -> u64 {
        self.value
    }

    pub fn textual(&self) -> &CanonicalTextualId {
        &self.textual
    }
}

/// A registry that rejects duplicate textual identities and typed u64 collisions.
#[derive(Debug, Default)]
pub struct StableIdRegistry {
    entries: BTreeMap<(EntityKind, u64), CanonicalTextualId>,
}

impl StableIdRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(
        &mut self,
        kind: EntityKind,
        textual: CanonicalTextualId,
    ) -> Result<StableId, IdError> {
        let value = derive_stable_id(kind, textual.as_str());
        self.insert_value(kind, textual, value)
    }

    fn insert_value(
        &mut self,
        kind: EntityKind,
        textual: CanonicalTextualId,
        value: u64,
    ) -> Result<StableId, IdError> {
        if value == 0 {
            return Err(IdError::ZeroStableId {
                namespace: kind,
                textual,
            });
        }
        if let Some(previous) = self.entries.get(&(kind, value)) {
            if previous == &textual {
                return Err(IdError::DuplicateTextualId {
                    namespace: kind,
                    textual,
                });
            }
            return Err(IdError::StableIdCollision {
                namespace: kind,
                value,
                first: previous.clone(),
                second: textual,
            });
        }
        self.entries.insert((kind, value), textual.clone());
        Ok(StableId {
            namespace: kind,
            value,
            textual,
        })
    }

    pub fn get(&self, kind: EntityKind, value: u64) -> Option<&CanonicalTextualId> {
        self.entries.get(&(kind, value))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Applies FCBC §6.2: SHA-256(namespace || 0x00 || UTF-8 textual ID), first 64 LE bits.
pub fn derive_stable_id(kind: EntityKind, textual: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(kind.namespace().as_bytes());
    hasher.update([0]);
    hasher.update(textual.as_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(
        digest[..8]
            .try_into()
            .expect("SHA-256 has at least 8 bytes"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdError {
    EmptyExplicitId,
    MissingExpansionPath,
    ReservedGeneratedPrefix {
        value: String,
    },
    InvalidPathSegment {
        field: &'static str,
        value: String,
    },
    ZeroStableId {
        namespace: EntityKind,
        textual: CanonicalTextualId,
    },
    DuplicateTextualId {
        namespace: EntityKind,
        textual: CanonicalTextualId,
    },
    StableIdCollision {
        namespace: EntityKind,
        value: u64,
        first: CanonicalTextualId,
        second: CanonicalTextualId,
    },
}

impl fmt::Display for IdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyExplicitId => formatter.write_str("explicit canonical ID must not be empty"),
            Self::MissingExpansionPath => {
                formatter.write_str("generated canonical ID is missing expansion provenance")
            }
            Self::ReservedGeneratedPrefix { value } => {
                write!(
                    formatter,
                    "explicit canonical ID uses reserved prefix: {value}"
                )
            }
            Self::InvalidPathSegment { field, value } => {
                write!(formatter, "invalid ASCII {field} path segment: {value}")
            }
            Self::ZeroStableId { namespace, textual } => {
                write!(
                    formatter,
                    "stable ID 0 is reserved for {namespace:?}: {textual}"
                )
            }
            Self::DuplicateTextualId { namespace, textual } => {
                write!(
                    formatter,
                    "duplicate textual ID in {namespace:?}: {textual}"
                )
            }
            Self::StableIdCollision {
                namespace,
                value,
                first,
                second,
            } => write!(
                formatter,
                "stable ID collision in {namespace:?} at {value}: {first} vs {second}"
            ),
        }
    }
}

impl std::error::Error for IdError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_textual_ids_are_deterministic_and_zero_based() {
        let path = ExpansionPath::new("notes", 0)
            .unwrap()
            .with_template_call("generatedTap", 0)
            .unwrap()
            .with_generator_index(3)
            .unwrap();
        let id = CanonicalTextualId::generated(EntityKind::Note, &path, 3);
        assert_eq!(
            id.as_str(),
            "generated/note/collection/notes/item/0/template/generatedTap/call/0/generate/3/order/3"
        );
        assert!(!id.as_str().contains("/03"));
    }

    #[test]
    fn explicit_ids_are_byte_exact_but_cannot_enter_generated_namespace() {
        let explicit = CanonicalTextualId::explicit("é/原样").unwrap();
        assert_eq!(explicit.as_str(), "é/原样");
        assert!(matches!(
            CanonicalTextualId::explicit("generated/note/x"),
            Err(IdError::ReservedGeneratedPrefix { .. })
        ));
    }

    #[test]
    fn typed_namespaces_allow_the_same_textual_id() {
        let textual = CanonicalTextualId::explicit("same").unwrap();
        let mut registry = StableIdRegistry::new();
        registry.insert(EntityKind::Line, textual.clone()).unwrap();
        registry.insert(EntityKind::Note, textual).unwrap();
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn duplicate_textual_ids_are_rejected_without_salt() {
        let textual = CanonicalTextualId::explicit("same").unwrap();
        let mut registry = StableIdRegistry::new();
        registry.insert(EntityKind::Note, textual.clone()).unwrap();
        assert!(matches!(
            registry.insert(EntityKind::Note, textual),
            Err(IdError::DuplicateTextualId { .. })
        ));
    }

    #[test]
    fn typed_stable_id_collisions_fail() {
        let first = CanonicalTextualId::explicit("first").unwrap();
        let second = CanonicalTextualId::explicit("second").unwrap();
        let second_value = derive_stable_id(EntityKind::Note, second.as_str());
        let mut registry = StableIdRegistry::new();
        registry
            .entries
            .insert((EntityKind::Note, second_value), first.clone());
        assert!(matches!(
            registry.insert(EntityKind::Note, second),
            Err(IdError::StableIdCollision {
                first: actual_first,
                ..
            }) if actual_first == first
        ));
    }

    #[test]
    fn zero_stable_ids_are_rejected_as_reserved() {
        let textual = CanonicalTextualId::explicit("zero").unwrap();
        let mut registry = StableIdRegistry::new();
        assert!(matches!(
            registry.insert_value(EntityKind::Line, textual, 0),
            Err(IdError::ZeroStableId { .. })
        ));
    }

    #[test]
    fn canonical_id_fixture_vectors_bind_direct_template_and_generator_inputs() {
        let direct =
            include_str!("../../../docs/conformance/fcs5/source/valid/canonical-id-direct.fcs");
        let template =
            include_str!("../../../docs/conformance/fcs5/source/valid/canonical-id-template.fcs");
        let generator =
            include_str!("../../../docs/conformance/fcs5/source/valid/compile-time-generator.fcs");
        let expected = include_str!("../../../docs/conformance/fcs5/expected/canonical-ids.json");

        assert!(direct.contains("notes"));
        assert!(template.contains("template Note makeTap"));
        assert!(generator.contains("generate at: beat"));
        for textual in [
            "generated/note/collection/notes/item/0/order/0",
            "generated/note/collection/notes/item/0/template/makeTap/call/0/order/0",
            "generated/note/collection/notes/item/0/template/generatedTap/call/3/generate/3/order/3",
        ] {
            assert!(
                expected.contains(textual),
                "missing fixture vector: {textual}"
            );
        }
        let line = CanonicalTextualId::generated(
            EntityKind::Line,
            &ExpansionPath::new("lines", 0).unwrap(),
            0,
        );
        assert!(expected.contains(line.as_str()));
    }

    #[test]
    fn sha256_vector_uses_namespace_separator_and_little_endian_bits() {
        let textual = CanonicalTextualId::explicit("equivalent-note").unwrap();
        assert_eq!(
            derive_stable_id(EntityKind::Note, textual.as_str()),
            0x936e_d104_3322_2dcc
        );
    }
}
