//! I3.3 source-to-canonical lowering for metadata, resources, artwork, sync,
//! and typed custom values.

use std::collections::{BTreeMap, BTreeSet};

use fcs_model::{
    AudioOffset, Beat as CanonicalBeat, CanonicalArtwork, CanonicalChart, CanonicalChartError,
    CanonicalColor, CanonicalContributor, CanonicalCredit, CanonicalCreditRole, CanonicalLineGraph,
    CanonicalMetadata, CanonicalObject, CanonicalObjectEntry, CanonicalPreview, CanonicalProfile,
    CanonicalProfileFeature, CanonicalRequiredExtension, CanonicalResource, CanonicalResourceKind,
    CanonicalSourceVersion, CanonicalSync, CanonicalValue, CanonicalValueType, DeclaredSha256,
};

use crate::ast::{
    Definition, Document, DocumentProfile, ExtensionRequirement, FieldPath, MetaBlock,
    ProfileFeature, ResourceKind, SchemaField, SchemaValue, SourceExpression, SourceLiteral,
    SourceSpan, SyncBlock, TopLevelBlockKind, TypedValue,
};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticStage};
use crate::elaborator::{CompileTimeLimits, elaborate};
use crate::schema::phase2_schema;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReferenceKind {
    Contributor,
    Resource,
}

#[derive(Debug, Clone)]
enum Expected {
    Any,
    Int,
    Float,
    Number,
    String,
    Time,
    Object,
    StringObject,
    Array(Box<Self>),
    Reference(ReferenceKind),
}

#[derive(Debug, Clone)]
enum RawValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Time(f64),
    Beat(CanonicalBeat),
    Color(CanonicalColor),
    Reference { name: String, span: SourceSpan },
    Array(Vec<Self>),
    Object(Vec<RawObjectEntry>),
}

#[derive(Debug, Clone)]
struct RawObjectEntry {
    key: String,
    key_span: SourceSpan,
    value: RawValue,
}

pub(crate) struct LoweredDocument {
    pub(crate) metadata: CanonicalMetadata,
    pub(crate) resource_sources: BTreeMap<String, LoweredResourceSource>,
}

#[derive(Debug, Clone)]
pub(crate) struct LoweredResourceSource {
    pub(crate) logical_path: String,
    pub(crate) span: SourceSpan,
}

struct LoweredResources {
    resources: BTreeMap<String, CanonicalResource>,
    sources: BTreeMap<String, LoweredResourceSource>,
}

impl Document {
    /// Compiles the parsed document into the immutable chart semantic product.
    ///
    /// Elaboration happens inside this boundary, so a caller cannot combine a
    /// parsed envelope with an expanded result from another document.
    pub fn canonical_chart(
        &self,
        limits: CompileTimeLimits,
    ) -> Result<CanonicalChart, Vec<Diagnostic>> {
        let expanded = elaborate(self, phase2_schema(), limits)?;
        let metadata = self.canonical_metadata()?;
        let lines = self.canonical_line_graph_with_expanded(&expanded)?;
        let profile_diagnostics = profile_requirement_diagnostics(self, &metadata, &lines);
        if !profile_diagnostics.is_empty() {
            return Err(profile_diagnostics);
        }
        let time_map = expanded.canonical_time_map().map_err(|error| {
            vec![canonical_diagnostic(
                DiagnosticCode::TEMPO_INVALID,
                error.to_string(),
                self.format.span,
            )]
        })?;
        let notes = expanded.canonical_notes(&time_map, &lines)?;
        let tracks = expanded.canonical_tracks(&time_map, &lines)?;
        let scroll = self.canonical_scroll_set_for_graph(&time_map, &lines)?;
        let source_version = CanonicalSourceVersion::new(self.source_version.to_string())
            .map_err(|error| vec![chart_diagnostic(error, self.format.span)])?;
        let required_extensions = self
            .extensions
            .iter()
            .flat_map(|block| &block.declarations)
            .filter(|declaration| declaration.requirement == ExtensionRequirement::Required)
            .map(|declaration| {
                CanonicalRequiredExtension::new(
                    declaration.header.namespace.clone(),
                    declaration.header.version.to_string(),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| vec![chart_diagnostic(error, self.format.span)])?;

        Ok(CanonicalChart::new(
            source_version,
            canonical_profile(self.profile),
            canonical_features(self),
            time_map,
            metadata,
            lines,
            notes,
            tracks,
            scroll,
            required_extensions,
        ))
    }

    /// Lowers the source metadata surface into an immutable canonical graph.
    ///
    /// This operation validates logical workspace member paths but never opens
    /// them, follows symlinks, reads bytes, or compares a declared hash with an
    /// input file. Use `canonical_resource_bundle` for that explicit-root I5
    /// boundary.
    pub fn canonical_metadata(&self) -> Result<CanonicalMetadata, Vec<Diagnostic>> {
        lower_document(self)
    }

    /// Validates the canonical requirements added by the declared profile and
    /// its orthogonal feature capabilities.
    ///
    /// A tempo-less `fragment` can pass this boundary because that profile does
    /// not require a chart time model. Building a [`CanonicalChart`] remains a
    /// stronger operation: FCS section 17 requires that product to contain a
    /// tempo map.
    pub fn validate_profile_requirements(
        &self,
        limits: CompileTimeLimits,
    ) -> Result<(), Vec<Diagnostic>> {
        let expanded = elaborate(self, phase2_schema(), limits)?;
        let metadata = self.canonical_metadata()?;
        let lines = self.canonical_line_graph_with_expanded(&expanded)?;
        let diagnostics = profile_requirement_diagnostics(self, &metadata, &lines);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }

        if self.profile != DocumentProfile::Fragment || self.tempo_map.is_some() {
            expanded.canonical_time_map().map_err(|error| {
                vec![canonical_diagnostic(
                    DiagnosticCode::TEMPO_INVALID,
                    error.to_string(),
                    self.format.span,
                )]
            })?;
        }
        Ok(())
    }
}

fn profile_requirement_diagnostics(
    document: &Document,
    metadata: &CanonicalMetadata,
    lines: &CanonicalLineGraph,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    if document.profile == DocumentProfile::Fragment {
        if let Some(features) = &document.format.features {
            for feature in &features.features {
                diagnostics.push(profile_diagnostic(
                    format!(
                        "fragment profile cannot declare the {} feature",
                        profile_feature_name(feature.value)
                    ),
                    feature.span,
                ));
            }
        }
        return diagnostics;
    }

    if document.tempo_map.is_none() {
        diagnostics.push(profile_diagnostic(
            "chart-capable profile requires a tempoMap",
            document.format.profile.span,
        ));
    }

    if let Some(span) = capability_span(document, ProfileFeature::Playable) {
        match metadata.sync() {
            None => diagnostics.push(profile_diagnostic(
                "playable capability requires a sync block",
                span,
            )),
            Some(sync) if sync.primary_audio().is_none() => diagnostics.push(profile_diagnostic(
                "playable capability requires sync.primaryAudio",
                document
                    .sync
                    .as_ref()
                    .map_or(span, |sync_block| sync_block.span),
            )),
            Some(_) => {}
        }
        if lines.lines().next().is_none() {
            diagnostics.push(profile_diagnostic(
                "playable capability requires at least one gameplay Line",
                span,
            ));
        }
    }

    if let Some(span) = capability_span(document, ProfileFeature::Renderable)
        && document.top_level(TopLevelBlockKind::Render).is_none()
    {
        diagnostics.push(profile_diagnostic(
            "renderable capability requires a Render scene envelope",
            span,
        ));
    }

    if document.profile == DocumentProfile::Publishable {
        let profile_span = document.format.profile.span;
        if explicit_features(document).next().is_none() {
            diagnostics.push(profile_diagnostic(
                "publishable profile requires at least one playable or renderable feature",
                profile_span,
            ));
        }

        let meta = metadata.meta();
        let meta_span = document
            .meta
            .as_ref()
            .map_or(profile_span, |block| block.span);
        for field in ["title", "documentId", "chartVersion", "license"] {
            if meta.is_none_or(|values| !values.contains_key(field)) {
                diagnostics.push(profile_diagnostic(
                    format!("publishable profile requires meta.{field}"),
                    meta_span,
                ));
            }
        }

        if metadata.credits().is_empty() {
            diagnostics.push(profile_diagnostic(
                "publishable profile requires at least one credit",
                document
                    .credits
                    .as_ref()
                    .map_or(profile_span, |block| block.span),
            ));
        }

        if let Some(resources) = &document.resources {
            for declaration in &resources.resources {
                if metadata
                    .resources()
                    .get(&declaration.name)
                    .is_some_and(|resource| resource.declared_sha256().is_none())
                {
                    diagnostics.push(profile_diagnostic(
                        format!(
                            "publishable resource {} requires a declared SHA-256 hash",
                            declaration.name
                        ),
                        declaration.name_span,
                    ));
                }
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.message().cmp(right.message()))
    });
    diagnostics
}

fn explicit_features(document: &Document) -> impl Iterator<Item = &crate::ast::FormatFeature> {
    document
        .format
        .features
        .iter()
        .flat_map(|features| features.features.iter())
}

fn capability_span(document: &Document, capability: ProfileFeature) -> Option<SourceSpan> {
    let primary_has_capability = matches!(
        (document.profile, capability),
        (DocumentProfile::Playable, ProfileFeature::Playable)
            | (DocumentProfile::Renderable, ProfileFeature::Renderable)
    );
    primary_has_capability
        .then_some(document.format.profile.span)
        .or_else(|| {
            explicit_features(document)
                .find(|feature| feature.value == capability)
                .map(|feature| feature.span)
        })
}

const fn profile_feature_name(feature: ProfileFeature) -> &'static str {
    match feature {
        ProfileFeature::Playable => "playable",
        ProfileFeature::Renderable => "renderable",
    }
}

