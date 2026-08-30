//! I3.3 source-to-canonical lowering for metadata, resources, artwork, sync,
//! and typed custom values.

use std::collections::{BTreeMap, BTreeSet};

use fcs_model::{
    AudioOffset, Beat as CanonicalBeat, CanonicalActiveInterval, CanonicalArcDirection,
    CanonicalArtwork, CanonicalChart, CanonicalChartError, CanonicalColor, CanonicalCompilation,
    CanonicalContributor, CanonicalCredit, CanonicalCreditRole, CanonicalDescriptorDomain,
    CanonicalDescriptorKind, CanonicalDescriptorRoot, CanonicalDescriptorTable,
    CanonicalExpressionType, CanonicalExpressionValue, CanonicalGlyphPlacement, CanonicalGlyphRun,
    CanonicalGradientSpread, CanonicalGradientStop, CanonicalImageRepeat, CanonicalImageSampling,
    CanonicalLineGraph, CanonicalMetadata, CanonicalNoteSet, CanonicalObject, CanonicalObjectEntry,
    CanonicalPathCommand, CanonicalPatternTransform, CanonicalPreview, CanonicalProfile,
    CanonicalProfileFeature, CanonicalPropertyDescriptor, CanonicalRenderAttachment,
    CanonicalRenderClip, CanonicalRenderColorSpace, CanonicalRenderComposite,
    CanonicalRenderFillRule, CanonicalRenderGeometry, CanonicalRenderGeometryData,
    CanonicalRenderLayer, CanonicalRenderNode, CanonicalRenderNodeKind, CanonicalRenderNodeSpec,
    CanonicalRenderPaint, CanonicalRenderPaintData, CanonicalRenderPass, CanonicalRenderPath,
    CanonicalRenderScene, CanonicalRenderSceneSpec, CanonicalRenderStroke,
    CanonicalRequiredExtension, CanonicalResource, CanonicalResourceBundle, CanonicalResourceKind,
    CanonicalSourceVersion, CanonicalStrokeCap, CanonicalStrokeJoin, CanonicalSync,
    CanonicalTextualId, CanonicalValue, CanonicalValueType, CanonicalViewport, ChartTimeMap,
    DeclaredSha256, DistributionMetadata, EntityKind, StableId, StableIdRegistry,
};

use crate::ast::{
    Definition, DefinitionsBlock, Document, DocumentProfile, ExtensionRequirement, FieldPath,
    MetaBlock, OrderedObject, ProfileFeature, RenderBodyItem, RenderItem, ResourceKind,
    SchemaField, SchemaValue, SourceExpression, SourceLiteral, SourceSpan, SyncBlock,
    TopLevelBlockKind, TypedValue,
};
use crate::custom::CustomValueLimits;
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
        let descriptors = expanded.canonical_runtime_descriptors()?;
        let scroll = self.canonical_scroll_set_for_graph(&time_map, &lines)?;
        let source_version = CanonicalSourceVersion::new(self.source_version.to_string())
            .map_err(|error| vec![chart_diagnostic(error, self.format.span)])?;
        let required_extensions = lower_required_extensions(self)?;

        let chart = CanonicalChart::new(
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
        );
        Ok(match descriptors {
            Some(descriptors) => chart.with_descriptors(descriptors),
            None => chart,
        })
    }

    /// Source-aware canonical boundary for a document carrying a Render scene.
    pub fn canonical_chart_with_source(
        &self,
        source: &str,
        limits: CompileTimeLimits,
    ) -> Result<CanonicalChart, Vec<Diagnostic>> {
        let chart = self.canonical_chart(limits)?;
        self.lower_source_render(source, chart, None)
    }

    fn lower_source_render(
        &self,
        source: &str,
        mut chart: CanonicalChart,
        resource_bundle: Option<&CanonicalResourceBundle>,
    ) -> Result<CanonicalChart, Vec<Diagnostic>> {
        let Some(crate::ast::TopLevelBlock::Render(block)) =
            self.top_level(TopLevelBlockKind::Render)
        else {
            return Ok(chart);
        };
        let scene = crate::parser::parse_render_scene(source, block).into_result()?;
        let (mut render, render_descriptors) = lower_render_scene(
            &scene,
            chart.metadata().resources(),
            resource_bundle,
            chart.time_map(),
            chart.lines(),
            chart.notes(),
            self.definitions.as_ref(),
            self.format.span,
        )?;
        let (descriptors, mapping) =
            merge_render_descriptors(chart.descriptors(), &render_descriptors, self.format.span)?;
        render
            .remap_descriptors(&mapping)
            .map_err(|error| vec![render_error(format!("{error:?}"), self.format.span)])?;
        chart = chart.with_descriptors(descriptors).with_render(render);
        Ok(chart)
    }

    /// Assembles the FCS §17 CanonicalCompilation product.
    ///
    /// Native FCS compile paths produce empty DistributionMetadata. Conversion
    /// importers may later supply restricted provenance without source AST.
    /// This boundary never retains workspace absolute paths in distribution
    /// metadata; resource bytes remain in the separate opaque bundle.
    pub fn canonical_compilation(
        &self,
        limits: CompileTimeLimits,
        workspace_root: impl AsRef<std::path::Path>,
        resource_limits: crate::resource::ResourceLimits,
    ) -> Result<CanonicalCompilation, Vec<Diagnostic>> {
        let chart = self.canonical_chart(limits)?;
        let resources = self.canonical_resource_bundle(workspace_root, resource_limits)?;
        Ok(CanonicalCompilation::new(
            chart,
            resources,
            DistributionMetadata::empty(),
        ))
    }

    pub fn canonical_compilation_with_source(
        &self,
        source: &str,
        limits: CompileTimeLimits,
        workspace_root: impl AsRef<std::path::Path>,
        resource_limits: crate::resource::ResourceLimits,
    ) -> Result<CanonicalCompilation, Vec<Diagnostic>> {
        let chart = self.canonical_chart(limits)?;
        let resources = self.canonical_resource_bundle(workspace_root, resource_limits)?;
        let chart = self.lower_source_render(source, chart, Some(&resources))?;
        Ok(CanonicalCompilation::new(
            chart,
            resources,
            DistributionMetadata::empty(),
        ))
    }

    /// Lowers the source metadata surface into an immutable canonical graph.
    ///
    /// This operation validates logical workspace member paths but never opens
    /// them, follows symlinks, reads bytes, or compares a declared hash with an
    /// input file. Use `canonical_resource_bundle` for that explicit-root I5
    /// boundary.
    pub fn canonical_metadata(&self) -> Result<CanonicalMetadata, Vec<Diagnostic>> {
        self.canonical_metadata_with_limits(CustomValueLimits::default())
    }

    /// Lowers metadata with an explicit typed-custom limit profile.
    pub fn canonical_metadata_with_limits(
        &self,
        limits: CustomValueLimits,
    ) -> Result<CanonicalMetadata, Vec<Diagnostic>> {
        lower_document(self, limits)
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

fn merge_render_descriptors(
    core: Option<&CanonicalDescriptorTable>,
    render: &CanonicalDescriptorTable,
    span: SourceSpan,
) -> Result<(CanonicalDescriptorTable, Vec<usize>), Vec<Diagnostic>> {
    let (mut descriptors, mut roots) = core
        .map(|table| (table.descriptors().to_vec(), table.roots().to_vec()))
        .unwrap_or_default();
    if core.is_some() {
        let offset = descriptors.len();
        roots.extend(
            render
                .roots()
                .iter()
                .map(|root| {
                    CanonicalDescriptorRoot::new(
                        root.target_path().to_owned(),
                        root.owner(),
                        root.descriptor() + offset,
                    )
                    .map_err(|error| render_error(format!("{error:?}"), span))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| vec![error])?,
        );
        descriptors.extend(render.descriptors().iter().cloned());
    } else {
        descriptors = render.descriptors().to_vec();
        roots = render.roots().to_vec();
    }
    let merged = CanonicalDescriptorTable::new(descriptors, roots)
        .map_err(|error| vec![render_error(format!("{error:?}"), span)])?;
    let mapping = render
        .descriptors()
        .iter()
        .map(|descriptor| {
            merged
                .descriptors()
                .iter()
                .position(|candidate| candidate == descriptor)
                .ok_or_else(|| {
                    vec![render_error(
                        "Render descriptor was lost during merge",
                        span,
                    )]
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((merged, mapping))
}

fn render_error(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::TYPE_INVALID_OPERATION,
        DiagnosticStage::Canonical,
        message,
        span,
    )
}

fn render_paint_error(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::RENDER_INVALID_PAINT,
        DiagnosticStage::Canonical,
        message,
        span,
    )
}

fn render_clip_error(message: impl Into<String>, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::RENDER_INVALID_CLIP,
        DiagnosticStage::Canonical,
        message,
        span,
    )
}

fn render_resource_error(
    code: DiagnosticCode,
    message: impl Into<String>,
    span: SourceSpan,
) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Canonical, message, span)
}

fn render_field<'a>(fields: &'a [SchemaField], path: &str) -> Option<&'a SchemaField> {
    fields
        .iter()
        .find(|field| field.path.segments.join(".") == path)
}

fn render_body_field<'a>(items: &'a [RenderBodyItem], path: &str) -> Option<&'a SchemaField> {
    items.iter().find_map(|item| match item {
        RenderBodyItem::Field(field) if field.path.segments.join(".") == path => {
            Some(field.as_ref())
        }
        _ => None,
    })
}

fn render_unique_body_field<'a>(
    items: &'a [RenderBodyItem],
    path: &str,
) -> Result<Option<&'a SchemaField>, Diagnostic> {
    let mut fields = items.iter().filter_map(|item| match item {
        RenderBodyItem::Field(field) if field.path.segments.join(".") == path => {
            Some(field.as_ref())
        }
        _ => None,
    });
    let first = fields.next();
    if let Some(duplicate) = fields.next() {
        return Err(Diagnostic::new(
            DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
            DiagnosticStage::Canonical,
            format!("Render field {path} is assigned more than once"),
            duplicate.span,
        )
        .with_label(DiagnosticLabel::new(
            first.expect("a duplicate has an earlier field").span,
            "first field assignment",
        )));
    }
    Ok(first)
}

fn render_scoped_body_path(scope: Option<&str>, path: &str) -> String {
    scope.map_or_else(|| path.to_owned(), |scope| format!("{scope}.{path}"))
}

fn render_scoped_body_field<'a>(
    items: &'a [RenderBodyItem],
    scope: Option<&str>,
    path: &str,
) -> Result<Option<&'a SchemaField>, Diagnostic> {
    let path = render_scoped_body_path(scope, path);
    if scope.is_some() {
        render_unique_body_field(items, &path)
    } else {
        Ok(render_body_field(items, &path))
    }
}

fn render_value(
    field: &SchemaField,
    definitions: Option<&DefinitionsBlock>,
) -> Result<TypedValue, Diagnostic> {
    let SchemaValue::Expression(expression) = &field.value else {
        return Err(render_error(
            "Render field must be a compile-time expression",
            field.span,
        ));
    };
    crate::elaborator::evaluate_metadata_expression(expression, definitions)
}

fn render_font_references(
    field: &SchemaField,
    definitions: Option<&DefinitionsBlock>,
) -> Result<Vec<String>, Diagnostic> {
    let SchemaValue::Expression(_) = &field.value else {
        return Err(render_error(
            "fallbackFonts must be a compile-time font reference array",
            field.span,
        ));
    };
    let value = render_value(field, definitions)?;
    let TypedValue::Array { values, .. } = value else {
        return Err(render_error(
            "fallbackFonts must be a compile-time font reference array",
            field.span,
        ));
    };
    values
        .into_iter()
        .map(|value| match value {
            TypedValue::Line(name) => Ok(name),
            other => Err(render_error(
                format!(
                    "fallbackFonts entries must be font references, found {}",
                    other.ty()
                ),
                field.span,
            )),
        })
        .collect()
}

fn render_empty_array(
    field: &SchemaField,
    name: &str,
    definitions: Option<&DefinitionsBlock>,
) -> Result<(), Diagnostic> {
    if matches!(
        &field.value,
        SchemaValue::Expression(SourceExpression::Array { elements, .. }) if elements.is_empty()
    ) {
        Ok(())
    } else if matches!(
        render_value(field, definitions)?,
        TypedValue::Array { values, .. } if values.is_empty()
    ) {
        Ok(())
    } else {
        Err(render_error(
            format!("{name} must be an explicit empty compile-time array"),
            field.span,
        ))
    }
}

