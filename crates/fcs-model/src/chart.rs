//! Immutable aggregate of the currently implemented canonical chart fields.
//!
//! The aggregate deliberately owns only canonical values. Source AST nodes,
//! spans, authoring structure, and workspace inputs remain outside this crate.

use std::collections::BTreeSet;
use std::fmt;

use crate::{
    CanonicalDescriptorTable, CanonicalLineGraph, CanonicalMetadata, CanonicalNoteSet,
    CanonicalScrollSet, CanonicalTrackSet, ChartTimeMap,
};

/// The profile declared by a canonical chart's source format envelope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalProfile {
    Fragment,
    Chart,
    Playable,
    Renderable,
    Publishable,
}

/// An additional capability declared by the canonical chart's format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalProfileFeature {
    Playable,
    Renderable,
}

/// A validated source-format version retained as canonical identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalSourceVersion(String);

impl CanonicalSourceVersion {
    pub fn new(value: impl Into<String>) -> Result<Self, CanonicalChartError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CanonicalChartError::EmptySourceVersion);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalSourceVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A required execution extension identity carried by the canonical chart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalRequiredExtension {
    namespace: String,
    version: String,
}

impl CanonicalRequiredExtension {
    pub fn new(
        namespace: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, CanonicalChartError> {
        let namespace = namespace.into();
        let version = version.into();
        if namespace.is_empty() {
            return Err(CanonicalChartError::EmptyExtensionNamespace);
        }
        if version.is_empty() {
            return Err(CanonicalChartError::EmptyExtensionVersion);
        }
        Ok(Self { namespace, version })
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Errors raised while constructing the immutable chart aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalChartError {
    EmptySourceVersion,
    EmptyExtensionNamespace,
    EmptyExtensionVersion,
}

impl fmt::Display for CanonicalChartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptySourceVersion => "canonical source version must not be empty",
            Self::EmptyExtensionNamespace => "canonical extension namespace must not be empty",
            Self::EmptyExtensionVersion => "canonical extension version must not be empty",
        })
    }
}

impl std::error::Error for CanonicalChartError {}

/// The immutable chart semantic product consumed by later phases.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalChart {
    source_version: CanonicalSourceVersion,
    profile: CanonicalProfile,
    features: BTreeSet<CanonicalProfileFeature>,
    time_map: ChartTimeMap,
    metadata: CanonicalMetadata,
    lines: CanonicalLineGraph,
    notes: CanonicalNoteSet,
    tracks: CanonicalTrackSet,
    scroll: CanonicalScrollSet,
    descriptors: Option<CanonicalDescriptorTable>,
    required_extensions: Vec<CanonicalRequiredExtension>,
}

impl CanonicalChart {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_version: CanonicalSourceVersion,
        profile: CanonicalProfile,
        features: impl IntoIterator<Item = CanonicalProfileFeature>,
        time_map: ChartTimeMap,
        metadata: CanonicalMetadata,
        lines: CanonicalLineGraph,
        notes: CanonicalNoteSet,
        tracks: CanonicalTrackSet,
        scroll: CanonicalScrollSet,
        required_extensions: impl IntoIterator<Item = CanonicalRequiredExtension>,
    ) -> Self {
        Self {
            source_version,
            profile,
            features: features.into_iter().collect(),
            time_map,
            metadata,
            lines,
            notes,
            tracks,
            scroll,
            descriptors: None,
            required_extensions: required_extensions.into_iter().collect(),
        }
    }

    pub fn with_descriptors(mut self, descriptors: CanonicalDescriptorTable) -> Self {
        self.descriptors = Some(descriptors);
        self
    }

    pub fn source_version(&self) -> &CanonicalSourceVersion {
        &self.source_version
    }

    pub const fn profile(&self) -> CanonicalProfile {
        self.profile
    }

    pub fn features(&self) -> &BTreeSet<CanonicalProfileFeature> {
        &self.features
    }

    pub const fn time_map(&self) -> &ChartTimeMap {
        &self.time_map
    }

    pub const fn metadata(&self) -> &CanonicalMetadata {
        &self.metadata
    }

    pub const fn lines(&self) -> &CanonicalLineGraph {
        &self.lines
    }

    pub const fn notes(&self) -> &CanonicalNoteSet {
        &self.notes
    }

    pub const fn tracks(&self) -> &CanonicalTrackSet {
        &self.tracks
    }

    pub const fn scroll(&self) -> &CanonicalScrollSet {
        &self.scroll
    }

    pub const fn descriptors(&self) -> Option<&CanonicalDescriptorTable> {
        self.descriptors.as_ref()
    }

    pub fn required_extensions(&self) -> &[CanonicalRequiredExtension] {
        &self.required_extensions
    }
}