fn profile_diagnostic(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    canonical_diagnostic(DiagnosticCode::PROFILE_REQUIREMENT_MISSING, message, span)
}

fn canonical_profile(profile: DocumentProfile) -> CanonicalProfile {
    match profile {
        DocumentProfile::Fragment => CanonicalProfile::Fragment,
        DocumentProfile::Chart => CanonicalProfile::Chart,
        DocumentProfile::Playable => CanonicalProfile::Playable,
        DocumentProfile::Renderable => CanonicalProfile::Renderable,
        DocumentProfile::Publishable => CanonicalProfile::Publishable,
    }
}

fn canonical_features(document: &Document) -> Vec<CanonicalProfileFeature> {
    document
        .format
        .features
        .iter()
        .flat_map(|features| features.features.iter())
        .map(|feature| match feature.value {
            ProfileFeature::Playable => CanonicalProfileFeature::Playable,
            ProfileFeature::Renderable => CanonicalProfileFeature::Renderable,
        })
        .collect()
}

fn chart_diagnostic(error: CanonicalChartError, span: SourceSpan) -> Diagnostic {
    canonical_diagnostic(
        DiagnosticCode::TYPE_INVALID_OPERATION,
        error.to_string(),
        span,
    )
}

fn lower_document(document: &Document) -> Result<CanonicalMetadata, Vec<Diagnostic>> {
    lower_document_with_sources(document).map(|lowered| lowered.metadata)
}

pub(crate) fn lower_document_with_sources(
    document: &Document,
) -> Result<LoweredDocument, Vec<Diagnostic>> {
    let contributor_names = contributor_names(document.contributors.as_ref());
    let resource_kinds = resource_kinds(document.resources.as_ref());
    let mut diagnostics = Vec::new();

    let contributors = lower_contributors(
        document.contributors.as_ref(),
        document.definitions.as_ref(),
        &mut diagnostics,
    );
    let resources = lower_resources(
        document.resources.as_ref(),
        document.definitions.as_ref(),
        &mut diagnostics,
    );
    let meta = lower_meta(
        document.meta.as_ref(),
        document.definitions.as_ref(),
        &contributor_names,
        &resource_kinds,
        &mut diagnostics,
    );
    let credits = lower_credits(
        document.credits.as_ref(),
        document.definitions.as_ref(),
        &contributor_names,
        &resource_kinds,
        &mut diagnostics,
    );
    let artwork = lower_artwork(
        document.artwork.as_ref(),
        document.definitions.as_ref(),
        &resource_kinds,
        &mut diagnostics,
    );
    let sync = lower_sync(
        document.sync.as_ref(),
        document.definitions.as_ref(),
        &resource_kinds,
        &mut diagnostics,
    );

    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.code().cmp(&right.code()))
    });
    if diagnostics.is_empty() {
        Ok(LoweredDocument {
            metadata: CanonicalMetadata::new(
                meta,
                contributors,
                credits,
                resources.resources,
                artwork,
                sync,
            ),
            resource_sources: resources.sources,
        })
    } else {
        Err(diagnostics)
    }
}