enum RenderPaintExpression<'a> {
    Solid(&'a SourceExpression),
    LinearGradient {
        start: &'a SourceExpression,
        end: &'a SourceExpression,
        spread: CanonicalGradientSpread,
        stops: Vec<(f64, &'a SourceExpression)>,
    },
    RadialGradient {
        start_center: &'a SourceExpression,
        start_radius: &'a SourceExpression,
        end_center: &'a SourceExpression,
        end_radius: &'a SourceExpression,
        spread: CanonicalGradientSpread,
        stops: Vec<(f64, &'a SourceExpression)>,
    },
    ImagePattern {
        resource: String,
        resource_span: SourceSpan,
    },
}

#[derive(Clone, Copy)]
struct RenderPatternSpec {
    transform: CanonicalPatternTransform,
    repeat: CanonicalImageRepeat,
    sampling: Option<CanonicalImageSampling>,
}

struct RenderStrokeSpec {
    id: StableId,
    paint: CanonicalRenderPaint,
    width: usize,
    cap: CanonicalStrokeCap,
    join: CanonicalStrokeJoin,
    miter_limit: f64,
    dash_offset: usize,
    dash: Vec<f64>,
}

fn render_gradient_stops<'a>(
    stops: &'a SourceExpression,
    field_span: SourceSpan,
    gradient_name: &str,
    definitions: Option<&DefinitionsBlock>,
) -> Result<Vec<(f64, &'a SourceExpression)>, Diagnostic> {
    let SourceExpression::Array { elements, .. } = stops else {
        return Err(render_error(
            format!("{gradient_name} stops must be an array of stop(offset, color)"),
            field_span,
        ));
    };
    let mut parsed_stops = Vec::with_capacity(elements.len());
    for stop in elements {
        let (callee, arguments) = match stop {
            SourceExpression::Call {
                callee, arguments, ..
            } => (callee, arguments),
            _ => {
                return Err(render_error(
                    format!("{gradient_name} stops must use stop(offset, color)"),
                    stop.span(),
                ));
            }
        };
        let SourceExpression::Name { name, .. } = callee.as_ref() else {
            return Err(render_error(
                format!("{gradient_name} stops must use stop(offset, color)"),
                stop.span(),
            ));
        };
        if name != "stop" {
            return Err(render_error(
                format!("{gradient_name} stops must use stop(offset, color)"),
                stop.span(),
            ));
        }
        let [offset, color] = arguments.as_slice() else {
            return Err(render_error(
                "stop requires offset and color arguments",
                stop.span(),
            ));
        };
        let offset = render_float(
            crate::elaborator::evaluate_metadata_expression(offset, definitions)?,
            offset.span(),
        )?;
        parsed_stops.push((offset, color));
    }
    Ok(parsed_stops)
}

fn render_gradient_spread(
    value: &SourceExpression,
    definitions: Option<&DefinitionsBlock>,
) -> Result<CanonicalGradientSpread, Diagnostic> {
    let spread = crate::elaborator::evaluate_metadata_expression(value, definitions)?;
    let span = value.span();
    match render_string(spread, span)?.as_str() {
        "pad" => Ok(CanonicalGradientSpread::Pad),
        "repeat" => Ok(CanonicalGradientSpread::Repeat),
        "reflect" => Ok(CanonicalGradientSpread::Reflect),
        value => Err(render_error(
            format!("unsupported Render gradient spread {value}"),
            span,
        )),
    }
}

fn render_paint_expression(
    field: &SchemaField,
    definitions: Option<&DefinitionsBlock>,
) -> Result<RenderPaintExpression<'_>, Diagnostic> {
    let SchemaValue::Expression(expression) = &field.value else {
        return Err(render_error(
            "Render paint must be an expression",
            field.span,
        ));
    };
    if let SourceExpression::Call {
        callee, arguments, ..
    } = expression
        && let SourceExpression::Name { name, .. } = callee.as_ref()
        && name == "solid"
    {
        let [argument] = arguments.as_slice() else {
            return Err(render_error(
                "solid paint requires one color argument",
                field.span,
            ));
        };
        return Ok(RenderPaintExpression::Solid(argument));
    }
    if let SourceExpression::Call {
        callee, arguments, ..
    } = expression
        && let SourceExpression::Name { name, .. } = callee.as_ref()
        && name == "linearGradient"
    {
        let [start, end, stops, spread] = arguments.as_slice() else {
            return Err(render_error(
                "linearGradient requires start, end, stops, and spread arguments",
                field.span,
            ));
        };
        let parsed_stops = render_gradient_stops(stops, field.span, "linearGradient", definitions)?;
        let spread = render_gradient_spread(spread, definitions)?;
        return Ok(RenderPaintExpression::LinearGradient {
            start,
            end,
            spread,
            stops: parsed_stops,
        });
    }
    if let SourceExpression::Call {
        callee, arguments, ..
    } = expression
        && let SourceExpression::Name { name, .. } = callee.as_ref()
        && name == "radialGradient"
    {
        let [
            start_center,
            start_radius,
            end_center,
            end_radius,
            stops,
            spread,
        ] = arguments.as_slice()
        else {
            return Err(render_error(
                "radialGradient requires startCenter, startRadius, endCenter, endRadius, stops, and spread arguments",
                field.span,
            ));
        };
        let parsed_stops = render_gradient_stops(stops, field.span, "radialGradient", definitions)?;
        let spread = render_gradient_spread(spread, definitions)?;
        return Ok(RenderPaintExpression::RadialGradient {
            start_center,
            start_radius,
            end_center,
            end_radius,
            spread,
            stops: parsed_stops,
        });
    }
    if let SourceExpression::Call {
        callee, arguments, ..
    } = expression
        && let SourceExpression::Name { name, .. } = callee.as_ref()
        && name == "imagePattern"
    {
        let [resource] = arguments.as_slice() else {
            return Err(render_paint_error(
                "imagePattern requires one resource argument",
                field.span,
            ));
        };
        let SourceExpression::Reference { name, span } = resource else {
            return Err(render_paint_error(
                "imagePattern requires a static resource reference",
                resource.span(),
            ));
        };
        return Ok(RenderPaintExpression::ImagePattern {
            resource: name.clone(),
            resource_span: *span,
        });
    }
    Err(render_error(
        "Render paint must use solid(color), linearGradient(start, end, stops, spread), radialGradient(startCenter, startRadius, endCenter, endRadius, stops, spread), or imagePattern(resource)",
        field.span,
    ))
}

fn render_value_or<T>(
    fields: &[SchemaField],
    path: &str,
    default: T,
    definitions: Option<&DefinitionsBlock>,
    convert: impl FnOnce(TypedValue) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    match render_field(fields, path) {
        Some(field) => convert(render_value(field, definitions)?),
        None => Ok(default),
    }
}

fn render_body_value_or<T>(
    items: &[RenderBodyItem],
    path: &str,
    default: T,
    definitions: Option<&DefinitionsBlock>,
    convert: impl FnOnce(TypedValue) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    match render_body_field(items, path) {
        Some(field) => convert(render_value(field, definitions)?),
        None => Ok(default),
    }
}

fn render_attachment(
    expression: &SourceExpression,
    definitions: Option<&DefinitionsBlock>,
    lines: &CanonicalLineGraph,
    notes: &CanonicalNoteSet,
) -> Result<CanonicalRenderAttachment, Diagnostic> {
    if let SourceExpression::Call {
        callee, arguments, ..
    } = expression
        && let SourceExpression::Name { name, .. } = callee.as_ref()
        && matches!(name.as_str(), "line" | "note")
    {
        let [target] = arguments.as_slice() else {
            return Err(render_error(
                format!("{name} attachment requires one resource reference"),
                expression.span(),
            ));
        };
        let SourceExpression::Reference {
            name: target_name,
            span,
        } = target
        else {
            return Err(render_error(
                format!("{name} attachment requires a static reference"),
                target.span(),
            ));
        };
        return match name.as_str() {
            "line" => lines
                .line_by_textual_id(target_name)
                .map(|line| CanonicalRenderAttachment::Line(line.id().clone()))
                .ok_or_else(|| {
                    render_error(
                        format!("Render line attachment references unknown line {target_name}"),
                        *span,
                    )
                }),
            "note" => notes
                .note_by_textual_id(target_name)
                .map(|note| CanonicalRenderAttachment::Note(note.id().clone()))
                .ok_or_else(|| {
                    render_error(
                        format!("Render note attachment references unknown note {target_name}"),
                        *span,
                    )
                }),
            _ => unreachable!("attachment constructor was checked above"),
        };
    }

    let value = crate::elaborator::evaluate_metadata_expression(expression, definitions)?;
    match render_string(value, expression.span())?.as_str() {
        "world" => Ok(CanonicalRenderAttachment::World),
        "screen" => Ok(CanonicalRenderAttachment::Screen),
        other => Err(render_error(
            format!("unsupported Render space {other}"),
            expression.span(),
        )),
    }
}

fn render_length(value: TypedValue, span: SourceSpan) -> Result<f64, Diagnostic> {
    match value {
        TypedValue::Length(value) if value.is_finite() => Ok(value),
        other => Err(render_error(
            format!("expected length, found {}", other.ty()),
            span,
        )),
    }
}

fn render_stroke_cap(
    value: TypedValue,
    span: SourceSpan,
) -> Result<CanonicalStrokeCap, Diagnostic> {
    match render_string(value, span)?.as_str() {
        "butt" => Ok(CanonicalStrokeCap::Butt),
        "round" => Ok(CanonicalStrokeCap::Round),
        "square" => Ok(CanonicalStrokeCap::Square),
        value => Err(render_error(
            format!("unsupported Render stroke cap {value}"),
            span,
        )),
    }
}

fn render_stroke_join(
    value: TypedValue,
    span: SourceSpan,
) -> Result<CanonicalStrokeJoin, Diagnostic> {
    match render_string(value, span)?.as_str() {
        "miter" => Ok(CanonicalStrokeJoin::Miter),
        "round" => Ok(CanonicalStrokeJoin::Round),
        "bevel" => Ok(CanonicalStrokeJoin::Bevel),
        value => Err(render_error(
            format!("unsupported Render stroke join {value}"),
            span,
        )),
    }
}

fn render_dash(
    field: &SchemaField,
    definitions: Option<&DefinitionsBlock>,
) -> Result<Vec<f64>, Diagnostic> {
    if matches!(
        &field.value,
        SchemaValue::Expression(SourceExpression::Array { elements, .. }) if elements.is_empty()
    ) {
        return Ok(Vec::new());
    }

    let value = render_value(field, definitions)?;
    let TypedValue::Array { values, .. } = value else {
        return Err(render_error(
            "Render dash must be an array of lengths",
            field.span,
        ));
    };
    let mut dash = Vec::with_capacity(values.len());
    for value in values {
        let value = render_length(value, field.span)?;
        if value < 0.0 {
            return Err(render_error(
                "Render dash elements must be non-negative",
                field.span,
            ));
        }
        dash.push(value);
    }
    if dash.len() % 2 == 1 {
        let length = dash.len();
        dash.extend_from_within(..length);
    }
    Ok(dash)
}

fn render_vec2_length(value: TypedValue, span: SourceSpan) -> Result<[f64; 2], Diagnostic> {
    let TypedValue::Vec2(x, y) = value else {
        return Err(render_error("expected vec2<length>", span));
    };
    Ok([render_length(*x, span)?, render_length(*y, span)?])
}

fn render_vec2_float(value: TypedValue, span: SourceSpan) -> Result<[f64; 2], Diagnostic> {
    let TypedValue::Vec2(x, y) = value else {
        return Err(render_error("expected vec2<float>", span));
    };
    let (TypedValue::Float(x), TypedValue::Float(y)) = (*x, *y) else {
        return Err(render_error("expected vec2<float>", span));
    };
    if !x.is_finite() || !y.is_finite() {
        return Err(render_error("Render float vector must be finite", span));
    }
    Ok([x, y])
}

fn render_image_sampling(
    value: TypedValue,
    span: SourceSpan,
) -> Result<CanonicalImageSampling, Diagnostic> {
    match render_string(value, span)?.as_str() {
        "nearest" => Ok(CanonicalImageSampling::Nearest),
        "linear" => Ok(CanonicalImageSampling::Bilinear),
        other => Err(render_error(
            format!("unsupported image sampling {other}"),
            span,
        )),
    }
}

fn render_pattern_repeat(
    value: TypedValue,
    span: SourceSpan,
) -> Result<CanonicalImageRepeat, Diagnostic> {
    match render_string(value, span)?.as_str() {
        "none" => Ok(CanonicalImageRepeat::None),
        "x" => Ok(CanonicalImageRepeat::X),
        "y" => Ok(CanonicalImageRepeat::Y),
        "both" => Ok(CanonicalImageRepeat::Both),
        other => Err(render_paint_error(
            format!("unsupported ImagePattern repeat {other}"),
            span,
        )),
    }
}

fn render_resource_sampling(
    resource: &CanonicalResource,
    span: SourceSpan,
) -> Result<CanonicalImageSampling, Diagnostic> {
    match resource.metadata().get("sampling") {
        Some(CanonicalValue::String(value)) if value == "nearest" => {
            Ok(CanonicalImageSampling::Nearest)
        }
        Some(CanonicalValue::String(value)) if value == "linear" => {
            Ok(CanonicalImageSampling::Bilinear)
        }
        _ => Err(render_resource_error(
            DiagnosticCode::RENDER_RESOURCE_DECODE_FAILED,
            format!(
                "image resource {} has invalid canonical sampling metadata",
                resource.id()
            ),
            span,
        )),
    }
}

fn render_string(value: TypedValue, span: SourceSpan) -> Result<String, Diagnostic> {
    match value {
        TypedValue::String(value) => Ok(value),
        other => Err(render_error(
            format!("expected string, found {}", other.ty()),
            span,
        )),
    }
}

fn render_int(value: TypedValue, span: SourceSpan) -> Result<i32, Diagnostic> {
    match value {
        TypedValue::Int(value) => {
            i32::try_from(value).map_err(|_| render_error("Render integer is outside i32", span))
        }
        other => Err(render_error(
            format!("expected int, found {}", other.ty()),
            span,
        )),
    }
}

fn render_float(value: TypedValue, span: SourceSpan) -> Result<f64, Diagnostic> {
    match value {
        TypedValue::Float(value) if value.is_finite() => Ok(value),
        other => Err(render_error(
            format!("expected finite float, found {}", other.ty()),
            span,
        )),
    }
}

fn render_bool(value: TypedValue, span: SourceSpan) -> Result<bool, Diagnostic> {
    match value {
        TypedValue::Bool(value) => Ok(value),
        other => Err(render_error(
            format!("expected bool, found {}", other.ty()),
            span,
        )),
    }
}

fn render_active_interval(
    items: &[RenderBodyItem],
    time_map: &ChartTimeMap,
    definitions: Option<&DefinitionsBlock>,
) -> Result<CanonicalActiveInterval, Diagnostic> {
    let Some(field) = render_body_field(items, "active") else {
        return Ok(CanonicalActiveInterval::unbounded());
    };
    let SchemaValue::Interval { start, end, .. } = &field.value else {
        return Err(render_error(
            "Render active must use a half-open interval",
            field.span,
        ));
    };
    let start = crate::elaborator::evaluate_metadata_expression(start, definitions)?;
    let end = crate::elaborator::evaluate_metadata_expression(end, definitions)?;
    let (start, end) = match (start, end) {
        (TypedValue::Beat(start), TypedValue::Beat(end)) => (
            time_map
                .chart_time(
                    CanonicalBeat::new(start.numerator(), start.denominator())
                        .map_err(|_| render_error("Render active beat is invalid", field.span))?,
                )
                .map_err(|error| render_error(error.to_string(), field.span))?
                .chart_time_seconds(),
            time_map
                .chart_time(
                    CanonicalBeat::new(end.numerator(), end.denominator())
                        .map_err(|_| render_error("Render active beat is invalid", field.span))?,
                )
                .map_err(|error| render_error(error.to_string(), field.span))?
                .chart_time_seconds(),
        ),
        (TypedValue::Time(start), TypedValue::Time(end)) => (start, end),
        _ => {
            return Err(render_error(
                "Render active endpoints must both be time or both be beat",
                field.span,
            ));
        }
    };
    CanonicalActiveInterval::bounded(start, end)
        .map_err(|error| render_error(format!("{error:?}"), field.span))
}

fn render_angle(value: TypedValue, span: SourceSpan) -> Result<f64, Diagnostic> {
    match value {
        TypedValue::Angle(value) if value.is_finite() => Ok(value),
        other => Err(render_error(
            format!("expected finite angle, found {}", other.ty()),
            span,
        )),
    }
}

fn validate_source_arc(
    radii: impl IntoIterator<Item = f64>,
    start_angle: Option<f64>,
    end_angle: Option<f64>,
    direction: CanonicalArcDirection,
    span: SourceSpan,
) -> Result<(), Diagnostic> {
    if radii.into_iter().any(|radius| radius < 0.0) {
        return Err(render_error("Path arc radii must be non-negative", span));
    }
    if let (Some(start), Some(end)) = (start_angle, end_angle) {
        let sweep = end - start;
        if (direction == CanonicalArcDirection::Clockwise && sweep > 0.0)
            || (direction == CanonicalArcDirection::CounterClockwise && sweep < 0.0)
        {
            return Err(render_error(
                "Path arc sweep does not match its direction",
                span,
            ));
        }
    }
    Ok(())
}

fn render_composite(
    value: TypedValue,
    span: SourceSpan,
) -> Result<CanonicalRenderComposite, Diagnostic> {
    let value = render_string(value, span)?;
    match value.as_str() {
        "sourceOver" => Ok(CanonicalRenderComposite::SourceOver),
        "copy" => Ok(CanonicalRenderComposite::Copy),
        "add" => Ok(CanonicalRenderComposite::Add),
        "multiply" => Ok(CanonicalRenderComposite::Multiply),
        "screen" => Ok(CanonicalRenderComposite::Screen),
        _ => Err(render_error(
            format!("unsupported Render composite {value}"),
            span,
        )),
    }
}

