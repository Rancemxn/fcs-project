//! I3.3 source-to-canonical lowering for metadata, resources, artwork, sync,
//! and typed custom values.

use std::collections::{BTreeMap, BTreeSet};

use fcs_model::{
    AudioOffset, Beat as CanonicalBeat, CanonicalActiveInterval, CanonicalArtwork, CanonicalChart,
    CanonicalChartError, CanonicalColor, CanonicalCompilation, CanonicalContributor,
    CanonicalCredit, CanonicalCreditRole, CanonicalDescriptorDomain, CanonicalDescriptorKind,
    CanonicalDescriptorRoot, CanonicalDescriptorTable, CanonicalExpressionType,
    CanonicalExpressionValue, CanonicalGradientSpread, CanonicalGradientStop,
    CanonicalImageSampling, CanonicalLineGraph, CanonicalMetadata, CanonicalObject,
    CanonicalObjectEntry, CanonicalPreview, CanonicalProfile, CanonicalProfileFeature,
    CanonicalPropertyDescriptor, CanonicalRenderAttachment, CanonicalRenderColorSpace,
    CanonicalRenderComposite, CanonicalRenderGeometry, CanonicalRenderGeometryData,
    CanonicalRenderLayer, CanonicalRenderNode, CanonicalRenderNodeKind, CanonicalRenderNodeSpec,
    CanonicalRenderPaint, CanonicalRenderPaintData, CanonicalRenderPass, CanonicalRenderScene,
    CanonicalRenderSceneSpec, CanonicalRenderStroke, CanonicalRequiredExtension, CanonicalResource,
    CanonicalResourceKind, CanonicalSourceVersion, CanonicalStrokeCap, CanonicalStrokeJoin,
    CanonicalSync, CanonicalTextualId, CanonicalValue, CanonicalValueType, CanonicalViewport,
    ChartTimeMap, DeclaredSha256, DistributionMetadata, EntityKind, StableId, StableIdRegistry,
};