fn contributor_names(block: Option<&crate::ast::ContributorsBlock>) -> BTreeSet<String> {
    block
        .into_iter()
        .flat_map(|block| block.people.iter().map(|person| person.name.clone()))
        .collect()
}

fn resource_kinds(block: Option<&crate::ast::ResourcesBlock>) -> BTreeMap<String, ResourceKind> {
    block
        .into_iter()
        .flat_map(|block| {
            block
                .resources
                .iter()
                .map(|resource| (resource.name.clone(), resource.kind))
        })
        .collect()
}

fn lower_meta(
    block: Option<&MetaBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<BTreeMap<String, CanonicalValue>> {
    let block = block?;
    let mut expected = BTreeMap::new();
    for name in [
        "title",
        "subtitle",
        "chartVersion",
        "difficulty",
        "description",
        "language",
        "license",
        "documentId",
    ] {
        expected.insert(name, Expected::String);
    }
    expected.insert(
        "alternativeTitles",
        Expected::Array(Box::new(Expected::String)),
    );
    expected.insert("tags", Expected::Array(Box::new(Expected::String)));
    expected.insert("level", Expected::Number);
    expected.insert("revision", Expected::Int);
    expected.insert("custom", Expected::Object);
    let mut values = lower_fields(
        &block.fields,
        &expected,
        definitions,
        contributors,
        resources,
        diagnostics,
        "meta",
    );
    if let Some(CanonicalValue::Int(revision)) = values.get("revision")
        && *revision < 0
    {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::TYPE_INVALID_OPERATION,
            "meta revision must be non-negative",
            block.span,
        ));
    }
    if let Some(CanonicalValue::Int(level)) = values.remove("level") {
        values.insert("level".into(), CanonicalValue::Float(level as f64));
    }
    Some(values)
}

fn lower_contributors(
    block: Option<&crate::ast::ContributorsBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, CanonicalContributor> {
    let mut output = BTreeMap::new();
    let Some(block) = block else { return output };
    let empty_contributors = BTreeSet::new();
    let empty_resources = BTreeMap::new();
    let mut previous = BTreeMap::<String, SourceSpan>::new();

    for person in &block.people {
        if let Some(first_span) = previous.insert(person.name.clone(), person.name_span) {
            diagnostics.push(
                canonical_diagnostic(
                    DiagnosticCode::NAME_DUPLICATE,
                    format!("contributor ID {} is declared more than once", person.name),
                    person.name_span,
                )
                .with_label(DiagnosticLabel::new(
                    first_span,
                    "first contributor declaration",
                )),
            );
            continue;
        }
        let mut expected = BTreeMap::new();
        expected.insert("name", Expected::String);
        expected.insert("aliases", Expected::Array(Box::new(Expected::String)));
        expected.insert("identifiers", Expected::StringObject);
        let fields = lower_fields(
            &person.fields,
            &expected,
            definitions,
            &empty_contributors,
            &empty_resources,
            diagnostics,
            "contributor",
        );
        let Some(name) = string_field(
            &fields,
            "name",
            person.span,
            diagnostics,
            "contributor name",
        ) else {
            continue;
        };
        if name.is_empty() {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "contributor name must not be empty",
                person.span,
            ));
            continue;
        }
        let aliases = fields
            .get("aliases")
            .and_then(array_values)
            .map(|values| values.iter().filter_map(string_value).collect())
            .unwrap_or_default();
        let identifiers = fields
            .get("identifiers")
            .and_then(|value| match value {
                CanonicalValue::Object(object) => Some(object.clone()),
                _ => None,
            })
            .unwrap_or_else(|| {
                CanonicalObject::new(Vec::new()).expect("an empty canonical object is valid")
            });
        output.insert(
            person.name.clone(),
            CanonicalContributor::new(person.name.clone(), name, aliases, identifiers)
                .expect("source validation establishes canonical contributor invariants"),
        );
    }
    output
}

fn lower_credits(
    block: Option<&crate::ast::CreditsBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CanonicalCredit> {
    let mut output = Vec::new();
    let Some(block) = block else { return output };
    for entry in &block.entries {
        let mut expected = BTreeMap::new();
        expected.insert("role", Expected::String);
        expected.insert("label", Expected::String);
        expected.insert(
            "contributors",
            Expected::Array(Box::new(Expected::Reference(ReferenceKind::Contributor))),
        );
        let fields = lower_fields(
            &entry.fields,
            &expected,
            definitions,
            contributors,
            resources,
            diagnostics,
            "credit",
        );
        let Some(role) = string_field(&fields, "role", entry.span, diagnostics, "credit role")
        else {
            continue;
        };
        let Ok(role) = CanonicalCreditRole::parse(role) else {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "credit role must be a standard role or a non-empty ASCII custom ID",
                entry.span,
            ));
            continue;
        };
        let label = fields.get("label").and_then(string_value);
        let credit_contributors = fields
            .get("contributors")
            .and_then(array_values)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| match value {
                        CanonicalValue::ContributorReference(name) => Some(name.clone()),
                        _ => None,
                    })
                    .collect()
            })
            .unwrap_or_default();
        match CanonicalCredit::new(role, label, credit_contributors) {
            Ok(credit) => output.push(credit),
            Err(_) => diagnostics.push(canonical_diagnostic(
                DiagnosticCode::NAME_DUPLICATE,
                "a credit contributor reference must be unique within its credit",
                entry.span,
            )),
        }
    }
    output
}