fn render_descriptor_value(
    value: TypedValue,
) -> Result<(CanonicalExpressionType, CanonicalExpressionValue), Diagnostic> {
    let canonical = match value {
        TypedValue::Bool(value) => CanonicalExpressionValue::Bool(value),
        TypedValue::Int(value) => CanonicalExpressionValue::Int(value),
        TypedValue::Float(value) => CanonicalExpressionValue::Float(value),
        TypedValue::Time(value) => CanonicalExpressionValue::Time(value),
        TypedValue::Beat(value) => CanonicalExpressionValue::ExactBeat(
            CanonicalBeat::new(value.numerator(), value.denominator())
                .map_err(|_| render_error("invalid Beat descriptor", SourceSpan::new(0, 0)))?,
        ),
        TypedValue::Length(value) => CanonicalExpressionValue::Length(value),
        TypedValue::Angle(value) => CanonicalExpressionValue::Angle(value),
        TypedValue::Color(value) => CanonicalExpressionValue::Color(value.to_linear()),
        TypedValue::Vec2(x, y) => {
            let (_, x) = render_descriptor_value(*x)?;
            let (_, y) = render_descriptor_value(*y)?;
            CanonicalExpressionValue::Vec2(Box::new(x), Box::new(y))
        }
        other => {
            return Err(render_error(
                format!("unsupported Render descriptor value {}", other.ty()),
                SourceSpan::new(0, 0),
            ));
        }
    };
    Ok((canonical.value_type(), canonical))
}

fn render_stable_id(
    registry: &mut StableIdRegistry,
    kind: EntityKind,
    textual: String,
    span: SourceSpan,
) -> Result<StableId, Diagnostic> {
    let textual = CanonicalTextualId::explicit(textual)
        .map_err(|error| render_error(error.to_string(), span))?;
    registry
        .insert(kind, textual)
        .map_err(|error| render_error(error.to_string(), span))
}

struct RenderFont<'a> {
    name: String,
    face: ttf_parser::Face<'a>,
    cmap: ttf_parser::cmap::Subtable<'a>,
}

struct ShapedTextRun {
    font_name: String,
    run_offset: f64,
    glyphs: Vec<CanonicalGlyphPlacement>,
}

fn big_endian_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn big_endian_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn raw_font_table<'a>(face: &ttf_parser::Face<'a>, tag: ttf_parser::Tag) -> Option<&'a [u8]> {
    let raw = face.raw_face();
    let record = raw
        .table_records
        .into_iter()
        .find(|record| record.tag == tag)?;
    let offset = usize::try_from(record.offset).ok()?;
    let length = usize::try_from(record.length).ok()?;
    raw.data.get(offset..offset.checked_add(length)?)
}

fn simple_ltr_cmap<'a>(
    face: &ttf_parser::Face<'a>,
    span: SourceSpan,
) -> Result<ttf_parser::cmap::Subtable<'a>, Diagnostic> {
    let bytes = raw_font_table(face, ttf_parser::Tag::from_bytes(b"cmap"))
        .ok_or_else(|| render_error("font resource has no readable cmap table", span))?;
    if big_endian_u16(bytes, 0) != Some(0) {
        return Err(render_error(
            "font resource has an unsupported cmap version",
            span,
        ));
    }
    let count = usize::from(
        big_endian_u16(bytes, 2)
            .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?,
    );
    let records_end = 4usize
        .checked_add(
            count
                .checked_mul(8)
                .ok_or_else(|| render_error("font resource cmap table is too large", span))?,
        )
        .ok_or_else(|| render_error("font resource cmap table is too large", span))?;
    if bytes.get(..records_end).is_none() {
        return Err(render_error(
            "font resource has a malformed cmap table",
            span,
        ));
    }
    let table = ttf_parser::cmap::Table::parse(bytes)
        .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?;
    let mut selected = None;
    for index in 0..count {
        let record = 4 + index * 8;
        let platform = big_endian_u16(bytes, record)
            .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?;
        let encoding = big_endian_u16(bytes, record + 2)
            .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?;
        let offset = big_endian_u32(bytes, record + 4)
            .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?;
        let subtable_offset = usize::try_from(offset)
            .map_err(|_| render_error("font resource cmap offset is too large", span))?;
        let format = big_endian_u16(bytes, subtable_offset)
            .ok_or_else(|| render_error("font resource has a malformed cmap table", span))?;
        let priority = match (platform, format, encoding) {
            (3, 12, 10) => 0,
            (3, 4, 1 | 10) => 1,
            _ => {
                return Err(render_error(
                    "font resource cmap must contain only platform 3 format 4/12 tables",
                    span,
                ));
            }
        };
        let index = u16::try_from(index)
            .map_err(|_| render_error("font resource cmap has too many subtables", span))?;
        table
            .subtables
            .get(index)
            .ok_or_else(|| render_error("font resource has a malformed cmap subtable", span))?;
        let candidate = (priority, encoding, offset, index);
        if selected.as_ref().is_none_or(|current| candidate < *current) {
            selected = Some(candidate);
        }
    }
    let (_, _, _, index) = selected
        .ok_or_else(|| render_error("font resource has no simple-ltr compatible cmap", span))?;
    table
        .subtables
        .get(index)
        .ok_or_else(|| render_error("font resource has a malformed cmap subtable", span))
}

fn parse_render_font<'a>(
    name: String,
    bytes: &'a [u8],
    span: SourceSpan,
) -> Result<RenderFont<'a>, Diagnostic> {
    if bytes.get(..4) != Some(&[0, 1, 0, 0][..]) || ttf_parser::fonts_in_collection(bytes).is_some()
    {
        return Err(render_error(
            "font resource must be a single-face TrueType sfnt",
            span,
        ));
    }
    let face = ttf_parser::Face::parse(bytes, 0)
        .map_err(|error| render_error(format!("font resource cannot be parsed: {error}"), span))?;
    let tables = face.tables();
    if tables.glyf.is_none()
        || raw_font_table(&face, ttf_parser::Tag::from_bytes(b"loca")).is_none()
        || tables.hmtx.is_none()
        || tables.cmap.is_none()
        || tables.cff.is_some()
    {
        return Err(render_error(
            "font resource must contain TrueType glyf/loca/hmtx/cmap tables",
            span,
        ));
    }
    for tag in [
        b"CFF ", b"CFF2", b"fvar", b"gvar", b"avar", b"HVAR", b"MVAR", b"VVAR", b"COLR", b"CPAL",
        b"CBDT", b"CBLC", b"EBDT", b"EBLC", b"sbix", b"SVG ",
    ] {
        if raw_font_table(&face, ttf_parser::Tag::from_bytes(tag)).is_some() {
            return Err(render_error(
                "font resource uses a TrueType feature outside truetype-glyf-1",
                span,
            ));
        }
    }
    let cmap = simple_ltr_cmap(&face, span)?;
    Ok(RenderFont { name, face, cmap })
}

fn forbidden_text_scalar(scalar: char) -> bool {
    scalar.is_control()
        || matches!(
            scalar,
            '\u{061c}'
                | '\u{200e}'
                | '\u{200f}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2066}'..='\u{2069}'
        )
}

struct RenderLowerer<'a> {
    resources: &'a BTreeMap<String, CanonicalResource>,
    resource_bundle: Option<&'a CanonicalResourceBundle>,
    time_map: &'a ChartTimeMap,
    definitions: Option<&'a DefinitionsBlock>,
    span: SourceSpan,
    descriptors: Vec<CanonicalPropertyDescriptor>,
    registry: StableIdRegistry,
    resource_ids: BTreeMap<String, StableId>,
    nodes: Vec<CanonicalRenderNode>,
    geometries: Vec<CanonicalRenderGeometry>,
    paths: Vec<CanonicalRenderPath>,
    paints: Vec<CanonicalRenderPaint>,
    strokes: Vec<CanonicalRenderStroke>,
    clips: Vec<CanonicalRenderClip>,
    glyph_runs: Vec<CanonicalGlyphRun>,
    descriptor_roots: Vec<(String, u64, usize)>,
}

impl<'a> RenderLowerer<'a> {
    fn new(
        resources: &'a BTreeMap<String, CanonicalResource>,
        resource_bundle: Option<&'a CanonicalResourceBundle>,
        time_map: &'a ChartTimeMap,
        definitions: Option<&'a DefinitionsBlock>,
        span: SourceSpan,
    ) -> Self {
        Self {
            resources,
            resource_bundle,
            time_map,
            definitions,
            span,
            descriptors: Vec::new(),
            registry: StableIdRegistry::new(),
            resource_ids: BTreeMap::new(),
            nodes: Vec::new(),
            geometries: Vec::new(),
            paths: Vec::new(),
            paints: Vec::new(),
            strokes: Vec::new(),
            clips: Vec::new(),
            glyph_runs: Vec::new(),
            descriptor_roots: Vec::new(),
        }
    }

    fn descriptor(&mut self, value: TypedValue) -> Result<usize, Diagnostic> {
        let (ty, canonical) = render_descriptor_value(value)?;
        let index = self.descriptors.len();
        self.descriptors.push(
            CanonicalPropertyDescriptor::new(
                ty.clone(),
                CanonicalDescriptorDomain::new(None, None, false).expect("unbounded domain"),
                CanonicalDescriptorKind::Constant(canonical.clone()),
            )
            .map_err(|error| render_error(error.to_string(), self.span))?,
        );
        Ok(index)
    }

    fn expression_descriptor(
        &mut self,
        expression: &SourceExpression,
        expected: CanonicalExpressionType,
    ) -> Result<(usize, Option<TypedValue>), Diagnostic> {
        match crate::elaborator::evaluate_metadata_expression(expression, self.definitions) {
            Ok(value) => {
                let (actual, _) = render_descriptor_value(value.clone())?;
                if actual != expected {
                    return Err(render_error(
                        format!("Render descriptor has type {actual:?}, expected {expected:?}"),
                        expression.span(),
                    ));
                }
                Ok((self.descriptor(value.clone())?, Some(value)))
            }
            Err(constant_error) => {
                let definitions = self.definitions;
                let dag = crate::expression::lower_runtime_expression_with_resolver(
                    expression,
                    |candidate| {
                        crate::elaborator::evaluate_metadata_expression(candidate, definitions).ok()
                    },
                )?;
                if dag.required_environment().is_empty() {
                    return Err(constant_error);
                }
                if dag.result_type() != &expected {
                    return Err(render_error(
                        format!(
                            "Render dynamic descriptor has type {:?}, expected {expected:?}",
                            dag.result_type()
                        ),
                        expression.span(),
                    ));
                }
                let index = self.descriptors.len();
                self.descriptors.push(
                    CanonicalPropertyDescriptor::new(
                        expected,
                        CanonicalDescriptorDomain::new(None, None, false)
                            .expect("unbounded domain"),
                        CanonicalDescriptorKind::Expression(dag),
                    )
                    .map_err(|error| render_error(error.to_string(), expression.span()))?,
                );
                Ok((index, None))
            }
        }
    }

    fn dynamic_opacity_descriptor(&mut self, field: &SchemaField) -> Result<usize, Diagnostic> {
        let SchemaValue::Expression(expression) = &field.value else {
            return Err(render_error(
                "Render field must be a compile-time expression",
                field.span,
            ));
        };
        let (descriptor, value) =
            self.expression_descriptor(expression, CanonicalExpressionType::Float)?;
        if let Some(value) = value {
            let opacity = render_float(value, field.span)?;
            if !(0.0..=1.0).contains(&opacity) {
                return Err(render_error(
                    "Render opacity must be within [0, 1]",
                    field.span,
                ));
            }
        }
        Ok(descriptor)
    }

    fn stable_id(
        &mut self,
        kind: EntityKind,
        textual: String,
        span: SourceSpan,
    ) -> Result<StableId, Diagnostic> {
        render_stable_id(&mut self.registry, kind, textual, span)
    }

    fn auxiliary_id(
        &mut self,
        kind: EntityKind,
        owner: &StableId,
        field: &str,
        ordinal: usize,
        span: SourceSpan,
    ) -> Result<StableId, Diagnostic> {
        self.stable_id(
            kind,
            format!(
                "owner/{:016x}/field/{field}/ordinal/{ordinal}",
                owner.value()
            ),
            span,
        )
    }

    fn add_descriptor_root(&mut self, path: &str, owner: u64, descriptor: usize) {
        self.descriptor_roots
            .push((path.to_owned(), owner, descriptor));
    }

    fn resource_id(&mut self, name: &str, span: SourceSpan) -> Result<StableId, Diagnostic> {
        if let Some(id) = self.resource_ids.get(name) {
            return Ok(id.clone());
        }
        let id = self.stable_id(EntityKind::Resource, name.to_owned(), span)?;
        self.resource_ids.insert(name.to_owned(), id.clone());
        Ok(id)
    }

    fn pattern_descriptor(
        &mut self,
        field: Option<&SchemaField>,
        default: usize,
        expected: CanonicalExpressionType,
    ) -> Result<usize, Diagnostic> {
        let Some(field) = field else {
            return Ok(default);
        };
        let SchemaValue::Expression(expression) = &field.value else {
            return Err(render_paint_error(
                "ImagePattern transform must be an expression",
                field.span,
            ));
        };
        self.expression_descriptor(expression, expected)
            .map(|(descriptor, _)| descriptor)
    }