use crate::ast::{
    Definition, Document, DocumentProfile, ExtensionRequirement, FieldPath, MetaBlock,
    OrderedObject, ProfileFeature, RenderBodyItem, RenderItem, ResourceKind, SchemaField,
    SchemaValue, SourceExpression, SourceLiteral, SourceSpan, SyncBlock, TopLevelBlockKind,
    TypedValue,
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
        let mut chart = self.canonical_chart(limits)?;
        let Some(crate::ast::TopLevelBlock::Render(block)) =
            self.top_level(TopLevelBlockKind::Render)
        else {
            return Ok(chart);
        };
        let scene = crate::parser::parse_render_scene(source, block).into_result()?;
        let (mut render, render_descriptors) = lower_render_scene(
            &scene,
            chart.metadata().resources(),
            chart.time_map(),
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
        let chart = self.canonical_chart_with_source(source, limits)?;
        let resources = self.canonical_resource_bundle(workspace_root, resource_limits)?;
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

fn render_value(field: &SchemaField) -> Result<TypedValue, Diagnostic> {
    let SchemaValue::Expression(expression) = &field.value else {
        return Err(render_error(
            "Render field must be a compile-time expression",
            field.span,
        ));
    };
    crate::elaborator::evaluate_metadata_expression(expression, None)
}

enum RenderPaintExpression {
    Solid(TypedValue),
    LinearGradient {
        start: TypedValue,
        end: TypedValue,
        spread: CanonicalGradientSpread,
        stops: Vec<(f64, TypedValue)>,
    },
    RadialGradient {
        start_center: TypedValue,
        start_radius: TypedValue,
        end_center: TypedValue,
        end_radius: TypedValue,
        spread: CanonicalGradientSpread,
        stops: Vec<(f64, TypedValue)>,
    },
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

fn render_gradient_stops(
    stops: &SourceExpression,
    field_span: SourceSpan,
    gradient_name: &str,
) -> Result<Vec<(f64, TypedValue)>, Diagnostic> {
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
            crate::elaborator::evaluate_metadata_expression(offset, None)?,
            offset.span(),
        )?;
        let color = crate::elaborator::evaluate_metadata_expression(color, None)?;
        parsed_stops.push((offset, color));
    }
    Ok(parsed_stops)
}

fn render_gradient_spread(value: &SourceExpression) -> Result<CanonicalGradientSpread, Diagnostic> {
    let spread = crate::elaborator::evaluate_metadata_expression(value, None)?;
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

fn render_paint_expression(field: &SchemaField) -> Result<RenderPaintExpression, Diagnostic> {
    let SchemaValue::Expression(expression) = &field.value else {
        return Err(render_error(
            "Render paint must be a compile-time expression",
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
        return crate::elaborator::evaluate_metadata_expression(argument, None)
            .map(RenderPaintExpression::Solid);
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
        let start = crate::elaborator::evaluate_metadata_expression(start, None)?;
        let end = crate::elaborator::evaluate_metadata_expression(end, None)?;
        let parsed_stops = render_gradient_stops(stops, field.span, "linearGradient")?;
        let spread = render_gradient_spread(spread)?;
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
        let start_center = crate::elaborator::evaluate_metadata_expression(start_center, None)?;
        let start_radius = crate::elaborator::evaluate_metadata_expression(start_radius, None)?;
        let end_center = crate::elaborator::evaluate_metadata_expression(end_center, None)?;
        let end_radius = crate::elaborator::evaluate_metadata_expression(end_radius, None)?;
        let parsed_stops = render_gradient_stops(stops, field.span, "radialGradient")?;
        let spread = render_gradient_spread(spread)?;
        return Ok(RenderPaintExpression::RadialGradient {
            start_center,
            start_radius,
            end_center,
            end_radius,
            spread,
            stops: parsed_stops,
        });
    }
    Err(render_error(
        "Render paint must use solid(color), linearGradient(start, end, stops, spread), or radialGradient(startCenter, startRadius, endCenter, endRadius, stops, spread)",
        field.span,
    ))
}

fn render_value_or<T>(
    fields: &[SchemaField],
    path: &str,
    default: T,
    convert: impl FnOnce(TypedValue) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    match render_field(fields, path) {
        Some(field) => convert(render_value(field)?),
        None => Ok(default),
    }
}

fn render_body_value_or<T>(
    items: &[RenderBodyItem],
    path: &str,
    default: T,
    convert: impl FnOnce(TypedValue) -> Result<T, Diagnostic>,
) -> Result<T, Diagnostic> {
    match render_body_field(items, path) {
        Some(field) => convert(render_value(field)?),
        None => Ok(default),
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

fn render_dash(value: TypedValue, span: SourceSpan) -> Result<Vec<f64>, Diagnostic> {
    let TypedValue::Array { values, .. } = value else {
        return Err(render_error(
            "Render dash must be an array of lengths",
            span,
        ));
    };
    let mut dash = Vec::with_capacity(values.len());
    for value in values {
        let value = render_length(value, span)?;
        if value < 0.0 {
            return Err(render_error(
                "Render dash elements must be non-negative",
                span,
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

fn render_radius(value: TypedValue, span: SourceSpan) -> Result<f64, Diagnostic> {
    let value = render_length(value, span)?;
    if value < 0.0 {
        return Err(render_error(
            "Render gradient radius must be non-negative",
            span,
        ));
    }
    Ok(value)
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
    let start = crate::elaborator::evaluate_metadata_expression(start, None)?;
    let end = crate::elaborator::evaluate_metadata_expression(end, None)?;
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

struct RenderLowerer<'a> {
    resources: &'a BTreeMap<String, CanonicalResource>,
    time_map: &'a ChartTimeMap,
    span: SourceSpan,
    descriptors: Vec<CanonicalPropertyDescriptor>,
    descriptor_values: Vec<(CanonicalExpressionType, CanonicalExpressionValue)>,
    registry: StableIdRegistry,
    resource_ids: BTreeMap<String, StableId>,
    nodes: Vec<CanonicalRenderNode>,
    geometries: Vec<CanonicalRenderGeometry>,
    paints: Vec<CanonicalRenderPaint>,
    strokes: Vec<CanonicalRenderStroke>,
    descriptor_roots: Vec<(String, u64, usize)>,
}

impl<'a> RenderLowerer<'a> {
    fn new(
        resources: &'a BTreeMap<String, CanonicalResource>,
        time_map: &'a ChartTimeMap,
        span: SourceSpan,
    ) -> Self {
        Self {
            resources,
            time_map,
            span,
            descriptors: Vec::new(),
            descriptor_values: Vec::new(),
            registry: StableIdRegistry::new(),
            resource_ids: BTreeMap::new(),
            nodes: Vec::new(),
            geometries: Vec::new(),
            paints: Vec::new(),
            strokes: Vec::new(),
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
        self.descriptor_values.push((ty, canonical));
        Ok(index)
    }

    fn stable_id(
        &mut self,
        kind: EntityKind,
        textual: String,
        span: SourceSpan,
    ) -> Result<StableId, Diagnostic> {
        render_stable_id(&mut self.registry, kind, textual, span)
    }

    fn add_descriptor_root(&mut self, path: &str, owner: u64, descriptor: usize) {
        self.descriptor_roots
            .push((path.to_owned(), owner, descriptor));
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
            _ => {}
        }
    }

    fn add_paint(
        &mut self,
        node_path: &str,
        node: &crate::ast::RenderNodeDeclaration,
        field_name: &str,
    ) -> Result<CanonicalRenderPaint, Diagnostic> {
        let field = render_body_field(&node.items, field_name).ok_or_else(|| {
            render_error(
                format!("drawable Render node requires {field_name}"),
                node.span,
            )
        })?;
        let id = self.stable_id(
            EntityKind::RenderPaint,
            format!("{node_path}/{field_name}"),
            field.span,
        )?;
        let data = match render_paint_expression(field)? {
            RenderPaintExpression::Solid(TypedValue::Color(color)) => {
                CanonicalRenderPaintData::Solid {
                    color: self.descriptor(TypedValue::Color(color))?,
                }
            }
            RenderPaintExpression::Solid(value) => {
                return Err(render_error(
                    format!("solid paint requires a color, found {}", value.ty()),
                    field.span,
                ));
            }
            RenderPaintExpression::LinearGradient {
                start,
                end,
                spread,
                stops,
            } => {
                let start = render_vec2_length(start, field.span)?;
                let end = render_vec2_length(end, field.span)?;
                let start = self.descriptor(
                    TypedValue::vec2(TypedValue::Length(start[0]), TypedValue::Length(start[1]))
                        .expect("homogeneous length vector"),
                )?;
                let end = self.descriptor(
                    TypedValue::vec2(TypedValue::Length(end[0]), TypedValue::Length(end[1]))
                        .expect("homogeneous length vector"),
                )?;
                let stops = stops
                    .into_iter()
                    .map(|(offset, color)| {
                        let TypedValue::Color(color) = color else {
                            return Err(render_error(
                                "linearGradient stop requires a color",
                                field.span,
                            ));
                        };
                        let color = self.descriptor(TypedValue::Color(color))?;
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
                let start_center = render_vec2_length(start_center, field.span)?;
                let start_radius = render_radius(start_radius, field.span)?;
                let end_center = render_vec2_length(end_center, field.span)?;
                let end_radius = render_radius(end_radius, field.span)?;
                let start_center = self.descriptor(
                    TypedValue::vec2(
                        TypedValue::Length(start_center[0]),
                        TypedValue::Length(start_center[1]),
                    )
                    .expect("homogeneous length vector"),
                )?;
                let start_radius = self.descriptor(TypedValue::Length(start_radius))?;
                let end_center = self.descriptor(
                    TypedValue::vec2(
                        TypedValue::Length(end_center[0]),
                        TypedValue::Length(end_center[1]),
                    )
                    .expect("homogeneous length vector"),
                )?;
                let end_radius = self.descriptor(TypedValue::Length(end_radius))?;
                let stops = stops
                    .into_iter()
                    .map(|(offset, color)| {
                        let TypedValue::Color(color) = color else {
                            return Err(render_error(
                                "radialGradient stop requires a color",
                                field.span,
                            ));
                        };
                        let color = self.descriptor(TypedValue::Color(color))?;
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
            _ => Vec::new(),
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
        node_path: &str,
        node: &crate::ast::RenderNodeDeclaration,
    ) -> Result<RenderStrokeSpec, Diagnostic> {
        let stroke_field = render_body_field(&node.items, "stroke")
            .ok_or_else(|| render_error("drawable Render node requires stroke", node.span))?;
        let paint = self.add_paint(node_path, node, "stroke")?;
        let width_field = render_body_field(&node.items, "width")
            .ok_or_else(|| render_error("Render stroke requires width", node.span))?;
        let width = render_length(render_value(width_field)?, width_field.span)?;
        if width < 0.0 {
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
        let miter_limit = render_float(render_value(miter_field)?, miter_field.span)?;
        if miter_limit < 1.0 {
            return Err(render_error(
                "Render stroke miterLimit must be at least 1",
                miter_field.span,
            ));
        }
        let dash_offset = render_length(render_value(dash_offset_field)?, dash_offset_field.span)?;
        Ok(RenderStrokeSpec {
            id: self.stable_id(
                EntityKind::RenderStroke,
                format!("{node_path}/stroke"),
                stroke_field.span,
            )?,
            paint,
            width: self.descriptor(TypedValue::Length(width))?,
            cap: render_stroke_cap(render_value(cap_field)?, cap_field.span)?,
            join: render_stroke_join(render_value(join_field)?, join_field.span)?,
            miter_limit,
            dash_offset: self.descriptor(TypedValue::Length(dash_offset))?,
            dash: render_dash(render_value(dash_field)?, dash_field.span)?,
        })
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
            Ok::<_, Diagnostic>,
        )?)?;
        let origin_value =
            render_body_value_or(&node.items, "origin", zero_length_vec(), |value| {
                Ok::<_, Diagnostic>(value)
            })?;
        let origin = self.descriptor(origin_value)?;
        let rotation = self.descriptor(TypedValue::Angle(render_body_value_or(
            &node.items,
            "rotation",
            0.0,
            |value| render_angle(value, node.span),
        )?))?;
        let scale = self.descriptor(render_body_value_or(
            &node.items,
            "scale",
            one_float_vec(),
            Ok::<_, Diagnostic>,
        )?)?;
        let opacity_value = render_body_value_or(&node.items, "opacity", 1.0, |value| {
            render_float(value, node.span)
        })?;
        if !(0.0..=1.0).contains(&opacity_value) {
            return Err(render_error(
                "Render opacity must be within [0, 1]",
                render_body_field(&node.items, "opacity").map_or(node.span, |field| field.span),
            ));
        }
        let opacity = self.descriptor(TypedValue::Float(opacity_value))?;
        let visibility = self.descriptor(TypedValue::Bool(render_body_value_or(
            &node.items,
            "visibility",
            true,
            |value| render_bool(value, node.span),
        )?))?;
        let z_order = render_body_value_or(&node.items, "zOrder", 0, |value| {
            render_int(value, node.span)
        })?;
        let isolate = render_body_value_or(&node.items, "isolate", false, |value| {
            render_bool(value, node.span)
        })?;
        let follow_hidden_attachment =
            render_body_value_or(&node.items, "followHiddenAttachment", false, |value| {
                render_bool(value, node.span)
            })?;
        let composite = render_body_value_or(
            &node.items,
            "composite",
            CanonicalRenderComposite::SourceOver,
            |value| render_composite(value, node.span),
        )?;
        let active = render_active_interval(&node.items, self.time_map)?;
        let stroke = match node.kind {
            CanonicalRenderNodeKind::Line => Some(self.render_stroke(node_path, node)?),
            // Render section 14.2 lets a fillable geometry carry a fill paint, a stroke, or
            // both, so a Circle stroke is optional and only lowered when it is declared.
            CanonicalRenderNodeKind::Circle
            | CanonicalRenderNodeKind::Polyline
            | CanonicalRenderNodeKind::Polygon => render_body_field(&node.items, "stroke")
                .is_some()
                .then(|| self.render_stroke(node_path, node))
                .transpose()?,
            _ => None,
        };

        let (geometry_data, paint) = match node.kind {
            CanonicalRenderNodeKind::Group => (None, None),
            CanonicalRenderNodeKind::Rect => {
                let size_field = render_body_field(&node.items, "size")
                    .ok_or_else(|| render_error("Rect requires size", node.span))?;
                let size = render_vec2_length(render_value(size_field)?, size_field.span)?;
                if size.iter().any(|value| *value < 0.0) {
                    return Err(render_error(
                        "Rect size must be non-negative",
                        size_field.span,
                    ));
                }
                (
                    Some(CanonicalRenderGeometryData::Rect {
                        origin,
                        size: self.descriptor(
                            TypedValue::vec2(
                                TypedValue::Length(size[0]),
                                TypedValue::Length(size[1]),
                            )
                            .expect("homogeneous length vector"),
                        )?,
                    }),
                    Some(self.add_paint(node_path, node, "fill")?),
                )
            }
            CanonicalRenderNodeKind::RoundedRect => {
                let size_field = render_body_field(&node.items, "size")
                    .ok_or_else(|| render_error("RoundedRect requires size", node.span))?;
                let size = render_vec2_length(render_value(size_field)?, size_field.span)?;
                if size.iter().any(|value| *value < 0.0) {
                    return Err(render_error(
                        "RoundedRect size must be non-negative",
                        size_field.span,
                    ));
                }
                let radius_field = render_body_field(&node.items, "radius")
                    .ok_or_else(|| render_error("RoundedRect requires radius", node.span))?;
                let radius = render_length(render_value(radius_field)?, radius_field.span)?;
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
                (
                    Some(CanonicalRenderGeometryData::RoundedRect {
                        origin,
                        size,
                        radii: [radius; 4],
                    }),
                    Some(self.add_paint(node_path, node, "fill")?),
                )
            }
            CanonicalRenderNodeKind::Circle => {
                let center = self.descriptor(render_body_value_or(
                    &node.items,
                    "center",
                    zero_length_vec(),
                    Ok::<_, Diagnostic>,
                )?)?;
                let radius_field = render_body_field(&node.items, "radius")
                    .ok_or_else(|| render_error("Circle requires radius", node.span))?;
                let radius = render_length(render_value(radius_field)?, radius_field.span)?;
                if radius < 0.0 {
                    return Err(render_error(
                        "Circle radius must be non-negative",
                        radius_field.span,
                    ));
                }
                (
                    Some(CanonicalRenderGeometryData::Circle {
                        center,
                        radius: self.descriptor(TypedValue::Length(radius))?,
                    }),
                    // Render section 14.2 requires at least one of the two, so a declared
                    // stroke is what makes `fill` optional here.
                    if stroke.is_some() && render_body_field(&node.items, "fill").is_none() {
                        None
                    } else {
                        Some(self.add_paint(node_path, node, "fill")?)
                    },
                )
            }
            CanonicalRenderNodeKind::Ellipse => {
                let center = self.descriptor(render_body_value_or(
                    &node.items,
                    "center",
                    zero_length_vec(),
                    Ok::<_, Diagnostic>,
                )?)?;
                let radius_x_field = render_body_field(&node.items, "radiusX")
                    .ok_or_else(|| render_error("Ellipse requires radiusX", node.span))?;
                let radius_y_field = render_body_field(&node.items, "radiusY")
                    .ok_or_else(|| render_error("Ellipse requires radiusY", node.span))?;
                let radius_x = render_length(render_value(radius_x_field)?, radius_x_field.span)?;
                let radius_y = render_length(render_value(radius_y_field)?, radius_y_field.span)?;
                if radius_x < 0.0 || radius_y < 0.0 {
                    return Err(render_error(
                        "Ellipse radii must be non-negative",
                        node.span,
                    ));
                }
                (
                    Some(CanonicalRenderGeometryData::Ellipse {
                        center,
                        radius_x: self.descriptor(TypedValue::Length(radius_x))?,
                        radius_y: self.descriptor(TypedValue::Length(radius_y))?,
                        rotation,
                    }),
                    Some(self.add_paint(node_path, node, "fill")?),
                )
            }
            CanonicalRenderNodeKind::Line => {
                if render_body_field(&node.items, "fill").is_some() {
                    return Err(render_error("Line must not declare fill", node.span));
                }
                let start_field = render_body_field(&node.items, "start")
                    .ok_or_else(|| render_error("Line requires start", node.span))?;
                let end_field = render_body_field(&node.items, "end")
                    .ok_or_else(|| render_error("Line requires end", node.span))?;
                let start = render_vec2_length(render_value(start_field)?, start_field.span)?;
                let end = render_vec2_length(render_value(end_field)?, end_field.span)?;
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
                let points = render_value(points_field)?;
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
                        Some(self.add_paint(node_path, node, "fill")?)
                    },
                )
            }
            CanonicalRenderNodeKind::Polygon => {
                let points_field = render_body_field(&node.items, "points")
                    .ok_or_else(|| render_error("Polygon requires points", node.span))?;
                let points = render_value(points_field)?;
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
                (
                    Some(CanonicalRenderGeometryData::Polygon { points }),
                    // Render section 14.2 requires at least one of the two, so a declared
                    // stroke is what makes `fill` optional here.
                    if stroke.is_some() && render_body_field(&node.items, "fill").is_none() {
                        None
                    } else {
                        Some(self.add_paint(node_path, node, "fill")?)
                    },
                )
            }
            CanonicalRenderNodeKind::Image => {
                let resource_field = render_body_field(&node.items, "resource")
                    .ok_or_else(|| render_error("Image requires resource", node.span))?;
                let resource_name = match render_value(resource_field)? {
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
                let resource_id = if let Some(id) = self.resource_ids.get(&resource_name) {
                    id.clone()
                } else {
                    let id = self.stable_id(
                        EntityKind::Resource,
                        resource_name.clone(),
                        resource_field.span,
                    )?;
                    self.resource_ids.insert(resource_name.clone(), id.clone());
                    id
                };
                let sampling = match render_body_field(&node.items, "sampling") {
                    Some(field) => render_image_sampling(render_value(field)?, field.span)?,
                    None => match resource.metadata().get("sampling") {
                        Some(CanonicalValue::String(value)) => render_image_sampling(
                            TypedValue::String(value.clone()),
                            resource_field.span,
                        )?,
                        _ => {
                            return Err(render_error(
                                "image resource metadata lacks sampling",
                                resource_field.span,
                            ));
                        }
                    },
                };
                let destination_origin_field = render_body_field(&node.items, "destination.origin")
                    .ok_or_else(|| render_error("Image requires destination.origin", node.span))?;
                let destination_origin = render_vec2_length(
                    render_value(destination_origin_field)?,
                    destination_origin_field.span,
                )?;
                let destination_size_field = render_body_field(&node.items, "destination.size")
                    .ok_or_else(|| render_error("Image requires destination.size", node.span))?;
                let destination_size = render_vec2_length(
                    render_value(destination_size_field)?,
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
                        let source_origin =
                            render_vec2_float(render_value(origin_field)?, origin_field.span)?;
                        let source_size =
                            render_vec2_float(render_value(size_field)?, size_field.span)?;
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
                let id = self.stable_id(
                    EntityKind::RenderGeometry,
                    format!("{node_path}/geometry"),
                    node.span,
                )?;
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
            clip: None,
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
                    &format!("{node_path}/{}", child.name),
                )?;
            }
        }
        Ok(node_index)
    }
}

fn lower_render_scene(
    scene: &crate::ast::RenderScene,
    resources: &BTreeMap<String, CanonicalResource>,
    time_map: &ChartTimeMap,
    span: SourceSpan,
) -> Result<(CanonicalRenderScene, CanonicalDescriptorTable), Vec<Diagnostic>> {
    let result = (|| {
        let viewport_width = render_field(&scene.viewport.fields, "width")
            .ok_or_else(|| render_error("Render viewport requires width", scene.viewport.span))
            .and_then(|field| render_length(render_value(field)?, field.span))?;
        let viewport_height = render_field(&scene.viewport.fields, "height")
            .ok_or_else(|| render_error("Render viewport requires height", scene.viewport.span))
            .and_then(|field| render_length(render_value(field)?, field.span))?;
        let color_space = match render_value_or(
            &scene.viewport.fields,
            "colorSpace",
            "linear-srgb".to_owned(),
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
        let mut lowerer = RenderLowerer::new(resources, time_map, span);
        let mut layers = Vec::new();
        for (layer_index, layer) in scene.layers.iter().enumerate() {
            let pass = render_body_field(&layer.items, "pass")
                .ok_or_else(|| render_error("Render layer requires pass", layer.span))
                .and_then(|field| render_string(render_value(field)?, field.span))
                .and_then(|value| {
                    CanonicalRenderPass::from_spelling(&value).ok_or_else(|| {
                        render_error(format!("unsupported Render pass {value}"), layer.span)
                    })
                })?;
            let z_order = render_body_value_or(&layer.items, "zOrder", 0, |value| {
                render_int(value, layer.span)
            })?;
            let attachment =
                match render_body_value_or(&layer.items, "space", "world".to_owned(), |value| {
                    render_string(value, layer.span)
                })?
                .as_str()
                {
                    "world" => CanonicalRenderAttachment::World,
                    "screen" => CanonicalRenderAttachment::Screen,
                    other => {
                        return Err(render_error(
                            format!("unsupported Render space {other}"),
                            layer.span,
                        ));
                    }
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
                        &format!("layer/{}/{}", layer.name, node.name),
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
        let descriptor_values = lowerer.descriptor_values;
        let table = CanonicalDescriptorTable::new(lowerer.descriptors, descriptor_roots)
            .map_err(|error| render_error(format!("{error:?}"), span))?;
        let mapping = descriptor_values
            .iter()
            .map(|(ty, value)| {
                let expected = CanonicalPropertyDescriptor::new(
                    ty.clone(),
                    CanonicalDescriptorDomain::new(None, None, false).expect("unbounded domain"),
                    CanonicalDescriptorKind::Constant(value.clone()),
                )
                .expect("descriptor already validated");
                table
                    .descriptors()
                    .iter()
                    .position(|candidate| candidate == &expected)
                    .ok_or_else(|| render_error("descriptor interning lost a Render value", span))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut render = CanonicalRenderScene::new(CanonicalRenderSceneSpec {
            viewport: CanonicalViewport::new(viewport_width, viewport_height, color_space)
                .map_err(|error| render_error(format!("{error:?}"), span))?,
            layers,
            nodes: lowerer.nodes,
            geometries: lowerer.geometries,
            paths: Vec::new(),
            paints: lowerer.paints,
            strokes: lowerer.strokes,
            clips: Vec::new(),
            glyph_runs: Vec::new(),
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