fn lower_resources(
    block: Option<&crate::ast::ResourcesBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> LoweredResources {
    let mut output = BTreeMap::new();
    let mut sources = BTreeMap::new();
    let Some(block) = block else {
        return LoweredResources {
            resources: output,
            sources,
        };
    };
    let empty_contributors = BTreeSet::new();
    let empty_resources = BTreeMap::new();
    let mut previous = BTreeMap::<String, SourceSpan>::new();

    for declaration in &block.resources {
        if let Some(first_span) = previous.insert(declaration.name.clone(), declaration.name_span) {
            diagnostics.push(
                canonical_diagnostic(
                    DiagnosticCode::NAME_DUPLICATE,
                    format!(
                        "resource ID {} is declared more than once",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .with_label(DiagnosticLabel::new(
                    first_span,
                    "first resource declaration",
                )),
            );
            continue;
        }
        let mut expected = BTreeMap::new();
        expected.insert("source", Expected::String);
        expected.insert("hash", Expected::String);
        expected.insert("mediaType", Expected::String);
        expected.insert("colorSpace", Expected::String);
        expected.insert("alpha", Expected::String);
        expected.insert("sampling", Expected::String);
        expected.insert("fontProfile", Expected::String);
        expected.insert("shapingProfile", Expected::String);
        expected.insert("faceCount", Expected::Int);
        let mut fields = lower_fields(
            &declaration.fields,
            &expected,
            definitions,
            &empty_contributors,
            &empty_resources,
            diagnostics,
            "resource",
        );
        let Some(source) = string_field(
            &fields,
            "source",
            declaration.span,
            diagnostics,
            "resource source",
        ) else {
            continue;
        };
        if !valid_workspace_member_path(&source) {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                "resource source must be a relative workspace member path",
                declaration.span,
            ));
            continue;
        }
        let Some(media_type) = string_field(
            &fields,
            "mediaType",
            declaration.span,
            diagnostics,
            "resource mediaType",
        ) else {
            continue;
        };
        if media_type.is_empty() {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "resource mediaType must not be empty",
                declaration.span,
            ));
            continue;
        }
        let declared_sha256 = match fields.remove("hash").and_then(|value| string_value(&value)) {
            Some(value) => match value.strip_prefix("sha256:") {
                Some(hex) => match DeclaredSha256::from_lower_hex(hex) {
                    Some(digest) => Some(digest),
                    None => {
                        diagnostics.push(canonical_diagnostic(
                            DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                            "resource hash must use sha256: followed by 64 lowercase hex digits",
                            declaration.span,
                        ));
                        continue;
                    }
                },
                None => {
                    diagnostics.push(canonical_diagnostic(
                        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        "resource hash must use the sha256: algorithm prefix",
                        declaration.span,
                    ));
                    continue;
                }
            },
            None => None,
        };
        fields.remove("source");
        fields.remove("mediaType");
        let metadata = canonical_resource_metadata(
            declaration.kind,
            &media_type,
            fields,
            declaration.span,
            diagnostics,
        );
        let source_span = declaration
            .fields
            .iter()
            .find(|field| field.path.segments.len() == 1 && field.path.segments[0] == "source")
            .map_or(declaration.span, |field| field.value.span());
        sources.insert(
            declaration.name.clone(),
            LoweredResourceSource {
                logical_path: source,
                span: source_span,
            },
        );
        output.insert(
            declaration.name.clone(),
            CanonicalResource::new(
                declaration.name.clone(),
                canonical_resource_kind(declaration.kind),
                media_type,
                declared_sha256,
                metadata,
            ),
        );
    }
    LoweredResources {
        resources: output,
        sources,
    }
}

fn canonical_resource_metadata(
    kind: ResourceKind,
    media_type: &str,
    mut fields: BTreeMap<String, CanonicalValue>,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> CanonicalObject {
    let entries = match kind {
        ResourceKind::Image | ResourceKind::Texture => {
            reject_resource_metadata_fields(
                &mut fields,
                &["fontProfile", "shapingProfile", "faceCount"],
                kind,
                span,
                diagnostics,
            );
            let color_space = resource_string_metadata(
                &mut fields,
                "colorSpace",
                "srgb",
                &["srgb", "linear-srgb"],
                span,
                diagnostics,
            );
            let alpha = resource_string_metadata(
                &mut fields,
                "alpha",
                "straight",
                &["straight", "premultiplied"],
                span,
                diagnostics,
            );
            let sampling = resource_string_metadata(
                &mut fields,
                "sampling",
                "linear",
                &["nearest", "linear"],
                span,
                diagnostics,
            );
            vec![
                CanonicalObjectEntry::new("colorSpace", CanonicalValue::String(color_space)),
                CanonicalObjectEntry::new("alpha", CanonicalValue::String(alpha)),
                CanonicalObjectEntry::new("sampling", CanonicalValue::String(sampling)),
            ]
        }
        ResourceKind::Font if media_type == "font/ttf" => {
            reject_resource_metadata_fields(
                &mut fields,
                &["colorSpace", "alpha", "sampling"],
                kind,
                span,
                diagnostics,
            );
            let font_profile = resource_exact_string_metadata(
                &mut fields,
                "fontProfile",
                "truetype-glyf-1",
                span,
                diagnostics,
            );
            let shaping_profile = resource_exact_string_metadata(
                &mut fields,
                "shapingProfile",
                "simple-ltr-1",
                span,
                diagnostics,
            );
            let face_count = match fields.remove("faceCount") {
                Some(CanonicalValue::Int(1)) | None => 1,
                Some(CanonicalValue::Int(value)) => {
                    diagnostics.push(canonical_diagnostic(
                        DiagnosticCode::TYPE_INVALID_OPERATION,
                        format!("font/ttf resource faceCount must be 1, got {value}"),
                        span,
                    ));
                    value
                }
                Some(_) => unreachable!("lower_fields enforces resource faceCount type"),
            };
            vec![
                CanonicalObjectEntry::new("fontProfile", CanonicalValue::String(font_profile)),
                CanonicalObjectEntry::new(
                    "shapingProfile",
                    CanonicalValue::String(shaping_profile),
                ),
                CanonicalObjectEntry::new("faceCount", CanonicalValue::Int(face_count)),
            ]
        }
        _ => {
            let names = fields.keys().cloned().collect::<Vec<_>>();
            for name in names {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                    format!(
                        "{} resource has no canonical metadata field {name}",
                        resource_kind_name(kind)
                    ),
                    span,
                ));
            }
            Vec::new()
        }
    };
    CanonicalObject::new(entries).expect("resource metadata keys are statically unique")
}

fn reject_resource_metadata_fields(
    fields: &mut BTreeMap<String, CanonicalValue>,
    names: &[&str],
    kind: ResourceKind,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for name in names {
        if fields.remove(*name).is_some() {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                format!(
                    "{} resource has no canonical metadata field {name}",
                    resource_kind_name(kind)
                ),
                span,
            ));
        }
    }
}