    fn pattern_spec(
        &mut self,
        node: &crate::ast::RenderNodeDeclaration,
        defaults: CanonicalPatternTransform,
    ) -> Result<Option<RenderPatternSpec>, Diagnostic> {
        let position = render_unique_body_field(&node.items, "patternPosition")?;
        let origin = render_unique_body_field(&node.items, "patternOrigin")?;
        let rotation = render_unique_body_field(&node.items, "patternRotation")?;
        let scale = render_unique_body_field(&node.items, "patternScale")?;
        let repeat = render_unique_body_field(&node.items, "patternRepeat")?;
        let sampling = render_unique_body_field(&node.items, "patternSampling")?;

        let mut used = false;
        for name in ["fill", "stroke"] {
            if let Some(field) = render_body_field(&node.items, name) {
                used |= matches!(
                    render_paint_expression(field, self.definitions)?,
                    RenderPaintExpression::ImagePattern { .. }
                );
            }
        }
        if !used {
            if [position, origin, rotation, scale, repeat, sampling]
                .into_iter()
                .any(|field| field.is_some())
            {
                return Err(render_paint_error(
                    "pattern fields require an ImagePattern fill or stroke",
                    node.span,
                ));
            }
            return Ok(None);
        }

        let vector_length =
            || CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length));
        let vector_float =
            || CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Float));
        let transform = CanonicalPatternTransform {
            position: self.pattern_descriptor(position, defaults.position, vector_length())?,
            origin: self.pattern_descriptor(origin, defaults.origin, vector_length())?,
            rotation: self.pattern_descriptor(
                rotation,
                defaults.rotation,
                CanonicalExpressionType::Angle,
            )?,
            scale: self.pattern_descriptor(scale, defaults.scale, vector_float())?,
        };
        let repeat = match repeat {
            Some(field) => {
                render_pattern_repeat(render_value(field, self.definitions)?, field.span)?
            }
            None => CanonicalImageRepeat::Both,
        };
        let sampling = sampling
            .map(|field| render_image_sampling(render_value(field, self.definitions)?, field.span))
            .transpose()?;
        Ok(Some(RenderPatternSpec {
            transform,
            repeat,
            sampling,
        }))
    }

    fn add_glyph_run_roots(&mut self, run: &CanonicalGlyphRun, size: usize) {
        self.add_descriptor_root("render.glyphRun.size", run.id().value(), size);
    }

    fn add_node_roots(&mut self, id: &StableId, descriptors: [usize; 6]) {
        let owner = id.value();
        for (path, descriptor) in [
            "render.node.position",
            "render.node.origin",
            "render.node.rotation",
            "render.node.scale",
            "render.node.opacity",
            "render.node.visibility",
        ]
        .into_iter()
        .zip(descriptors)
        {
            self.add_descriptor_root(path, owner, descriptor);
        }
    }

    fn add_geometry_roots(&mut self, geometry: &CanonicalRenderGeometry) {
        let owner = geometry.id().value();
        match geometry.data() {
            CanonicalRenderGeometryData::Rect { origin, size }
            | CanonicalRenderGeometryData::RoundedRect { origin, size, .. } => {
                self.add_descriptor_root("render.geometry.origin", owner, *origin);
                self.add_descriptor_root("render.geometry.size", owner, *size);
                if let CanonicalRenderGeometryData::RoundedRect { radii, .. } = geometry.data() {
                    for (index, descriptor) in radii.iter().enumerate() {
                        self.add_descriptor_root(
                            &format!("render.geometry.radiiDescriptors[{index}]"),
                            owner,
                            *descriptor,
                        );
                    }
                }
            }
            CanonicalRenderGeometryData::Circle { center, radius } => {
                self.add_descriptor_root("render.geometry.center", owner, *center);
                self.add_descriptor_root("render.geometry.radius", owner, *radius);
            }
            CanonicalRenderGeometryData::Ellipse {
                center,
                radius_x,
                radius_y,
                rotation,
            } => {
                self.add_descriptor_root("render.geometry.center", owner, *center);
                self.add_descriptor_root("render.geometry.radiusX", owner, *radius_x);
                self.add_descriptor_root("render.geometry.radiusY", owner, *radius_y);
                self.add_descriptor_root("render.geometry.rotation", owner, *rotation);
            }
            CanonicalRenderGeometryData::Line { start, end } => {
                self.add_descriptor_root("render.geometry.start", owner, *start);
                self.add_descriptor_root("render.geometry.end", owner, *end);
            }
            CanonicalRenderGeometryData::Polyline { points }
            | CanonicalRenderGeometryData::Polygon { points } => {
                for (index, descriptor) in points.iter().enumerate() {
                    self.add_descriptor_root(
                        &format!("render.geometry.pointDescriptors[{index}]"),
                        owner,
                        *descriptor,
                    );
                }
            }
            CanonicalRenderGeometryData::Image {
                destination,
                source,
                ..
            } => {
                for (name, descriptor) in [
                    ("x", destination[0]),
                    ("y", destination[1]),
                    ("width", destination[2]),
                    ("height", destination[3]),
                ] {
                    self.add_descriptor_root(
                        &format!("render.geometry.destination.{name}"),
                        owner,
                        descriptor,
                    );
                }
                if let Some(source) = source {
                    for (name, descriptor) in [
                        ("x", source[0]),
                        ("y", source[1]),
                        ("width", source[2]),
                        ("height", source[3]),
                    ] {
                        self.add_descriptor_root(
                            &format!("render.geometry.source.{name}"),
                            owner,
                            descriptor,
                        );
                    }
                }
            }
            CanonicalRenderGeometryData::Text { origin, .. } => {
                self.add_descriptor_root("render.geometry.origin", owner, *origin);
            }
            _ => {}
        }
    }

    fn add_path_roots(&mut self, path: &CanonicalRenderPath) {
        let owner = path.id().value();
        for (index, command) in path.commands().iter().enumerate() {
            let add = |lowerer: &mut Self, name: &str, descriptor: usize| {
                lowerer.add_descriptor_root(
                    &format!("render.path.command[{index}].{name}"),
                    owner,
                    descriptor,
                );
            };
            match command {
                CanonicalPathCommand::MoveTo(point) | CanonicalPathCommand::LineTo(point) => {
                    add(self, "point", *point)
                }
                CanonicalPathCommand::QuadraticTo(control, end) => {
                    add(self, "control", *control);
                    add(self, "end", *end);
                }
                CanonicalPathCommand::CubicTo(control1, control2, end) => {
                    add(self, "control1", *control1);
                    add(self, "control2", *control2);
                    add(self, "end", *end);
                }
                CanonicalPathCommand::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    add(self, "center", *center);
                    add(self, "radius", *radius);
                    add(self, "startAngle", *start_angle);
                    add(self, "endAngle", *end_angle);
                }
                CanonicalPathCommand::EllipseArc {
                    center,
                    radius_x,
                    radius_y,
                    rotation,
                    start_angle,
                    end_angle,
                    ..
                } => {
                    add(self, "center", *center);
                    add(self, "radiusX", *radius_x);
                    add(self, "radiusY", *radius_y);
                    add(self, "rotation", *rotation);
                    add(self, "startAngle", *start_angle);
                    add(self, "endAngle", *end_angle);
                }
                CanonicalPathCommand::Close => {}
            }
        }
    }

    fn add_paint(
        &mut self,
        owner: &StableId,
        owner_field: &str,
        node: &crate::ast::RenderNodeDeclaration,
        field_name: &str,
        pattern: Option<&RenderPatternSpec>,
    ) -> Result<CanonicalRenderPaint, Diagnostic> {
        let field = render_body_field(&node.items, field_name).ok_or_else(|| {
            render_error(
                format!("drawable Render node requires {field_name}"),
                node.span,
            )
        })?;
        let id = self.auxiliary_id(EntityKind::RenderPaint, owner, owner_field, 0, field.span)?;
        let data = match render_paint_expression(field, self.definitions)? {
            RenderPaintExpression::Solid(color) => {
                let (color, _) =
                    self.expression_descriptor(color, CanonicalExpressionType::Color)?;
                CanonicalRenderPaintData::Solid { color }
            }
            RenderPaintExpression::LinearGradient {
                start,
                end,
                spread,
                stops,
            } => {
                let (start, _) = self.expression_descriptor(
                    start,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                )?;
                let (end, _) = self.expression_descriptor(
                    end,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                )?;
                let stops = stops
                    .into_iter()
                    .map(|(offset, color)| {
                        let (color, _) =
                            self.expression_descriptor(color, CanonicalExpressionType::Color)?;
                        CanonicalGradientStop::new(offset, color)
                            .map_err(|error| render_error(format!("{error:?}"), field.span))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                CanonicalRenderPaintData::LinearGradient {
                    start,
                    end,
                    spread,
                    stops,
                }
            }
            RenderPaintExpression::RadialGradient {
                start_center,
                start_radius,
                end_center,
                end_radius,
                spread,
                stops,
            } => {
                let (start_center, _) = self.expression_descriptor(
                    start_center,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                )?;
                let start_radius_span = start_radius.span();
                let (start_radius, start_radius_value) =
                    self.expression_descriptor(start_radius, CanonicalExpressionType::Length)?;
                if matches!(start_radius_value, Some(TypedValue::Length(value)) if value < 0.0) {
                    return Err(render_error(
                        "Render gradient radius must be non-negative",
                        start_radius_span,
                    ));
                }
                let (end_center, _) = self.expression_descriptor(
                    end_center,
                    CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
                )?;
                let end_radius_span = end_radius.span();
                let (end_radius, end_radius_value) =
                    self.expression_descriptor(end_radius, CanonicalExpressionType::Length)?;
                if matches!(end_radius_value, Some(TypedValue::Length(value)) if value < 0.0) {
                    return Err(render_error(
                        "Render gradient radius must be non-negative",
                        end_radius_span,
                    ));
                }
                let stops = stops
                    .into_iter()
                    .map(|(offset, color)| {
                        let (color, _) =
                            self.expression_descriptor(color, CanonicalExpressionType::Color)?;
                        CanonicalGradientStop::new(offset, color)
                            .map_err(|error| render_error(format!("{error:?}"), field.span))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                CanonicalRenderPaintData::RadialGradient {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                    spread,
                    stops,
                }
            }
            RenderPaintExpression::ImagePattern {
                resource,
                resource_span,
            } => {
                let metadata = self.resources.get(&resource).ok_or_else(|| {
                    render_resource_error(
                        DiagnosticCode::RENDER_RESOURCE_NOT_FOUND,
                        format!("ImagePattern references unknown resource {resource}"),
                        resource_span,
                    )
                })?;
                if !matches!(
                    metadata.kind(),
                    CanonicalResourceKind::Image | CanonicalResourceKind::Texture
                ) {
                    return Err(render_resource_error(
                        DiagnosticCode::RENDER_RESOURCE_TYPE_MISMATCH,
                        format!("ImagePattern resource {resource} is not image/texture"),
                        resource_span,
                    ));
                }
                let pattern = pattern.ok_or_else(|| {
                    render_paint_error("ImagePattern has no resolved pattern fields", field.span)
                })?;
                let sampling = match pattern.sampling {
                    Some(sampling) => sampling,
                    None => render_resource_sampling(metadata, resource_span)?,
                };
                CanonicalRenderPaintData::ImagePattern {
                    resource: self.resource_id(&resource, resource_span)?,
                    transform: pattern.transform,
                    repeat: pattern.repeat,
                    sampling,
                }
            }
        };
        CanonicalRenderPaint::new(id, data)
            .map_err(|error| render_error(format!("{error:?}"), node.span))
    }

    fn add_paint_roots(&mut self, paint_index: usize) {
        let owner = self.paints[paint_index].id().value();
        let roots = match self.paints[paint_index].data() {
            CanonicalRenderPaintData::Solid { color } => {
                vec![("render.paint.color".to_owned(), *color)]
            }
            CanonicalRenderPaintData::LinearGradient {
                start, end, stops, ..
            } => {
                let mut roots = vec![
                    ("render.paint.start".to_owned(), *start),
                    ("render.paint.end".to_owned(), *end),
                ];
                roots.extend(stops.iter().enumerate().map(|(index, stop)| {
                    (format!("render.paint.stop[{index}].color"), stop.color())
                }));
                roots
            }
            CanonicalRenderPaintData::RadialGradient {
                start_center,
                start_radius,
                end_center,
                end_radius,
                stops,
                ..
            } => {
                let mut roots = vec![
                    ("render.paint.startCenter".to_owned(), *start_center),
                    ("render.paint.startRadius".to_owned(), *start_radius),
                    ("render.paint.endCenter".to_owned(), *end_center),
                    ("render.paint.endRadius".to_owned(), *end_radius),
                ];
                roots.extend(stops.iter().enumerate().map(|(index, stop)| {
                    (format!("render.paint.stop[{index}].color"), stop.color())
                }));
                roots
            }
            CanonicalRenderPaintData::ImagePattern { transform, .. } => vec![
                ("render.paint.position".to_owned(), transform.position),
                ("render.paint.origin".to_owned(), transform.origin),
                ("render.paint.rotation".to_owned(), transform.rotation),
                ("render.paint.scale".to_owned(), transform.scale),
            ],
        };
        for (path, descriptor) in roots {
            self.add_descriptor_root(&path, owner, descriptor);
        }
    }

    fn add_stroke_roots(&mut self, stroke_index: usize) {
        let (owner, width, dash_offset) = {
            let stroke = &self.strokes[stroke_index];
            (stroke.id().value(), stroke.width(), stroke.dash_offset())
        };
        self.add_descriptor_root("render.stroke.width", owner, width);
        self.add_descriptor_root("render.stroke.dashOffset", owner, dash_offset);
    }
}

impl<'a> RenderLowerer<'a> {
    fn render_stroke(
        &mut self,
        node_id: &StableId,
        node: &crate::ast::RenderNodeDeclaration,
        pattern: Option<&RenderPatternSpec>,
    ) -> Result<RenderStrokeSpec, Diagnostic> {
        let stroke_field = render_body_field(&node.items, "stroke")
            .ok_or_else(|| render_error("drawable Render node requires stroke", node.span))?;
        let id = self.auxiliary_id(
            EntityKind::RenderStroke,
            node_id,
            "strokeRef",
            0,
            stroke_field.span,
        )?;
        let paint = self.add_paint(&id, "paintRef", node, "stroke", pattern)?;
        let width_field = render_body_field(&node.items, "width")
            .ok_or_else(|| render_error("Render stroke requires width", node.span))?;
        let SchemaValue::Expression(width_expression) = &width_field.value else {
            return Err(render_error(
                "Render stroke width must be an expression",
                width_field.span,
            ));
        };
        let (width, static_width) =
            self.expression_descriptor(width_expression, CanonicalExpressionType::Length)?;
        if matches!(static_width, Some(TypedValue::Length(value)) if value < 0.0) {
            return Err(render_error(
                "Render stroke width must be non-negative",
                width_field.span,
            ));
        }
        let cap_field = render_body_field(&node.items, "cap")
            .ok_or_else(|| render_error("Render stroke requires cap", node.span))?;
        let join_field = render_body_field(&node.items, "join")
            .ok_or_else(|| render_error("Render stroke requires join", node.span))?;
        let miter_field = render_body_field(&node.items, "miterLimit")
            .ok_or_else(|| render_error("Render stroke requires miterLimit", node.span))?;
        let dash_field = render_body_field(&node.items, "dash")
            .ok_or_else(|| render_error("Render stroke requires dash", node.span))?;
        let dash_offset_field = render_body_field(&node.items, "dashOffset")
            .ok_or_else(|| render_error("Render stroke requires dashOffset", node.span))?;
        let miter_limit = render_float(
            render_value(miter_field, self.definitions)?,
            miter_field.span,
        )?;
        if miter_limit < 1.0 {
            return Err(render_error(
                "Render stroke miterLimit must be at least 1",
                miter_field.span,
            ));
        }
        let SchemaValue::Expression(dash_offset_expression) = &dash_offset_field.value else {
            return Err(render_error(
                "Render stroke dashOffset must be an expression",
                dash_offset_field.span,
            ));
        };
        let (dash_offset, _) =
            self.expression_descriptor(dash_offset_expression, CanonicalExpressionType::Length)?;
        Ok(RenderStrokeSpec {
            id,
            paint,
            width,
            cap: render_stroke_cap(render_value(cap_field, self.definitions)?, cap_field.span)?,
            join: render_stroke_join(render_value(join_field, self.definitions)?, join_field.span)?,
            miter_limit,
            dash_offset,
            dash: render_dash(dash_field, self.definitions)?,
        })
    }

    fn path_vec2_descriptor(&mut self, expression: &SourceExpression) -> Result<usize, Diagnostic> {
        self.expression_descriptor(
            expression,
            CanonicalExpressionType::Vec2(Box::new(CanonicalExpressionType::Length)),
        )
        .map(|(descriptor, _)| descriptor)
    }

    fn path_length_descriptor(
        &mut self,
        expression: &SourceExpression,
    ) -> Result<(usize, Option<f64>), Diagnostic> {
        let (descriptor, value) =
            self.expression_descriptor(expression, CanonicalExpressionType::Length)?;
        Ok((
            descriptor,
            value.map(|value| match value {
                TypedValue::Length(value) => value,
                _ => unreachable!("the descriptor type was checked"),
            }),
        ))
    }

    fn path_angle_descriptor(
        &mut self,
        expression: &SourceExpression,
    ) -> Result<(usize, Option<f64>), Diagnostic> {
        let (descriptor, value) =
            self.expression_descriptor(expression, CanonicalExpressionType::Angle)?;
        Ok((
            descriptor,
            value.map(|value| match value {
                TypedValue::Angle(value) => value,
                _ => unreachable!("the descriptor type was checked"),
            }),
        ))
    }

    fn path_direction(
        &self,
        expression: &SourceExpression,
    ) -> Result<CanonicalArcDirection, Diagnostic> {
        let value = crate::elaborator::evaluate_metadata_expression(expression, self.definitions)?;
        match render_string(value, expression.span())?.as_str() {
            "clockwise" => Ok(CanonicalArcDirection::Clockwise),
            "counterClockwise" => Ok(CanonicalArcDirection::CounterClockwise),
            other => Err(render_error(
                format!("unsupported Path arc direction {other}"),
                expression.span(),
            )),
        }
    }

    fn render_fill_rule(
        &self,
        node: &crate::ast::RenderNodeDeclaration,
        scope: Option<&str>,
    ) -> Result<CanonicalRenderFillRule, Diagnostic> {
        let path = render_scoped_body_path(scope, "fillRule");
        let field = render_scoped_body_field(&node.items, scope, "fillRule")?.ok_or_else(|| {
            if scope.is_some() {
                render_clip_error(format!("ClipGroup requires {path}"), node.span)
            } else {
                render_error("Path requires fillRule", node.span)
            }
        })?;
        let SchemaValue::Expression(expression) = &field.value else {
            let message = format!("{path} must be a compile-time expression");
            return Err(if scope.is_some() {
                render_clip_error(message, field.span)
            } else {
                render_error(message, field.span)
            });
        };
        let value = render_string(
            crate::elaborator::evaluate_metadata_expression(expression, self.definitions).map_err(
                |error| {
                    if scope.is_some() {
                        render_clip_error(error.message().to_owned(), field.span)
                    } else {
                        error
                    }
                },
            )?,
            field.span,
        )
        .map_err(|error| {
            if scope.is_some() {
                render_clip_error(error.message().to_owned(), field.span)
            } else {
                error
            }
        })?;
        match value.as_str() {
            "nonzero" => Ok(CanonicalRenderFillRule::NonZero),
            "evenodd" => Ok(CanonicalRenderFillRule::EvenOdd),
            other => Err(if scope.is_some() {
                render_clip_error(format!("unsupported Clip fillRule {other}"), field.span)
            } else {
                render_error(format!("unsupported Path fillRule {other}"), field.span)
            }),
        }
    }

    fn lower_path(
        &mut self,
        node: &crate::ast::RenderNodeDeclaration,
        geometry_id: &StableId,
        scope: Option<&str>,
        fill_rule: CanonicalRenderFillRule,
    ) -> Result<CanonicalRenderPath, Diagnostic> {
        let commands_path = render_scoped_body_path(scope, "commands");
        let commands_field = render_scoped_body_field(&node.items, scope, "commands")?
            .ok_or_else(|| render_error(format!("Path requires {commands_path}"), node.span))?;
        let SchemaValue::Expression(SourceExpression::Array { elements, .. }) =
            &commands_field.value
        else {
            return Err(render_error(
                format!("{commands_path} must be a compile-time array"),
                commands_field.span,
            ));
        };

        let mut commands = Vec::with_capacity(elements.len());
        let mut open = false;
        let mut closed = false;
        let mut has_drawing = false;
        for expression in elements {
            let SourceExpression::Call {
                callee, arguments, ..
            } = expression
            else {
                return Err(render_error(
                    "Path command must be a command call",
                    expression.span(),
                ));
            };
            let SourceExpression::Name { name, .. } = callee.as_ref() else {
                return Err(render_error(
                    "Path command must use a fixed command name",
                    expression.span(),
                ));
            };
            let command = match name.as_str() {
                "moveTo" | "lineTo" => {
                    let [point] = arguments.as_slice() else {
                        return Err(render_error(
                            format!("{name} requires one point"),
                            expression.span(),
                        ));
                    };
                    let point = self.path_vec2_descriptor(point)?;
                    if name == "moveTo" {
                        CanonicalPathCommand::MoveTo(point)
                    } else {
                        CanonicalPathCommand::LineTo(point)
                    }
                }
                "quadraticTo" => {
                    let [control, end] = arguments.as_slice() else {
                        return Err(render_error(
                            "quadraticTo requires control and end points",
                            expression.span(),
                        ));
                    };
                    CanonicalPathCommand::QuadraticTo(
                        self.path_vec2_descriptor(control)?,
                        self.path_vec2_descriptor(end)?,
                    )
                }
                "cubicTo" => {
                    let [control1, control2, end] = arguments.as_slice() else {
                        return Err(render_error(
                            "cubicTo requires two control points and an end point",
                            expression.span(),
                        ));
                    };
                    CanonicalPathCommand::CubicTo(
                        self.path_vec2_descriptor(control1)?,
                        self.path_vec2_descriptor(control2)?,
                        self.path_vec2_descriptor(end)?,
                    )
                }
                "arc" => {
                    let [center, radius, start_angle, end_angle, direction] = arguments.as_slice()
                    else {
                        return Err(render_error(
                            "arc requires center, radius, startAngle, endAngle, and direction",
                            expression.span(),
                        ));
                    };
                    let center = self.path_vec2_descriptor(center)?;
                    let (radius, radius_value) = self.path_length_descriptor(radius)?;
                    let (start_angle, start_value) = self.path_angle_descriptor(start_angle)?;
                    let (end_angle, end_value) = self.path_angle_descriptor(end_angle)?;
                    let direction = self.path_direction(direction)?;
                    validate_source_arc(
                        radius_value.into_iter(),
                        start_value,
                        end_value,
                        direction,
                        expression.span(),
                    )?;
                    CanonicalPathCommand::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        direction,
                    }
                }
                "ellipseArc" => {
                    let [
                        center,
                        radius_x,
                        radius_y,
                        rotation,
                        start_angle,
                        end_angle,
                        direction,
                    ] = arguments.as_slice()
                    else {
                        return Err(render_error(
                            "ellipseArc requires center, radiusX, radiusY, rotation, startAngle, endAngle, and direction",
                            expression.span(),
                        ));
                    };
                    let center = self.path_vec2_descriptor(center)?;
                    let (radius_x, radius_x_value) = self.path_length_descriptor(radius_x)?;
                    let (radius_y, radius_y_value) = self.path_length_descriptor(radius_y)?;
                    let (rotation, _) = self.path_angle_descriptor(rotation)?;
                    let (start_angle, start_value) = self.path_angle_descriptor(start_angle)?;
                    let (end_angle, end_value) = self.path_angle_descriptor(end_angle)?;
                    let direction = self.path_direction(direction)?;
                    validate_source_arc(
                        [radius_x_value, radius_y_value].into_iter().flatten(),
                        start_value,
                        end_value,
                        direction,
                        expression.span(),
                    )?;
                    CanonicalPathCommand::EllipseArc {
                        center,
                        radius_x,
                        radius_y,
                        rotation,
                        start_angle,
                        end_angle,
                        direction,
                    }
                }
                "close" => {
                    if !arguments.is_empty() {
                        return Err(render_error("close takes no arguments", expression.span()));
                    }
                    CanonicalPathCommand::Close
                }
                other => {
                    return Err(render_error(
                        format!("unsupported Path command {other}"),
                        expression.span(),
                    ));
                }
            };
            match &command {
                CanonicalPathCommand::MoveTo(_) => {
                    open = true;
                    closed = false;
                    has_drawing = false;
                }
                CanonicalPathCommand::Close if !open || closed || !has_drawing => {
                    return Err(render_error(
                        "Path Close is not valid here",
                        expression.span(),
                    ));
                }
                CanonicalPathCommand::Close => closed = true,
                _ if !open => {
                    return Err(render_error(
                        "Path drawing command requires an earlier moveTo",
                        expression.span(),
                    ));
                }
                _ => {
                    closed = false;
                    has_drawing = true;
                }
            }
            commands.push(command);
        }

        let id = self.auxiliary_id(
            EntityKind::RenderPath,
            geometry_id,
            "pathRef",
            0,
            commands_field.span,
        )?;
        CanonicalRenderPath::new(id, fill_rule, commands)
            .map_err(|error| render_error(format!("{error:?}"), commands_field.span))
    }

    fn text_fonts(
        &self,
        names: &[String],
        span: SourceSpan,
    ) -> Result<Vec<RenderFont<'_>>, Diagnostic> {
        let bundle = self.resource_bundle.ok_or_else(|| {
            render_error("Text lowering requires a resolved resource bundle", span)
        })?;
        names
            .iter()
            .map(|name| {
                let resource = self.resources.get(name).ok_or_else(|| {
                    render_error(format!("Text references unknown resource {name}"), span)
                })?;
                if !matches!(resource.kind(), CanonicalResourceKind::Font) {
                    return Err(render_error(
                        format!("Text resource {name} is not a font"),
                        span,
                    ));
                }
                if resource.media_type() != "font/ttf" {
                    return Err(render_error(
                        format!("Text font resource {name} is not font/ttf"),
                        span,
                    ));
                }
                let bundled = bundle.get(name).ok_or_else(|| {
                    render_error(format!("Text resource {name} has no resolved bytes"), span)
                })?;
                parse_render_font(name.clone(), bundled.bytes(), span)
            })
            .collect()
    }

    fn lower_text(
        &mut self,
        node: &crate::ast::RenderNodeDeclaration,
        geometry_id: &StableId,
    ) -> Result<Vec<usize>, Diagnostic> {
        let content_field = render_body_field(&node.items, "content")
            .ok_or_else(|| render_error("Text requires content", node.span))?;
        let content = render_string(
            render_value(content_field, self.definitions)?,
            content_field.span,
        )?;
        let font_field = render_body_field(&node.items, "font")
            .ok_or_else(|| render_error("Text requires font", node.span))?;
        let primary_font = match render_value(font_field, self.definitions)? {
            TypedValue::Line(name) => name,
            other => {
                return Err(render_error(
                    format!("Text font must be a reference, found {}", other.ty()),
                    font_field.span,
                ));
            }
        };
        let fallback_fonts = match render_body_field(&node.items, "fallbackFonts") {
            Some(field) => render_font_references(field, self.definitions)?,
            None => Vec::new(),
        };
        let face_index =
            render_body_value_or(&node.items, "faceIndex", 0, self.definitions, |value| {
                render_int(value, node.span)
            })?;
        if face_index != 0 {
            return Err(render_error(
                "Text faceIndex is fixed to 0 in simple-ltr-1",
                render_body_field(&node.items, "faceIndex").map_or(node.span, |field| field.span),
            ));
        }
        for (name, expected) in [
            ("shapingProfile", "simple-ltr-1"),
            ("language", "und"),
            ("script", "Latn"),
            ("direction", "ltr"),
        ] {
            if let Some(field) = render_body_field(&node.items, name) {
                let value = render_string(render_value(field, self.definitions)?, field.span)?;
                if value != expected {
                    return Err(render_error(
                        format!("Text {name} is fixed to {expected} in simple-ltr-1"),
                        field.span,
                    ));
                }
            }
        }
        if let Some(field) = render_body_field(&node.items, "features") {
            render_empty_array(field, "Text features", self.definitions)?;
        }
        let size_field = render_body_field(&node.items, "size")
            .ok_or_else(|| render_error("Text requires size", node.span))?;
        let size = render_length(render_value(size_field, self.definitions)?, size_field.span)?;
        if size <= 0.0 {
            return Err(render_error(
                "Text size must be greater than zero",
                size_field.span,
            ));
        }

        let mut font_names = Vec::with_capacity(1 + fallback_fonts.len());
        font_names.push(primary_font);
        font_names.extend(fallback_fonts);
        let fonts = self.text_fonts(&font_names, font_field.span)?;
        let mut shaped_runs = Vec::new();
        let mut current_font = None;
        let mut run_offset = 0.0;
        for scalar in content.chars() {
            if forbidden_text_scalar(scalar) {
                return Err(render_error(
                    format!(
                        "Text content contains forbidden scalar U+{:04X}",
                        u32::from(scalar)
                    ),
                    content_field.span,
                ));
            }
            let mut selected = None;
            for (index, font) in fonts.iter().enumerate() {
                let Some(glyph) = font.cmap.glyph_index(u32::from(scalar)) else {
                    continue;
                };
                if glyph.0 == 0 {
                    continue;
                }
                if u32::from(glyph.0) >= u32::from(font.face.number_of_glyphs()) {
                    return Err(render_error(
                        format!("font resource {} maps to an invalid glyph", font.name),
                        content_field.span,
                    ));
                }
                let advance = font.face.glyph_hor_advance(glyph).ok_or_else(|| {
                    render_error(
                        format!("font resource {} has no glyph advance", font.name),
                        content_field.span,
                    )
                })?;
                selected = Some((index, glyph, advance));
                break;
            }
            let Some((font_index, glyph, advance)) = selected else {
                return Err(render_error(
                    format!(
                        "Text content scalar U+{:04X} has no glyph in any declared font",
                        u32::from(scalar)
                    ),
                    content_field.span,
                ));
            };
            if current_font != Some(font_index) {
                shaped_runs.push(ShapedTextRun {
                    font_name: fonts[font_index].name.clone(),
                    run_offset,
                    glyphs: Vec::new(),
                });
                current_font = Some(font_index);
            }
            let x_advance = f64::from(advance) / f64::from(fonts[font_index].face.units_per_em());
            shaped_runs
                .last_mut()
                .expect("a new or current Text glyph run")
                .glyphs
                .push(CanonicalGlyphPlacement {
                    glyph_id: u32::from(glyph.0),
                    x_advance,
                    y_advance: 0.0,
                    x_offset: 0.0,
                    y_offset: 0.0,
                });
            run_offset += x_advance;
            if !run_offset.is_finite() {
                return Err(render_error(
                    "Text run offset is not finite",
                    content_field.span,
                ));
            }
        }
        if shaped_runs.is_empty() {
            shaped_runs.push(ShapedTextRun {
                font_name: fonts[0].name.clone(),
                run_offset: 0.0,
                glyphs: Vec::new(),
            });
        }
        drop(fonts);

        let size_descriptor = self.descriptor(TypedValue::Length(size))?;
        let mut glyph_run_refs = Vec::with_capacity(shaped_runs.len());
        for (index, run) in shaped_runs.into_iter().enumerate() {
            let font_id = self.resource_id(&run.font_name, font_field.span)?;
            let run_id = self.auxiliary_id(
                EntityKind::RenderGlyphRun,
                geometry_id,
                "glyphRunRefs",
                index,
                node.span,
            )?;
            let glyph_run = CanonicalGlyphRun::new(
                run_id,
                font_id,
                u32::try_from(face_index).expect("faceIndex is zero"),
                size_descriptor,
                [run.run_offset, 0.0],
                run.glyphs,
            )
            .map_err(|error| render_error(format!("{error:?}"), node.span))?;
            let glyph_run_index = self.glyph_runs.len();
            self.add_glyph_run_roots(&glyph_run, size_descriptor);
            self.glyph_runs.push(glyph_run);
            glyph_run_refs.push(glyph_run_index);
        }
        Ok(glyph_run_refs)
    }

    fn lower_fill_geometry(
        &mut self,
        node: &crate::ast::RenderNodeDeclaration,
        kind: CanonicalRenderNodeKind,
        geometry_id: &StableId,
        scope: Option<&str>,
        origin: Option<usize>,
        rotation: Option<usize>,
        path_fill_rule: Option<CanonicalRenderFillRule>,
    ) -> Result<CanonicalRenderGeometryData, Diagnostic> {
        let zero_length_vec = || {
            TypedValue::vec2(TypedValue::Length(0.0), TypedValue::Length(0.0))
                .expect("homogeneous length vector")
        };
        match kind {
            CanonicalRenderNodeKind::Rect => {
                let path = render_scoped_body_path(scope, "size");
                let field = render_scoped_body_field(&node.items, scope, "size")?
                    .ok_or_else(|| render_error(format!("Rect requires {path}"), node.span))?;
                let size = render_vec2_length(render_value(field, self.definitions)?, field.span)?;
                if size.iter().any(|value| *value < 0.0) {
                    return Err(render_error("Rect size must be non-negative", field.span));
                }
                Ok(CanonicalRenderGeometryData::Rect {
                    origin: origin.expect("Rect lowering provides an origin"),
                    size: self.descriptor(
                        TypedValue::vec2(TypedValue::Length(size[0]), TypedValue::Length(size[1]))
                            .expect("homogeneous length vector"),
                    )?,
                })
            }
            CanonicalRenderNodeKind::RoundedRect => {
                let size_path = render_scoped_body_path(scope, "size");
                let size_field =
                    render_scoped_body_field(&node.items, scope, "size")?.ok_or_else(|| {
                        render_error(format!("RoundedRect requires {size_path}"), node.span)
                    })?;
                let size = render_vec2_length(
                    render_value(size_field, self.definitions)?,
                    size_field.span,
                )?;
                if size.iter().any(|value| *value < 0.0) {
                    return Err(render_error(
                        "RoundedRect size must be non-negative",
                        size_field.span,
                    ));
                }
                let radius_path = render_scoped_body_path(scope, "radius");
                let radius_field = render_scoped_body_field(&node.items, scope, "radius")?
                    .ok_or_else(|| {
                        render_error(format!("RoundedRect requires {radius_path}"), node.span)
                    })?;
                let radius = render_length(
                    render_value(radius_field, self.definitions)?,
                    radius_field.span,
                )?;
                if radius < 0.0 {
                    return Err(render_error(
                        "RoundedRect radius must be non-negative",
                        radius_field.span,
                    ));
                }
                let size = self.descriptor(
                    TypedValue::vec2(TypedValue::Length(size[0]), TypedValue::Length(size[1]))
                        .expect("homogeneous length vector"),
                )?;
                let radius = self.descriptor(TypedValue::Length(radius))?;
                Ok(CanonicalRenderGeometryData::RoundedRect {
                    origin: origin.expect("RoundedRect lowering provides an origin"),
                    size,
                    radii: [radius; 4],
                })
            }
            CanonicalRenderNodeKind::Circle => {
                let center = match render_scoped_body_field(&node.items, scope, "center")? {
                    Some(field) => render_value(field, self.definitions)?,
                    None => zero_length_vec(),
                };
                let radius_path = render_scoped_body_path(scope, "radius");
                let radius_field = render_scoped_body_field(&node.items, scope, "radius")?
                    .ok_or_else(|| {
                        render_error(format!("Circle requires {radius_path}"), node.span)
                    })?;
                let radius = render_length(
                    render_value(radius_field, self.definitions)?,
                    radius_field.span,
                )?;
                if radius < 0.0 {
                    return Err(render_error(
                        "Circle radius must be non-negative",
                        radius_field.span,
                    ));
                }
                Ok(CanonicalRenderGeometryData::Circle {
                    center: self.descriptor(center)?,
                    radius: self.descriptor(TypedValue::Length(radius))?,
                })
            }
            CanonicalRenderNodeKind::Ellipse => {
                let center = match render_scoped_body_field(&node.items, scope, "center")? {
                    Some(field) => render_value(field, self.definitions)?,
                    None => zero_length_vec(),
                };
                let radius_x_path = render_scoped_body_path(scope, "radiusX");
                let radius_y_path = render_scoped_body_path(scope, "radiusY");
                let radius_x_field = render_scoped_body_field(&node.items, scope, "radiusX")?
                    .ok_or_else(|| {
                        render_error(format!("Ellipse requires {radius_x_path}"), node.span)
                    })?;
                let radius_y_field = render_scoped_body_field(&node.items, scope, "radiusY")?
                    .ok_or_else(|| {
                        render_error(format!("Ellipse requires {radius_y_path}"), node.span)
                    })?;
                let radius_x = render_length(
                    render_value(radius_x_field, self.definitions)?,
                    radius_x_field.span,
                )?;
                let radius_y = render_length(
                    render_value(radius_y_field, self.definitions)?,
                    radius_y_field.span,
                )?;
                if radius_x < 0.0 || radius_y < 0.0 {
                    return Err(render_error(
                        "Ellipse radii must be non-negative",
                        node.span,
                    ));
                }
                Ok(CanonicalRenderGeometryData::Ellipse {
                    center: self.descriptor(center)?,
                    radius_x: self.descriptor(TypedValue::Length(radius_x))?,
                    radius_y: self.descriptor(TypedValue::Length(radius_y))?,
                    rotation: rotation.expect("Ellipse lowering provides a rotation"),
                })
            }
            CanonicalRenderNodeKind::Polygon => {
                let points_path = render_scoped_body_path(scope, "points");
                let points_field = render_scoped_body_field(&node.items, scope, "points")?
                    .ok_or_else(|| {
                        render_error(format!("Polygon requires {points_path}"), node.span)
                    })?;
                let points = render_value(points_field, self.definitions)?;
                let TypedValue::Array { values, .. } = points else {
                    return Err(render_error(
                        "Polygon points must be an array of vec2<length>",
                        points_field.span,
                    ));
                };
                let points = values
                    .into_iter()
                    .map(|value| {
                        let [x, y] = render_vec2_length(value, points_field.span)?;
                        self.descriptor(
                            TypedValue::vec2(TypedValue::Length(x), TypedValue::Length(y))
                                .expect("homogeneous length vector"),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(CanonicalRenderGeometryData::Polygon { points })
            }
            CanonicalRenderNodeKind::Path => {
                let path = self.lower_path(
                    node,
                    geometry_id,
                    scope,
                    path_fill_rule.expect("Path lowering provides a fill rule"),
                )?;
                let path_index = self.paths.len();
                self.add_path_roots(&path);
                self.paths.push(path);
                Ok(CanonicalRenderGeometryData::Path { path: path_index })
            }
            _ => unreachable!("only fillable Clip geometry kinds reach this helper"),
        }
    }

    fn lower_clip(
        &mut self,
        node_id: &StableId,
        node: &crate::ast::RenderNodeDeclaration,
    ) -> Result<(usize, CanonicalRenderGeometry, CanonicalRenderClip), Diagnostic> {
        for forbidden in [
            "fill",
            "stroke",
            "patternPosition",
            "patternOrigin",
            "patternRotation",
            "patternScale",
            "patternRepeat",
            "patternSampling",
        ] {
            if let Some(field) = render_body_field(&node.items, forbidden) {
                return Err(Diagnostic::new(
                    DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                    DiagnosticStage::Canonical,
                    format!("ClipGroup does not allow {forbidden}"),
                    field.span,
                ));
            }
        }

        let kind_field = render_unique_body_field(&node.items, "clip.kind")?
            .ok_or_else(|| render_clip_error("ClipGroup requires clip.kind", node.span))?;
        let kind_value = render_string(
            render_value(kind_field, self.definitions)
                .map_err(|error| render_clip_error(error.message().to_owned(), kind_field.span))?,
            kind_field.span,
        )
        .map_err(|error| render_clip_error(error.message().to_owned(), kind_field.span))?;
        let kind = match kind_value.as_str() {
            "rect" => CanonicalRenderNodeKind::Rect,
            "roundedRect" => CanonicalRenderNodeKind::RoundedRect,
            "circle" => CanonicalRenderNodeKind::Circle,
            "ellipse" => CanonicalRenderNodeKind::Ellipse,
            "polygon" => CanonicalRenderNodeKind::Polygon,
            "path" => CanonicalRenderNodeKind::Path,
            other => {
                return Err(render_clip_error(
                    format!("unsupported Clip geometry kind {other}"),
                    kind_field.span,
                ));
            }
        };

        for item in &node.items {
            let RenderBodyItem::Field(field) = item else {
                continue;
            };
            let segments = &field.path.segments;
            if segments.first().is_none_or(|segment| segment != "clip") {
                continue;
            }
            let allowed = match segments.get(1).map(String::as_str) {
                Some("kind" | "fillRule") if segments.len() == 2 => true,
                Some(name) if segments.len() == 2 => match kind {
                    CanonicalRenderNodeKind::Rect => matches!(name, "origin" | "size"),
                    CanonicalRenderNodeKind::RoundedRect => {
                        matches!(name, "origin" | "size" | "radius")
                    }
                    CanonicalRenderNodeKind::Circle => matches!(name, "center" | "radius"),
                    CanonicalRenderNodeKind::Ellipse => {
                        matches!(name, "center" | "radiusX" | "radiusY" | "rotation")
                    }
                    CanonicalRenderNodeKind::Polygon => name == "points",
                    CanonicalRenderNodeKind::Path => name == "commands",
                    _ => false,
                },
                _ => false,
            };
            if !allowed {
                return Err(Diagnostic::new(
                    DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                    DiagnosticStage::Canonical,
                    format!("unknown Clip field {}", segments.join(".")),
                    field.span,
                ));
            }
        }

        let fill_rule = self.render_fill_rule(node, Some("clip"))?;
        let clip_id = self.auxiliary_id(
            EntityKind::RenderClip,
            node_id,
            "clipRef",
            0,
            kind_field.span,
        )?;
        let geometry_id = self.auxiliary_id(
            EntityKind::RenderGeometry,
            &clip_id,
            "geometryRef",
            0,
            kind_field.span,
        )?;
        let origin = if matches!(
            kind,
            CanonicalRenderNodeKind::Rect | CanonicalRenderNodeKind::RoundedRect
        ) {
            let value = match render_scoped_body_field(&node.items, Some("clip"), "origin")? {
                Some(field) => render_value(field, self.definitions)?,
                None => TypedValue::vec2(TypedValue::Length(0.0), TypedValue::Length(0.0))
                    .expect("homogeneous length vector"),
            };
            Some(self.descriptor(value)?)
        } else {
            None
        };
        let rotation = if kind == CanonicalRenderNodeKind::Ellipse {
            let value = match render_scoped_body_field(&node.items, Some("clip"), "rotation")? {
                Some(field) => render_angle(render_value(field, self.definitions)?, field.span)?,
                None => 0.0,
            };
            Some(self.descriptor(TypedValue::Angle(value))?)
        } else {
            None
        };
        let geometry = CanonicalRenderGeometry::new(
            geometry_id.clone(),
            self.lower_fill_geometry(
                node,
                kind,
                &geometry_id,
                Some("clip"),
                origin,
                rotation,
                (kind == CanonicalRenderNodeKind::Path).then_some(fill_rule),
            )?,
        )
        .map_err(|error| render_error(format!("{error:?}"), node.span))?;
        let geometry_index = self.geometries.len();
        let clip_index = self.clips.len();
        let clip = CanonicalRenderClip::new(clip_id, fill_rule, geometry_index)
            .map_err(|error| render_clip_error(format!("{error:?}"), node.span))?;
        Ok((clip_index, geometry, clip))
    }

    fn lower_node(
        &mut self,
        node: &crate::ast::RenderNodeDeclaration,
        layer_index: usize,
        attachment: &CanonicalRenderAttachment,
        parent: Option<usize>,
        document_order: u32,
        node_path: &str,
    ) -> Result<usize, Diagnostic> {
        let node_id =
            self.stable_id(EntityKind::RenderNode, node_path.to_owned(), node.name_span)?;
        if node.kind != CanonicalRenderNodeKind::ClipGroup {
            for item in &node.items {
                let RenderBodyItem::Field(field) = item else {
                    continue;
                };
                if field
                    .path
                    .segments
                    .first()
                    .is_some_and(|segment| segment == "clip")
                {
                    return Err(Diagnostic::new(
                        DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                        DiagnosticStage::Canonical,
                        "clip.* fields are only valid on ClipGroup",
                        field.span,
                    ));
                }
            }
        }
        let geometry_id = (!matches!(
            node.kind,
            CanonicalRenderNodeKind::Group | CanonicalRenderNodeKind::ClipGroup
        ))
        .then(|| {
            self.auxiliary_id(
                EntityKind::RenderGeometry,
                &node_id,
                "geometryRef",
                0,
                node.span,
            )
        })
        .transpose()?;
        let zero_length_vec = || {
            TypedValue::vec2(TypedValue::Length(0.0), TypedValue::Length(0.0))
                .expect("homogeneous length vector")
        };
        let one_float_vec = || {
            TypedValue::vec2(TypedValue::Float(1.0), TypedValue::Float(1.0))
                .expect("homogeneous float vector")
        };
        let position = self.descriptor(render_body_value_or(
            &node.items,
            "position",
            zero_length_vec(),
            self.definitions,
            Ok::<_, Diagnostic>,
        )?)?;
        let origin_value = render_body_value_or(
            &node.items,
            "origin",
            zero_length_vec(),
            self.definitions,
            |value| Ok::<_, Diagnostic>(value),
        )?;
        let origin = self.descriptor(origin_value)?;
        let rotation = self.descriptor(TypedValue::Angle(render_body_value_or(
            &node.items,
            "rotation",
            0.0,
            self.definitions,
            |value| render_angle(value, node.span),
        )?))?;
        let scale = self.descriptor(render_body_value_or(
            &node.items,
            "scale",
            one_float_vec(),
            self.definitions,
            Ok::<_, Diagnostic>,
        )?)?;
        let pattern = if node.kind == CanonicalRenderNodeKind::ClipGroup {
            None
        } else {
            self.pattern_spec(
                node,
                CanonicalPatternTransform {
                    position,
                    origin,
                    rotation,
                    scale,
                },
            )?
        };
        let opacity = match render_body_field(&node.items, "opacity") {
            Some(field) => self.dynamic_opacity_descriptor(field)?,
            None => self.descriptor(TypedValue::Float(1.0))?,
        };
        let visibility = self.descriptor(TypedValue::Bool(render_body_value_or(
            &node.items,
            "visibility",
            true,
            self.definitions,
            |value| render_bool(value, node.span),
        )?))?;
        let z_order = render_body_value_or(&node.items, "zOrder", 0, self.definitions, |value| {
            render_int(value, node.span)
        })?;
        let isolate =
            render_body_value_or(&node.items, "isolate", false, self.definitions, |value| {
                render_bool(value, node.span)
            })?;
        let follow_hidden_attachment = render_body_value_or(
            &node.items,
            "followHiddenAttachment",
            false,
            self.definitions,
            |value| render_bool(value, node.span),
        )?;
        let composite = render_body_value_or(
            &node.items,
            "composite",
            CanonicalRenderComposite::SourceOver,
            self.definitions,
            |value| render_composite(value, node.span),
        )?;
        let active = render_active_interval(&node.items, self.time_map, self.definitions)?;
        let clip = (node.kind == CanonicalRenderNodeKind::ClipGroup)
            .then(|| self.lower_clip(&node_id, node))
            .transpose()?;
        let stroke = match node.kind {
            CanonicalRenderNodeKind::Line => {
                Some(self.render_stroke(&node_id, node, pattern.as_ref())?)
            }
            // Render section 14.2 lets a fillable geometry carry a fill paint, a stroke, or
            // both, so a stroke is optional and only lowered when it is declared.
            CanonicalRenderNodeKind::Rect
            | CanonicalRenderNodeKind::RoundedRect
            | CanonicalRenderNodeKind::Circle
            | CanonicalRenderNodeKind::Ellipse
            | CanonicalRenderNodeKind::Polyline
            | CanonicalRenderNodeKind::Polygon
            | CanonicalRenderNodeKind::Path
            | CanonicalRenderNodeKind::Text => render_body_field(&node.items, "stroke")
                .is_some()
                .then(|| self.render_stroke(&node_id, node, pattern.as_ref()))
                .transpose()?,
            _ => None,
        };

        let (geometry_data, paint) = match node.kind {
            CanonicalRenderNodeKind::Group | CanonicalRenderNodeKind::ClipGroup => (None, None),
            kind @ (CanonicalRenderNodeKind::Rect
            | CanonicalRenderNodeKind::RoundedRect
            | CanonicalRenderNodeKind::Circle
            | CanonicalRenderNodeKind::Ellipse) => {
                let geometry_id = geometry_id
                    .as_ref()
                    .expect("drawable nodes have a preallocated geometry ID");
                let data = self.lower_fill_geometry(
                    node,
                    kind,
                    geometry_id,
                    None,
                    Some(origin),
                    Some(rotation),
                    None,
                )?;
                let paint = if stroke.is_some() && render_body_field(&node.items, "fill").is_none()
                {
                    None
                } else {
                    Some(self.add_paint(&node_id, "fillPaint", node, "fill", pattern.as_ref())?)
                };
                (Some(data), paint)
            }
            CanonicalRenderNodeKind::Line => {
                if render_body_field(&node.items, "fill").is_some() {
                    return Err(render_error("Line must not declare fill", node.span));
                }
                let start_field = render_body_field(&node.items, "start")
                    .ok_or_else(|| render_error("Line requires start", node.span))?;
                let end_field = render_body_field(&node.items, "end")
                    .ok_or_else(|| render_error("Line requires end", node.span))?;
                let start = render_vec2_length(
                    render_value(start_field, self.definitions)?,
                    start_field.span,
                )?;
                let end =
                    render_vec2_length(render_value(end_field, self.definitions)?, end_field.span)?;
                (
                    Some(CanonicalRenderGeometryData::Line {
                        start: self.descriptor(
                            TypedValue::vec2(
                                TypedValue::Length(start[0]),
                                TypedValue::Length(start[1]),
                            )
                            .expect("homogeneous length vector"),
                        )?,
                        end: self.descriptor(
                            TypedValue::vec2(
                                TypedValue::Length(end[0]),
                                TypedValue::Length(end[1]),
                            )
                            .expect("homogeneous length vector"),
                        )?,
                    }),
                    None,
                )
            }
            CanonicalRenderNodeKind::Polyline => {
                let points_field = render_body_field(&node.items, "points")
                    .ok_or_else(|| render_error("Polyline requires points", node.span))?;
                let points = render_value(points_field, self.definitions)?;
                let TypedValue::Array { values, .. } = points else {
                    return Err(render_error(
                        "Polyline points must be an array of vec2<length>",
                        points_field.span,
                    ));
                };
                let points = values
                    .into_iter()
                    .map(|value| {
                        let [x, y] = render_vec2_length(value, points_field.span)?;
                        self.descriptor(
                            TypedValue::vec2(TypedValue::Length(x), TypedValue::Length(y))
                                .expect("homogeneous length vector"),
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                (
                    Some(CanonicalRenderGeometryData::Polyline { points }),
                    // Render section 14.2 requires at least one of the two, so a declared
                    // stroke is what makes `fill` optional here.
                    if stroke.is_some() && render_body_field(&node.items, "fill").is_none() {
                        None
                    } else {
                        Some(self.add_paint(
                            &node_id,
                            "fillPaint",
                            node,
                            "fill",
                            pattern.as_ref(),
                        )?)
                    },
                )
            }
            kind @ (CanonicalRenderNodeKind::Polygon | CanonicalRenderNodeKind::Path) => {
                let geometry_id = geometry_id
                    .as_ref()
                    .expect("drawable nodes have a preallocated geometry ID");
                let fill_rule = (kind == CanonicalRenderNodeKind::Path)
                    .then(|| self.render_fill_rule(node, None))
                    .transpose()?;
                let data = self.lower_fill_geometry(
                    node,
                    kind,
                    geometry_id,
                    None,
                    Some(origin),
                    Some(rotation),
                    fill_rule,
                )?;
                let paint = if stroke.is_some() && render_body_field(&node.items, "fill").is_none()
                {
                    None
                } else {
                    Some(self.add_paint(&node_id, "fillPaint", node, "fill", pattern.as_ref())?)
                };
                (Some(data), paint)
            }
            CanonicalRenderNodeKind::Text => {
                let geometry_id = geometry_id
                    .as_ref()
                    .expect("Text nodes have a preallocated geometry ID");
                let glyph_runs = self.lower_text(node, geometry_id)?;
                let paint = if stroke.is_some() && render_body_field(&node.items, "fill").is_none()
                {
                    None
                } else {
                    Some(self.add_paint(&node_id, "fillPaint", node, "fill", pattern.as_ref())?)
                };
                (
                    Some(CanonicalRenderGeometryData::Text { glyph_runs, origin }),
                    paint,
                )
            }
            CanonicalRenderNodeKind::Image => {
                let resource_field = render_body_field(&node.items, "resource")
                    .ok_or_else(|| render_error("Image requires resource", node.span))?;
                let resource_name = match render_value(resource_field, self.definitions)? {
                    TypedValue::Line(name) => name,
                    other => {
                        return Err(render_error(
                            format!("Image resource must be a reference, found {}", other.ty()),
                            resource_field.span,
                        ));
                    }
                };
                let resource = self.resources.get(&resource_name).ok_or_else(|| {
                    render_error(
                        format!("Image references unknown resource {resource_name}"),
                        resource_field.span,
                    )
                })?;
                if !matches!(
                    resource.kind(),
                    CanonicalResourceKind::Image | CanonicalResourceKind::Texture
                ) {
                    return Err(render_error(
                        format!("Image resource {resource_name} is not image/texture"),
                        resource_field.span,
                    ));
                }
                let sampling = match render_body_field(&node.items, "sampling") {
                    Some(field) => {
                        render_image_sampling(render_value(field, self.definitions)?, field.span)?
                    }
                    None => render_resource_sampling(resource, resource_field.span)?,
                };
                let resource_id = self.resource_id(&resource_name, resource_field.span)?;
                let destination_origin_field = render_body_field(&node.items, "destination.origin")
                    .ok_or_else(|| render_error("Image requires destination.origin", node.span))?;
                let destination_origin = render_vec2_length(
                    render_value(destination_origin_field, self.definitions)?,
                    destination_origin_field.span,
                )?;
                let destination_size_field = render_body_field(&node.items, "destination.size")
                    .ok_or_else(|| render_error("Image requires destination.size", node.span))?;
                let destination_size = render_vec2_length(
                    render_value(destination_size_field, self.definitions)?,
                    destination_size_field.span,
                )?;
                if destination_size.iter().any(|value| *value < 0.0) {
                    return Err(render_error(
                        "Image destination.size must be non-negative",
                        destination_size_field.span,
                    ));
                }
                let source_origin_field = render_body_field(&node.items, "sourceRect.origin");
                let source_size_field = render_body_field(&node.items, "sourceRect.size");
                if source_origin_field.is_some() != source_size_field.is_some() {
                    return Err(render_error(
                        "Image sourceRect.origin and sourceRect.size must be paired",
                        node.span,
                    ));
                }
                let source = match (source_origin_field, source_size_field) {
                    (Some(origin_field), Some(size_field)) => {
                        let source_origin = render_vec2_float(
                            render_value(origin_field, self.definitions)?,
                            origin_field.span,
                        )?;
                        let source_size = render_vec2_float(
                            render_value(size_field, self.definitions)?,
                            size_field.span,
                        )?;
                        if source_origin.iter().any(|value| *value < 0.0)
                            || source_size.iter().any(|value| *value < 0.0)
                        {
                            return Err(render_error(
                                "Image sourceRect origin and size must be non-negative",
                                node.span,
                            ));
                        }
                        Some([
                            self.descriptor(TypedValue::Float(source_origin[0]))?,
                            self.descriptor(TypedValue::Float(source_origin[1]))?,
                            self.descriptor(TypedValue::Float(source_size[0]))?,
                            self.descriptor(TypedValue::Float(source_size[1]))?,
                        ])
                    }
                    (None, None) => None,
                    _ => unreachable!("paired sourceRect fields were checked"),
                };
                let destination = [
                    self.descriptor(TypedValue::Length(destination_origin[0]))?,
                    self.descriptor(TypedValue::Length(destination_origin[1]))?,
                    self.descriptor(TypedValue::Length(destination_size[0]))?,
                    self.descriptor(TypedValue::Length(destination_size[1]))?,
                ];
                (
                    Some(CanonicalRenderGeometryData::Image {
                        resource: resource_id,
                        destination,
                        source,
                        sampling,
                    }),
                    None,
                )
            }
            other => {
                return Err(render_error(
                    format!("product Render lowering does not support {:?} nodes", other),
                    node.span,
                ));
            }
        };

        let geometry = geometry_data
            .map(|data| {
                let id = geometry_id.expect("drawable nodes have a preallocated geometry ID");
                CanonicalRenderGeometry::new(id, data)
                    .map_err(|error| render_error(format!("{error:?}"), node.span))
            })
            .transpose()?;
        let geometry_index = geometry.as_ref().map(|_| self.geometries.len());
        let fill_paint = paint.map(|paint| {
            let index = self.paints.len();
            self.paints.push(paint);
            index
        });
        let stroke_index = if let Some(stroke) = stroke {
            let paint_index = self.paints.len();
            self.paints.push(stroke.paint);
            let stroke_index = self.strokes.len();
            self.strokes.push(
                CanonicalRenderStroke::new(
                    stroke.id,
                    paint_index,
                    stroke.width,
                    stroke.cap,
                    stroke.join,
                    stroke.miter_limit,
                    stroke.dash_offset,
                    stroke.dash,
                )
                .map_err(|error| render_error(format!("{error:?}"), node.span))?,
            );
            Some(stroke_index)
        } else {
            None
        };
        let clip_index = clip.as_ref().map(|(index, _, _)| *index);
        let node_index = self.nodes.len();
        let canonical_node = CanonicalRenderNode::new(CanonicalRenderNodeSpec {
            id: node_id.clone(),
            kind: node.kind,
            parent,
            layer: layer_index,
            document_order,
            z_order,
            attachment: attachment.clone(),
            active,
            isolate,
            follow_hidden_attachment,
            position,
            origin,
            rotation,
            scale,
            opacity,
            visibility,
            geometry: geometry_index,
            fill_paint,
            stroke: stroke_index,
            clip: clip_index,
            composite,
        })
        .map_err(|error| render_error(format!("{error:?}"), node.span))?;
        self.nodes.push(canonical_node);
        self.add_node_roots(
            &node_id,
            [position, origin, rotation, scale, opacity, visibility],
        );
        if let Some(geometry) = geometry {
            self.add_geometry_roots(&geometry);
            self.geometries.push(geometry);
        }
        if let Some((_, clip_geometry, clip_record)) = clip {
            self.add_geometry_roots(&clip_geometry);
            self.geometries.push(clip_geometry);
            self.clips.push(clip_record);
        }
        if let Some(paint_index) = fill_paint {
            self.add_paint_roots(paint_index);
        }
        if let Some(stroke_index) = stroke_index {
            let stroke_paint = self.strokes[stroke_index].paint();
            self.add_paint_roots(stroke_paint);
            self.add_stroke_roots(stroke_index);
        }

        if let Some(children) = node.items.iter().find_map(|item| match item {
            RenderBodyItem::Children(children) => Some(&children.items),
            _ => None,
        }) {
            for (child_order, item) in children.iter().enumerate() {
                let RenderItem::Node(child) = item else {
                    return Err(render_error(
                        "Render children must contain concrete nodes",
                        item.span(),
                    ));
                };
                self.lower_node(
                    child,
                    layer_index,
                    attachment,
                    Some(node_index),
                    child_order as u32,
                    &format!("{node_path}/node/{}", child.name),
                )?;
            }
        }
        Ok(node_index)
    }
}

fn lower_render_scene(
    scene: &crate::ast::RenderScene,
    resources: &BTreeMap<String, CanonicalResource>,
    resource_bundle: Option<&CanonicalResourceBundle>,
    time_map: &ChartTimeMap,
    lines: &CanonicalLineGraph,
    notes: &CanonicalNoteSet,
    definitions: Option<&DefinitionsBlock>,
    span: SourceSpan,
) -> Result<(CanonicalRenderScene, CanonicalDescriptorTable), Vec<Diagnostic>> {
    let result = (|| {
        let viewport_width = render_field(&scene.viewport.fields, "width")
            .ok_or_else(|| render_error("Render viewport requires width", scene.viewport.span))
            .and_then(|field| render_length(render_value(field, definitions)?, field.span))?;
        let viewport_height = render_field(&scene.viewport.fields, "height")
            .ok_or_else(|| render_error("Render viewport requires height", scene.viewport.span))
            .and_then(|field| render_length(render_value(field, definitions)?, field.span))?;
        let color_space = match render_value_or(
            &scene.viewport.fields,
            "colorSpace",
            "linear-srgb".to_owned(),
            definitions,
            |value| render_string(value, scene.viewport.span),
        )?
        .as_str()
        {
            "linear-srgb" => CanonicalRenderColorSpace::LinearSrgb,
            "srgb" => CanonicalRenderColorSpace::Srgb,
            other => {
                return Err(render_error(
                    format!("unsupported Render colorSpace {other}"),
                    scene.viewport.span,
                ));
            }
        };
        let mut lowerer =
            RenderLowerer::new(resources, resource_bundle, time_map, definitions, span);
        let mut layers = Vec::new();
        for (layer_index, layer) in scene.layers.iter().enumerate() {
            let pass = render_body_field(&layer.items, "pass")
                .ok_or_else(|| render_error("Render layer requires pass", layer.span))
                .and_then(|field| render_string(render_value(field, definitions)?, field.span))
                .and_then(|value| {
                    CanonicalRenderPass::from_spelling(&value).ok_or_else(|| {
                        render_error(format!("unsupported Render pass {value}"), layer.span)
                    })
                })?;
            let z_order = render_body_value_or(&layer.items, "zOrder", 0, definitions, |value| {
                render_int(value, layer.span)
            })?;
            let attachment = match render_unique_body_field(&layer.items, "space")? {
                Some(field) => {
                    let SchemaValue::Expression(expression) = &field.value else {
                        return Err(render_error(
                            "Render layer space must be an expression",
                            field.span,
                        ));
                    };
                    render_attachment(expression, definitions, lines, notes)?
                }
                None => CanonicalRenderAttachment::World,
            };
            let layer_id = lowerer.stable_id(
                EntityKind::RenderLayer,
                format!("layer/{}", layer.name),
                layer.name_span,
            )?;
            let mut roots = Vec::new();
            if let Some(children) = layer.items.iter().find_map(|item| match item {
                RenderBodyItem::Children(children) => Some(&children.items),
                _ => None,
            }) {
                for (document_order, item) in children.iter().enumerate() {
                    let RenderItem::Node(node) = item else {
                        return Err(render_error(
                            "Render children must contain concrete nodes",
                            item.span(),
                        ));
                    };
                    roots.push(lowerer.lower_node(
                        node,
                        layer_index,
                        &attachment,
                        None,
                        document_order as u32,
                        &format!("layer/{}/node/{}", layer.name, node.name),
                    )?);
                }
            }
            layers.push(
                CanonicalRenderLayer::new(layer_id, pass, z_order, layer_index as u32, roots)
                    .map_err(|error| render_error(format!("{error:?}"), layer.span))?,
            );
        }

        let descriptor_roots = lowerer
            .descriptor_roots
            .into_iter()
            .map(|(target_path, owner, descriptor)| {
                CanonicalDescriptorRoot::new(target_path, owner, descriptor)
                    .map_err(|error| render_error(format!("{error:?}"), span))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let local_descriptors = lowerer.descriptors.clone();
        let table = CanonicalDescriptorTable::new(lowerer.descriptors, descriptor_roots)
            .map_err(|error| render_error(format!("{error:?}"), span))?;
        let mapping = local_descriptors
            .iter()
            .map(|descriptor| {
                table
                    .descriptors()
                    .iter()
                    .position(|candidate| candidate == descriptor)
                    .ok_or_else(|| render_error("descriptor interning lost a Render value", span))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut render = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
            viewport: CanonicalViewport::new(viewport_width, viewport_height, color_space)
                .map_err(|error| render_error(format!("{error:?}"), span))?,
            layers,
            nodes: lowerer.nodes,
            geometries: lowerer.geometries,
            paths: lowerer.paths,
            paints: lowerer.paints,
            strokes: lowerer.strokes,
            clips: lowerer.clips,
            glyph_runs: lowerer.glyph_runs,
        })
        .map_err(|error| render_error(format!("{error:?}"), span))?;
        render
            .remap_descriptors(&mapping)
            .map_err(|error| render_error(format!("{error:?}"), span))?;
        Ok((render, table))
    })();
    result.map_err(|diagnostic| vec![diagnostic])
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

fn lower_required_extensions(
    document: &Document,
) -> Result<Vec<CanonicalRequiredExtension>, Vec<Diagnostic>> {
    let contributors = contributor_names(document.contributors.as_ref());
    let resources = resource_kinds(document.resources.as_ref());
    let mut diagnostics = Vec::new();
    let mut extensions = Vec::new();
    for declaration in document
        .extensions
        .iter()
        .flat_map(|block| &block.declarations)
        .filter(|declaration| declaration.requirement == ExtensionRequirement::Required)
    {
        let Some(payload) = lower_ordered_object(
            &declaration.payload,
            document.definitions.as_ref(),
            &contributors,
            &resources,
            CustomValueLimits::default(),
            &mut diagnostics,
        ) else {
            continue;
        };
        match CanonicalRequiredExtension::with_payload(
            declaration.header.namespace.clone(),
            declaration.header.version.to_string(),
            payload,
        ) {
            Ok(extension) => extensions.push(extension),
            Err(error) => diagnostics.push(chart_diagnostic(error, declaration.span)),
        }
    }
    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.code().cmp(&right.code()))
    });
    if diagnostics.is_empty() {
        Ok(extensions)
    } else {
        Err(diagnostics)
    }
}

fn lower_ordered_object(
    object: &OrderedObject,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    limits: CustomValueLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalObject> {
    let raw = RawValue::Object(
        object
            .entries
            .iter()
            .filter_map(|entry| {
                lower_expression(&entry.value, definitions, &mut Vec::new(), diagnostics).map(
                    |value| RawObjectEntry {
                        key: entry.key.clone(),
                        key_span: entry.key_span,
                        value,
                    },
                )
            })
            .collect(),
    );
    let mut total_bytes = 0;
    match resolve_raw(
        raw,
        &Expected::Object,
        contributors,
        resources,
        limits,
        diagnostics,
        1,
        &mut total_bytes,
        object.span,
    )? {
        CanonicalValue::Object(value) => Some(value),
        _ => None,
    }
}

fn lower_document(
    document: &Document,
    limits: CustomValueLimits,
) -> Result<CanonicalMetadata, Vec<Diagnostic>> {
    lower_document_with_sources_and_limits(document, limits).map(|lowered| lowered.metadata)
}

pub(crate) fn lower_document_with_sources(
    document: &Document,
) -> Result<LoweredDocument, Vec<Diagnostic>> {
    lower_document_with_sources_and_limits(document, CustomValueLimits::default())
}

fn lower_document_with_sources_and_limits(
    document: &Document,
    limits: CustomValueLimits,
) -> Result<LoweredDocument, Vec<Diagnostic>> {
    if let Some(definitions) = document.definitions.as_ref() {
        crate::elaborator::preflight_definition_cycles(definitions)
            .map_err(|diagnostic| vec![diagnostic])?;
    }
    let contributor_names = contributor_names(document.contributors.as_ref());
    let resource_kinds = resource_kinds(document.resources.as_ref());
    let mut diagnostics = Vec::new();

    let contributors = lower_contributors(
        document.contributors.as_ref(),
        document.definitions.as_ref(),
        limits,
        &mut diagnostics,
    );
    let resources = lower_resources(
        document.resources.as_ref(),
        document.definitions.as_ref(),
        limits,
        &mut diagnostics,
    );
    let meta = lower_meta(
        document.meta.as_ref(),
        document.definitions.as_ref(),
        &contributor_names,
        &resource_kinds,
        limits,
        &mut diagnostics,
    );
    let credits = lower_credits(
        document.credits.as_ref(),
        document.definitions.as_ref(),
        &contributor_names,
        &resource_kinds,
        limits,
        &mut diagnostics,
    );
    let artwork = lower_artwork(
        document.artwork.as_ref(),
        document.definitions.as_ref(),
        &resource_kinds,
        limits,
        &mut diagnostics,
    );
    let sync = lower_sync(
        document.sync.as_ref(),
        document.definitions.as_ref(),
        &resource_kinds,
        limits,
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
    limits: CustomValueLimits,
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
        limits,
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
    match values.remove("level") {
        Some(CanonicalValue::Int(level)) => {
            values.insert("level".into(), CanonicalValue::Float(level as f64));
        }
        Some(level) => {
            values.insert("level".into(), level);
        }
        None => {}
    }
    Some(values)
}

fn lower_contributors(
    block: Option<&crate::ast::ContributorsBlock>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    limits: CustomValueLimits,
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
            limits,
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
    limits: CustomValueLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<CanonicalCredit> {
    let mut output = Vec::new();
    let Some(block) = block else { return output };
    let mut registry = StableIdRegistry::new();
    for entry in &block.entries {
        let mut expected = BTreeMap::new();
        expected.insert("id", Expected::String);
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
            limits,
            diagnostics,
            "credit",
        );
        let Some(textual_id) = string_field(&fields, "id", entry.span, diagnostics, "credit ID")
        else {
            continue;
        };
        let textual_id = match CanonicalTextualId::explicit(textual_id) {
            Ok(textual_id) => textual_id,
            Err(error) => {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::NAME_DUPLICATE,
                    error.to_string(),
                    entry.span,
                ));
                continue;
            }
        };
        let id = match registry.insert(EntityKind::Credit, textual_id) {
            Ok(id) => id,
            Err(error) => {
                diagnostics.push(canonical_diagnostic(
                    DiagnosticCode::NAME_DUPLICATE,
                    error.to_string(),
                    entry.span,
                ));
                continue;
            }
        };
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
        match CanonicalCredit::new(id, role, label, credit_contributors) {
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
    limits: CustomValueLimits,
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
            limits,
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
    limits: CustomValueLimits,
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
        limits,
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
    limits: CustomValueLimits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalSync> {
    let block = block?;
    let mut previous = BTreeMap::<String, SourceSpan>::new();
    let mut primary_audio = None;
    let mut audio_offset = AudioOffset::new(0.0).expect("zero audio offset is finite");
    let mut preview = None;
    for field in &block.fields {
        let mut total_bytes = 0usize;
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
                    limits,
                    diagnostics,
                    1,
                    &mut total_bytes,
                    field.span,
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
                    limits,
                    diagnostics,
                    1,
                    &mut total_bytes,
                    field.span,
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
                        limits,
                        diagnostics,
                        1,
                        &mut total_bytes,
                        field.span,
                    ) else {
                        continue;
                    };
                    let Some(end) = resolve_raw(
                        end,
                        &Expected::Time,
                        &BTreeSet::new(),
                        resources,
                        limits,
                        diagnostics,
                        1,
                        &mut total_bytes,
                        field.span,
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
    match CanonicalSync::new(primary_audio, audio_offset, preview) {
        Ok(sync) => Some(sync),
        Err(fcs_model::CanonicalSyncError::PreviewRequiresPrimaryAudio) => {
            diagnostics.push(canonical_diagnostic(
                DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                "sync preview requires primaryAudio",
                block.span,
            ));
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_fields(
    fields: &[SchemaField],
    expected: &BTreeMap<&str, Expected>,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    limits: CustomValueLimits,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &str,
) -> BTreeMap<String, CanonicalValue> {
    let mut output = BTreeMap::new();
    let mut previous = BTreeMap::<String, SourceSpan>::new();
    let mut total_bytes = 0usize;
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
        if let Some(value) = resolve_raw(
            raw,
            expected_type,
            contributors,
            resources,
            limits,
            diagnostics,
            1,
            &mut total_bytes,
            field.span,
        ) {
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

#[allow(clippy::too_many_arguments)]
fn resolve_raw(
    raw: RawValue,
    expected: &Expected,
    contributors: &BTreeSet<String>,
    resources: &BTreeMap<String, ResourceKind>,
    limits: CustomValueLimits,
    diagnostics: &mut Vec<Diagnostic>,
    depth: usize,
    total_bytes: &mut usize,
    span: SourceSpan,
) -> Option<CanonicalValue> {
    if depth > limits.max_depth() {
        diagnostics.push(custom_limit_diagnostic(
            "custom-depth",
            limits.max_depth(),
            depth,
            span,
        ));
        return None;
    }
    match raw {
        RawValue::Reference { name, span } => {
            charge_bytes(total_bytes, limits, name.len(), span, diagnostics)?;
            resolve_reference(name, span, expected, contributors, resources, diagnostics)
        }
        RawValue::Array(values) => {
            charge_bytes(total_bytes, limits, 8, span, diagnostics)?;
            let expected_element = match expected {
                Expected::Array(element) => Some(element.as_ref()),
                _ => None,
            };
            let mut output = Vec::new();
            for value in values {
                let child_span = span;
                let Some(value) = resolve_raw(
                    value,
                    expected_element.unwrap_or(&Expected::Any),
                    contributors,
                    resources,
                    limits,
                    diagnostics,
                    depth + 1,
                    total_bytes,
                    child_span,
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
                    span,
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
                    span,
                ));
                return None;
            }
            match CanonicalValue::typed_array(element_type, output) {
                Ok(value) => value_matches_expected(value, expected, diagnostics),
                Err(_) => None,
            }
        }
        RawValue::Object(entries) => {
            charge_bytes(total_bytes, limits, 8, span, diagnostics)?;
            if !matches!(
                expected,
                Expected::Any | Expected::Object | Expected::StringObject
            ) {
                diagnostics.push(type_mismatch(expected, "object", span));
                return None;
            }
            if entries.len() > limits.max_fields() {
                diagnostics.push(custom_limit_diagnostic(
                    "custom-fields",
                    limits.max_fields(),
                    entries.len(),
                    span,
                ));
                return None;
            }
            let mut keys = BTreeSet::new();
            let mut output = Vec::new();
            for entry in entries {
                charge_bytes(
                    total_bytes,
                    limits,
                    entry.key.len(),
                    entry.key_span,
                    diagnostics,
                )?;
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
                    limits,
                    diagnostics,
                    depth + 1,
                    total_bytes,
                    entry.key_span,
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
            match &value {
                CanonicalValue::String(s) => {
                    if s.len() > limits.max_string_bytes() {
                        diagnostics.push(custom_limit_diagnostic(
                            "custom-string-bytes",
                            limits.max_string_bytes(),
                            s.len(),
                            span,
                        ));
                        return None;
                    }
                    charge_bytes(total_bytes, limits, s.len(), span, diagnostics)?;
                }
                CanonicalValue::Null
                | CanonicalValue::Bool(_)
                | CanonicalValue::Int(_)
                | CanonicalValue::Float(_)
                | CanonicalValue::Time(_)
                | CanonicalValue::Beat(_)
                | CanonicalValue::Color(_)
                | CanonicalValue::ResourceReference(_)
                | CanonicalValue::ContributorReference(_) => {
                    charge_bytes(total_bytes, limits, 8, span, diagnostics)?;
                }
                CanonicalValue::Array { .. } | CanonicalValue::Object(_) => {
                    charge_bytes(total_bytes, limits, 8, span, diagnostics)?;
                }
            }
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

fn charge_bytes(
    total_bytes: &mut usize,
    limits: CustomValueLimits,
    amount: usize,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<()> {
    let next = total_bytes.saturating_add(amount);
    if next > limits.max_total_bytes() {
        diagnostics.push(custom_limit_diagnostic(
            "custom-total-bytes",
            limits.max_total_bytes(),
            next,
            span,
        ));
        return None;
    }
    *total_bytes = next;
    Some(())
}

fn custom_limit_diagnostic(
    kind: &'static str,
    limit: usize,
    observed: usize,
    span: SourceSpan,
) -> Diagnostic {
    canonical_diagnostic(
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
        format!("resource limit {kind} exceeded: limit {limit}, observed {observed}"),
        span,
    )
    .with_budget(kind, limit, observed)
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
                    match kind {
                        Some(ReferenceKind::Resource) => DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
                        Some(ReferenceKind::Contributor) | None => DiagnosticCode::NAME_UNKNOWN,
                    },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_canonical_image_sampling_uses_the_render_diagnostic() {
        for metadata in [
            CanonicalObject::new(Vec::new()).expect("empty metadata"),
            CanonicalObject::new(vec![CanonicalObjectEntry::new(
                "sampling",
                CanonicalValue::Int(1),
            )])
            .expect("typed metadata"),
            CanonicalObject::new(vec![CanonicalObjectEntry::new(
                "sampling",
                CanonicalValue::String("cubic".to_owned()),
            )])
            .expect("enum metadata"),
        ] {
            let resource = CanonicalResource::new(
                "sprite",
                CanonicalResourceKind::Image,
                "image/png",
                None,
                metadata,
            );
            let error = render_resource_sampling(&resource, SourceSpan::new(0, 0))
                .expect_err("invalid sampling metadata must fail");
            assert_eq!(error.code(), DiagnosticCode::RENDER_RESOURCE_DECODE_FAILED);
        }
    }
}