fn resource_string_metadata(
    fields: &mut BTreeMap<String, CanonicalValue>,
    name: &str,
    default: &str,
    allowed: &[&str],
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let value = match fields.remove(name) {
        Some(CanonicalValue::String(value)) => value,
        None => default.to_owned(),
        Some(_) => unreachable!("lower_fields enforces resource metadata string type"),
    };
    if !allowed.contains(&value.as_str()) {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::TYPE_INVALID_OPERATION,
            format!(
                "resource {name} must be one of {}, got {value}",
                allowed.join(", ")
            ),
            span,
        ));
    }
    value
}

fn resource_exact_string_metadata(
    fields: &mut BTreeMap<String, CanonicalValue>,
    name: &str,
    expected: &str,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let value = match fields.remove(name) {
        Some(CanonicalValue::String(value)) => value,
        None => expected.to_owned(),
        Some(_) => unreachable!("lower_fields enforces resource metadata string type"),
    };
    if value != expected {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::TYPE_INVALID_OPERATION,
            format!("resource {name} must be {expected}, got {value}"),
            span,
        ));
    }
    value
}

const fn resource_kind_name(kind: ResourceKind) -> &'static str {
    match kind {
        ResourceKind::Audio => "audio",
        ResourceKind::Image => "image",
        ResourceKind::Font => "font",
        ResourceKind::Texture => "texture",
        ResourceKind::Path => "path",
        ResourceKind::Shader => "shader",
        ResourceKind::Binary => "binary",
    }
}

fn lower_artwork(
    block: Option<&crate::ast::ArtworkBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalArtwork> {
    let block = block?;
    let mut expected = BTreeMap::new();
    expected.insert("primary", Expected::Reference(ReferenceKind::Resource));
    let fields = lower_fields(
        &block.fields,
        &expected,
        definitions,
        &BTreeSet::new(),
        resources,
        diagnostics,
        "artwork",
    );
    let Some(primary) = fields.get("primary").and_then(resource_reference) else {
        if !fields.contains_key("primary") {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                "artwork primary is required",
                block.span,
            ));
        }
        return Some(CanonicalArtwork::new(None));
    };
    if resources.get(&primary).copied() != Some(ResourceKind::Image) {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::RESOURCE_TYPE_MISMATCH,
            "artwork primary must reference an image resource",
            block.span,
        ));
    }
    Some(CanonicalArtwork::new(Some(primary)))
}

fn lower_sync(
    block: Option<&SyncBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalSync> {
    let block = block?;
    let mut previous = BTreeMap::<String, SourceSpan>::new();
    let mut primary_audio = None;
    let mut audio_offset = AudioOffset::new(0.0).expect("zero audio offset is finite");
    let mut preview = None;
    for field in &block.fields {
        let Some(name) = single_field_name(&field.path, field.span, diagnostics, "sync") else {
            continue;
        };
        if let Some(first_span) = previous.insert(name.clone(), field.path.span) {
            diagnostics.push(
                canonical_diagnostic(
                    DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                    format!("sync field {name} is assigned more than once"),
                    field.path.span,
                )
                .with_label(DiagnosticLabel::new(first_span, "first field assignment")),
            );
            continue;
        }
        match name.as_str() {
            "primaryAudio" => {
                let Some(raw) = lower_schema_value(&field.value, definitions, diagnostics) else {
                    continue;
                };
                let Some(value) = resolve_raw(
                    raw,
                    &Expected::Reference(ReferenceKind::Resource),
                    &BTreeSet::new(),
                    resources,
                    diagnostics,
                ) else {
                    continue;
                };
                if let Some(name) = resource_reference(&value) {
                    if resources.get(&name).copied() != Some(ResourceKind::Audio) {
                        diagnostics.push(canonical_diagnostic(
                            DiagnosticCode::RESOURCE_TYPE_MISMATCH,
                            "sync primaryAudio must reference an audio resource",
                            field.span,
                        ));
                    } else {
                        primary_audio = Some(name);
                    }
                }
            }
            "audioOffset" => {
                let Some(raw) = lower_schema_value(&field.value, definitions, diagnostics) else {
                    continue;
                };
                let Some(value) = resolve_raw(
                    raw,
                    &Expected::Time,
                    &BTreeSet::new(),
                    resources,
                    diagnostics,
                ) else {
                    continue;
                };
                if let Some(seconds) = time_value(&value) {
                    match AudioOffset::new(seconds) {
                        Ok(offset) => audio_offset = offset,
                        Err(_) => diagnostics.push(canonical_diagnostic(
                            DiagnosticCode::NUMERIC_NON_FINITE,
                            "audioOffset must be finite",
                            field.span,
                        )),
                    }
                }
            }
            "preview" => match &field.value {
                SchemaValue::Interval { start, end, span } => {
                    let Some(start) =
                        lower_expression(start, definitions, &mut Vec::new(), diagnostics)
                    else {
                        continue;
                    };
                    let Some(end) =
                        lower_expression(end, definitions, &mut Vec::new(), diagnostics)
                    else {
                        continue;
                    };
                    let Some(start) = resolve_raw(
                        start,
                        &Expected::Time,
                        &BTreeSet::new(),
                        resources,
                        diagnostics,
                    ) else {
                        continue;
                    };
                    let Some(end) = resolve_raw(
                        end,
                        &Expected::Time,
                        &BTreeSet::new(),
                        resources,
                        diagnostics,
                    ) else {
                        continue;
                    };
                    let (Some(start), Some(end)) = (time_value(&start), time_value(&end)) else {
                        continue;
                    };
                    match CanonicalPreview::new(start, end) {
                        Some(value) => preview = Some(value),
                        None => diagnostics.push(canonical_diagnostic(
                            DiagnosticCode::TYPE_INVALID_OPERATION,
                            "preview must be a finite non-empty interval with start >= 0s",
                            *span,
                        )),
                    }
                }
                _ => diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::TYPE_MISMATCH,
                    "sync preview must be an audio-time interval",
                    field.value.span(),
                )),
            },
            _ => diagnostics.push(canonical_diagnostic(
                DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                format!("sync has no field {name}"),
                field.path.span,
            )),
        }
    }
    if preview.is_some() && primary_audio.is_none() {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
            "sync preview requires primaryAudio",
            block.span,
        ));
    }
    Some(CanonicalSync::new(primary_audio, audio_offset, preview))
}

fn lower_fields(
    fields: &[SchemaField],
    expected: &BTreeMap<&str, Expected>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &str,
) -> BTreeMap<String, CanonicalValue> {
    let mut output = BTreeMap::new();
    let mut previous = BTreeMap::<String, SourceSpan>::new();
    for field in fields {
        let Some(name) = single_field_name(&field.path, field.span, diagnostics, owner) else {
            continue;
        };
        if let Some(first_span) = previous.insert(name.clone(), field.path.span) {
            diagnostics.push(
                canonical_diagnostic(
                    DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                    format!("{owner} field {name} is assigned more than once"),
                    field.path.span,
                )
                .with_label(DiagnosticLabel::new(first_span, "first field assignment")),
            );
            continue;
        }
        let Some(expected_type) = expected.get(name.as_str()) else {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                format!("{owner} has no field {name}"),
                field.path.span,
            ));
            continue;
        };
        let Some(raw) = lower_schema_value(&field.value, definitions, diagnostics) else {
            continue;
        };
        if let Some(value) = resolve_raw(raw, expected_type, contributors, resources, diagnostics) {
            output.insert(name, value);
        }
    }
    output
}

fn lower_schema_value(
    value: &SchemaValue,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    match value {
        SchemaValue::Expression(expression) => {
            lower_expression(expression, definitions, &mut Vec::new(), diagnostics)
        }
        SchemaValue::Interval { .. } => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_MISMATCH,
                "interval value is only valid for sync preview",
                value.span(),
            ));
            None
        }
        SchemaValue::CubicBezier { .. } => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "cubic-bezier value is not valid in the metadata graph",
                value.span(),
            ));
            None
        }
    }
}

fn lower_expression(
    expression: &SourceExpression,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    const_stack: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    match expression {
        SourceExpression::Literal { literal, span } => lower_literal(literal, *span, diagnostics),
        SourceExpression::Reference { name, span } => Some(RawValue::Reference {
            name: name.clone(),
            span: *span,
        }),
        SourceExpression::Array { elements, .. } => Some(RawValue::Array(
            elements
                .iter()
                .filter_map(|element| {
                    lower_expression(element, definitions, const_stack, diagnostics)
                })
                .collect(),
        )),
        SourceExpression::Object { entries, .. } => Some(RawValue::Object(
            entries
                .iter()
                .filter_map(|entry| {
                    lower_expression(&entry.value, definitions, const_stack, diagnostics).map(
                        |value| RawObjectEntry {
                            key: entry.key.clone(),
                            key_span: entry.key_span,
                            value,
                        },
                    )
                })
                .collect(),
        )),
        SourceExpression::Choose {
            arms,
            else_value,
            span,
        } => {
            for arm in arms {
                let condition = match crate::elaborator::evaluate_metadata_expression(
                    &arm.condition,
                    definitions,
                ) {
                    Ok(TypedValue::Bool(value)) => value,
                    Ok(_) => {
                        diagnostics.push(type_mismatch(
                            &Expected::Any,
                            "non-bool condition",
                            arm.condition.span(),
                        ));
                        return None;
                    }
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic);
                        return None;
                    }
                };
                if condition {
                    return lower_expression(&arm.value, definitions, const_stack, diagnostics);
                }
            }
            lower_expression(else_value, definitions, const_stack, diagnostics).or_else(|| {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    "metadata choose expression has no selected value",
                    *span,
                ));
                None
            })
        }
        SourceExpression::Name { name, span } => {
            if let Some(constant) = find_constant(definitions, name) {
                if const_stack.iter().any(|bound| bound == name) {
                    diagnostics.push(canonical_diagnostic(
                        DiagnosticCode::NAME_CYCLE,
                        format!("cyclic metadata constant {name}"),
                        *span,
                    ));
                    return None;
                }
                const_stack.push(name.clone());
                let result =
                    lower_expression(&constant.initializer, definitions, const_stack, diagnostics);
                const_stack.pop();
                result
            } else {
                evaluated_expression(expression, definitions, diagnostics)
            }
        }
        _ => evaluated_expression(expression, definitions, diagnostics),
    }
}

fn evaluated_expression(
    expression: &SourceExpression,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    match crate::elaborator::evaluate_metadata_expression(expression, definitions) {
        Ok(value) => raw_from_typed(value, expression.span(), diagnostics),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
    }
}

fn lower_literal(
    literal: &SourceLiteral,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    match literal {
        SourceLiteral::Bool(value) => Some(RawValue::Bool(*value)),
        SourceLiteral::Null => Some(RawValue::Null),
        SourceLiteral::Int(value) => Some(RawValue::Int(*value)),
        SourceLiteral::IntMagnitude(value) => match value.parse::<i64>() {
            Ok(value) => Some(RawValue::Int(value)),
            Err(_) => {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::NUMERIC_OVERFLOW,
                    "integer magnitude is outside the signed 64-bit range",
                    span,
                ));
                None
            }
        },
        SourceLiteral::Float(value) => finite_raw(RawValue::Float(*value), span, diagnostics),
        SourceLiteral::String(value) => Some(RawValue::String(value.clone())),
        SourceLiteral::Time(value) => finite_raw(RawValue::Time(*value), span, diagnostics),
        SourceLiteral::Beat(value) => CanonicalBeat::new(value.numerator(), value.denominator())
            .ok()
            .map(RawValue::Beat),
        SourceLiteral::Length(_) | SourceLiteral::Angle(_) => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "length and angle are not permitted in canonical metadata values",
                span,
            ));
            None
        }
        SourceLiteral::Color(value) => Some(RawValue::Color(canonical_color(*value))),
        SourceLiteral::Line(value) => Some(RawValue::Reference {
            name: value.clone(),
            span,
        }),
    }
}

fn raw_from_typed(
    value: TypedValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    match value {
        TypedValue::Bool(value) => Some(RawValue::Bool(value)),
        TypedValue::Int(value) => Some(RawValue::Int(value)),
        TypedValue::Float(value) => finite_raw(RawValue::Float(value), span, diagnostics),
        TypedValue::String(value) => Some(RawValue::String(value)),
        TypedValue::Time(value) => finite_raw(RawValue::Time(value), span, diagnostics),
        TypedValue::Beat(value) => CanonicalBeat::new(value.numerator(), value.denominator())
            .ok()
            .map(RawValue::Beat),
        TypedValue::Color(value) => Some(RawValue::Color(canonical_color(value))),
        TypedValue::Line(value) => Some(RawValue::Reference { name: value, span }),
        TypedValue::Array { values, .. } => Some(RawValue::Array(
            values
                .into_iter()
                .filter_map(|value| raw_from_typed(value, span, diagnostics))
                .collect(),
        )),
        TypedValue::Length(_) | TypedValue::Angle(_) | TypedValue::Vec2(..) => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "value type is not permitted in canonical metadata",
                span,
            ));
            None
        }
        TypedValue::GeneratorRange(_) => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::TYPE_INVALID_OPERATION,
                "generator range is not permitted in canonical metadata",
                span,
            ));
            None
        }
    }
}

fn canonical_color(value: crate::ast::Color) -> CanonicalColor {
    CanonicalColor::from_linear(value.to_linear())
        .expect("source Color::to_linear must produce valid canonical components")
}

fn finite_raw(
    value: RawValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<RawValue> {
    let finite = match &value {
        RawValue::Float(value) | RawValue::Time(value) => value.is_finite(),
        _ => true,
    };
    if finite {
        Some(value)
    } else {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::NUMERIC_NON_FINITE,
            "metadata numeric value must be finite",
            span,
        ));
        None
    }
}

fn resolve_raw(
    raw: RawValue,
    expected: &Expected,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalValue> {
    match raw {
        RawValue::Reference { name, span } => {
            resolve_reference(name, span, expected, contributors, resources, diagnostics)
        }
        RawValue::Array(values) => {
            let expected_element = match expected {
                Expected::Array(element) => Some(element.as_ref()),
                _ => None,
            };
            let mut output = Vec::new();
            for value in values {
                let Some(value) = resolve_raw(
                    value,
                    expected_element.unwrap_or(&Expected::Any),
                    contributors,
                    resources,
                    diagnostics,
                ) else {
                    continue;
                };
                output.push(value);
            }
            let element_type = if let Some(expected_element) = expected_element {
                Some(expected_type_to_value_type(expected_element))
            } else {
                output.first().map(CanonicalValue::value_type)
            };
            let Some(element_type) = element_type else {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    "empty custom arrays require an explicit element type",
                    SourceSpan::new(0, 0),
                ));
                return None;
            };
            if output
                .iter()
                .any(|value| value.value_type() != element_type)
            {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::TYPE_MISMATCH,
                    "array elements must have one homogeneous type",
                    SourceSpan::new(0, 0),
                ));
                return None;
            }
            match CanonicalValue::typed_array(element_type, output) {
                Ok(value) => value_matches_expected(value, expected, diagnostics),
                Err(_) => None,
            }
        }
        RawValue::Object(entries) => {
            if !matches!(
                expected,
                Expected::Any | Expected::Object | Expected::StringObject
            ) {
                diagnostics.push(type_mismatch(expected, "object", SourceSpan::new(0, 0)));
                return None;
            }
            let mut keys = BTreeSet::new();
            let mut output = Vec::new();
            for entry in entries {
                if !keys.insert(entry.key.clone()) {
                    diagnostics.push(canonical_diagnostic(
                        DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                        format!("custom object key {} is repeated", entry.key),
                        entry.key_span,
                    ));
                    continue;
                }
                let Some(value) = resolve_raw(
                    entry.value,
                    &Expected::Any,
                    contributors,
                    resources,
                    diagnostics,
                ) else {
                    continue;
                };
                if matches!(expected, Expected::StringObject)
                    && !matches!(value, CanonicalValue::String(_))
                {
                    diagnostics.push(type_mismatch(
                        &Expected::String,
                        &format_value_type(&value),
                        entry.key_span,
                    ));
                    continue;
                }
                output.push(CanonicalObjectEntry::new(entry.key, value));
            }
            let object = CanonicalObject::new(output).expect("duplicate keys were checked");
            value_matches_expected(CanonicalValue::Object(object), expected, diagnostics)
        }
        value => {
            let value = raw_to_canonical(value);
            if matches!(expected, Expected::Number) && matches!(value, CanonicalValue::Int(_)) {
                return value_matches_expected(
                    match value {
                        CanonicalValue::Int(value) => CanonicalValue::Float(value as f64),
                        _ => unreachable!(),
                    },
                    &Expected::Float,
                    diagnostics,
                );
            }
            value_matches_expected(value, expected, diagnostics)
        }
    }
}

fn resolve_reference(
    name: String,
    span: SourceSpan,
    expected: &Expected,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalValue> {
    let kind = match expected {
        Expected::Reference(kind) => Some(*kind),
        _ => None,
    };
    match kind {
        Some(ReferenceKind::Contributor) if contributors.contains(&name) => {
            Some(CanonicalValue::ContributorReference(name))
        }
        Some(ReferenceKind::Resource) if resources.contains_key(&name) => {
            Some(CanonicalValue::ResourceReference(name))
        }
        Some(ReferenceKind::Contributor) | Some(ReferenceKind::Resource) => {
            if resources.contains_key(&name) || contributors.contains(&name) {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::RESOURCE_TYPE_MISMATCH,
                    "metadata reference has the wrong declaration type",
                    span,
                ));
            } else {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::NAME_UNKNOWN,
                    format!("unknown metadata reference @{name}"),
                    span,
                ));
            }
            None
        }
        None => match (contributors.contains(&name), resources.contains_key(&name)) {
            (true, false) => Some(CanonicalValue::ContributorReference(name)),
            (false, true) => Some(CanonicalValue::ResourceReference(name)),
            (true, true) => {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::TYPE_INVALID_OPERATION,
                    "untyped custom reference is ambiguous between contributor and resource",
                    span,
                ));
                None
            }
            (false, false) => {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::NAME_UNKNOWN,
                    format!("unknown metadata reference @{name}"),
                    span,
                ));
                None
            }
        },
    }
}

fn value_matches_expected(
    value: CanonicalValue,
    expected: &Expected,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalValue> {
    let accepted = match expected {
        Expected::Any => true,
        Expected::Int => matches!(value, CanonicalValue::Int(_)),
        Expected::Float => matches!(value, CanonicalValue::Float(_)),
        Expected::Number => matches!(value, CanonicalValue::Float(_) | CanonicalValue::Int(_)),
        Expected::String => matches!(value, CanonicalValue::String(_)),
        Expected::Time => matches!(value, CanonicalValue::Time(_)),
        Expected::Object | Expected::StringObject => matches!(value, CanonicalValue::Object(_)),
        Expected::Array(element) => {
            matches!(value, CanonicalValue::Array { ref element_type, .. } if expected_type_to_value_type(element) == *element_type)
        }
        Expected::Reference(kind) => matches!(
            (kind, &value),
            (
                ReferenceKind::Contributor,
                CanonicalValue::ContributorReference(_)
            ) | (
                ReferenceKind::Resource,
                CanonicalValue::ResourceReference(_)
            )
        ),
    };
    if accepted {
        Some(value)
    } else {
        diagnostics.push(type_mismatch(
            expected,
            &format_value_type(&value),
            SourceSpan::new(0, 0),
        ));
        None
    }
}

fn raw_to_canonical(value: RawValue) -> CanonicalValue {
    match value {
        RawValue::Null => CanonicalValue::Null,
        RawValue::Bool(value) => CanonicalValue::Bool(value),
        RawValue::Int(value) => CanonicalValue::Int(value),
        RawValue::Float(value) => CanonicalValue::Float(value),
        RawValue::String(value) => CanonicalValue::String(value),
        RawValue::Time(value) => CanonicalValue::Time(value),
        RawValue::Beat(value) => CanonicalValue::Beat(value),
        RawValue::Color(value) => CanonicalValue::Color(value),
        RawValue::Reference { name, .. } => CanonicalValue::String(name),
        RawValue::Array(values) => CanonicalValue::Array {
            element_type: CanonicalValueType::Null,
            values: values.into_iter().map(raw_to_canonical).collect(),
        },
        RawValue::Object(entries) => CanonicalValue::Object(
            CanonicalObject::new(
                entries
                    .into_iter()
                    .map(|entry| {
                        CanonicalObjectEntry::new(entry.key, raw_to_canonical(entry.value))
                    })
                    .collect(),
            )
            .expect("raw object duplicate checking occurs during resolution"),
        ),
    }
}

fn expected_type_to_value_type(expected: &Expected) -> CanonicalValueType {
    match expected {
        Expected::Int => CanonicalValueType::Int,
        Expected::Float | Expected::Number => CanonicalValueType::Float,
        Expected::String => CanonicalValueType::String,
        Expected::Time => CanonicalValueType::Time,
        Expected::Reference(ReferenceKind::Contributor) => CanonicalValueType::ContributorReference,
        Expected::Reference(ReferenceKind::Resource) => CanonicalValueType::ResourceReference,
        Expected::Array(element) => {
            CanonicalValueType::Array(Box::new(expected_type_to_value_type(element)))
        }
        Expected::Object | Expected::StringObject => CanonicalValueType::Object,
        Expected::Any => CanonicalValueType::Null,
    }
}

fn find_constant<'a>(
    definitions: Option<&'a crate::ast::DefinitionsBlock>,
    name: &str,
) -> Option<&'a crate::ast::ConstDeclaration> {
    definitions?
        .declarations
        .iter()
        .find_map(|definition| match definition {
            Definition::Const(constant) if constant.name == name => Some(constant),
            _ => None,
        })
}

fn string_field(
    fields: &BTreeMap<String, CanonicalValue>,
    name: &str,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
    label: &str,
) -> Option<String> {
    match fields.get(name).and_then(string_value) {
        Some(value) => Some(value),
        None => {
            if !fields.contains_key(name) {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD,
                    format!("{label} is required"),
                    span,
                ));
            }
            None
        }
    }
}

fn string_value(value: &CanonicalValue) -> Option<String> {
    match value {
        CanonicalValue::String(value) => Some(value.clone()),
        _ => None,
    }
}

fn array_values(value: &CanonicalValue) -> Option<&[CanonicalValue]> {
    match value {
        CanonicalValue::Array { values, .. } => Some(values),
        _ => None,
    }
}

fn resource_reference(value: &CanonicalValue) -> Option<String> {
    match value {
        CanonicalValue::ResourceReference(value) => Some(value.clone()),
        _ => None,
    }
}

fn time_value(value: &CanonicalValue) -> Option<f64> {
    match value {
        CanonicalValue::Time(value) => Some(*value),
        _ => None,
    }
}

fn single_field_name(
    path: &FieldPath,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &str,
) -> Option<String> {
    if path.segments.len() == 1 {
        Some(path.segments[0].clone())
    } else {
        diagnostics.push(canonical_diagnostic(
            DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
            format!("{owner} field path must contain one name"),
            span,
        ));
        None
    }
}

fn canonical_resource_kind(kind: ResourceKind) -> CanonicalResourceKind {
    match kind {
        ResourceKind::Audio => CanonicalResourceKind::Audio,
        ResourceKind::Image => CanonicalResourceKind::Image,
        ResourceKind::Font => CanonicalResourceKind::Font,
        ResourceKind::Texture => CanonicalResourceKind::Texture,
        ResourceKind::Path => CanonicalResourceKind::Path,
        ResourceKind::Shader => CanonicalResourceKind::Shader,
        ResourceKind::Binary => CanonicalResourceKind::Binary,
    }
}

fn valid_workspace_member_path(path: &str) -> bool {
    if path.is_empty()
        || path.contains('\\')
        || path.contains('\0')
        || path.starts_with('/')
        || path.as_bytes().get(1) == Some(&b':')
    {
        return false;
    }
    if let Some(colon) = path.find(':') {
        let scheme = &path[..colon];
        if !scheme.is_empty()
            && scheme
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic())
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
        {
            return false;
        }
    }
    !path
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
}

fn type_mismatch(expected: &Expected, actual: &str, span: SourceSpan) -> Diagnostic {
    canonical_diagnostic(
        DiagnosticCode::TYPE_MISMATCH,
        format!("expected {}, found {actual}", expected_name(expected)),
        span,
    )
}

fn expected_name(expected: &Expected) -> &'static str {
    match expected {
        Expected::Any => "any value",
        Expected::Int => "int",
        Expected::Float => "float",
        Expected::Number => "number",
        Expected::String => "string",
        Expected::Time => "time",
        Expected::Object => "object",
        Expected::StringObject => "string object",
        Expected::Array(_) => "array",
        Expected::Reference(ReferenceKind::Contributor) => "contributor reference",
        Expected::Reference(ReferenceKind::Resource) => "resource reference",
    }
}

fn format_value_type(value: &CanonicalValue) -> String {
    format!("{:?}", value.value_type())
}

fn canonical_diagnostic(
    code: DiagnosticCode,
    message: impl Into<String>,
    span: SourceSpan,
) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Canonical, message, span)
}
