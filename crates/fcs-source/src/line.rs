//! I3.4 source-to-canonical lowering for Line values and parent topology.

use std::collections::{BTreeMap, BTreeSet};

use fcs_model::{
    Beat as CanonicalBeat, CanonicalLine, CanonicalLineBase, CanonicalLineGraph,
    CanonicalLineInherit, CanonicalScrollTempo, CanonicalScrollTempoMap, CanonicalScrollTempoPoint,
    CanonicalTextualId, CanonicalVec2, EntityKind, LineBaseError, LineGraphError, ScrollTempoError,
    ScrollTempoKey, StableId, StableIdRegistry,
};

use crate::ast::{
    Document, EntityField, LineBodyItem, LineDeclaration, SourceExpression, SourceLiteral,
    SourceSpan, TypedValue,
};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticStage};

impl Document {
    /// Lowers direct `lines` declarations into the immutable canonical Line graph.
    ///
    /// Track bodies are intentionally not consumed here. Their normalization is
    /// owned by I3.6; this boundary only validates Line-owned static fields,
    /// parent topology, and the declared scroll-tempo envelope.
    pub fn canonical_line_graph(&self) -> Result<CanonicalLineGraph, Vec<Diagnostic>> {
        lower_line_graph(self)
    }
}

#[derive(Debug)]
struct LoweredLine {
    id: StableId,
    parent_name: Option<(String, SourceSpan)>,
    base: CanonicalLineBase,
    inherit: CanonicalLineInherit,
    scroll_tempo: CanonicalScrollTempo,
    document_order: u64,
}

fn lower_line_graph(document: &Document) -> Result<CanonicalLineGraph, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut registry = StableIdRegistry::new();
    let mut ids_by_name = BTreeMap::<String, StableId>::new();
    let mut first_spans = BTreeMap::<String, SourceSpan>::new();
    let mut identities = Vec::new();

    for (document_order, declaration) in document.lines.iter().enumerate() {
        let textual = match CanonicalTextualId::explicit(declaration.name.clone()) {
            Ok(textual) => textual,
            Err(error) => {
                diagnostics.push(identity_diagnostic(
                    error.to_string(),
                    declaration.name_span,
                ));
                continue;
            }
        };
        match registry.insert(EntityKind::Line, textual) {
            Ok(id) => {
                if let Some(previous_span) =
                    first_spans.insert(declaration.name.clone(), declaration.name_span)
                {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Canonical,
                            format!("Line ID {} is declared more than once", declaration.name),
                            declaration.name_span,
                        )
                        .with_label(DiagnosticLabel::new(
                            previous_span,
                            "first Line declaration",
                        )),
                    );
                } else {
                    ids_by_name.insert(declaration.name.clone(), id.clone());
                    identities.push((document_order as u64, declaration, id));
                }
            }
            Err(error) => diagnostics.push(identity_diagnostic(
                error.to_string(),
                declaration.name_span,
            )),
        }
    }

    let mut lowered = Vec::new();
    for (document_order, declaration, id) in identities {
        if let Some(line) = lower_line(
            declaration,
            id,
            document_order,
            document.definitions.as_ref(),
            &mut diagnostics,
        ) {
            lowered.push(line);
        }
    }

    let mut resolved = Vec::new();
    for line in lowered {
        let parent = match line.parent_name.as_ref() {
            Some((name, span)) => match ids_by_name.get(name) {
                Some(id) => Some(id.clone()),
                None => {
                    diagnostics.push(Diagnostic::new(
                        DiagnosticCode::GRAPH_UNKNOWN_PARENT,
                        DiagnosticStage::Canonical,
                        format!("Line parent {name} does not name a declared Line"),
                        *span,
                    ));
                    None
                }
            },
            None => None,
        };
        match CanonicalLine::new(
            line.id,
            parent,
            line.document_order,
            line.base,
            line.inherit,
            line.scroll_tempo,
        ) {
            Ok(line) => resolved.push(line),
            Err(error) => diagnostics.push(graph_diagnostic(error, SourceSpan::new(0, 0))),
        }
    }

    if diagnostics.is_empty() {
        match CanonicalLineGraph::new(resolved) {
            Ok(graph) => Ok(graph),
            Err(error) => Err(vec![graph_diagnostic(error, SourceSpan::new(0, 0))]),
        }
    } else {
        sort_diagnostics(&mut diagnostics);
        Err(diagnostics)
    }
}

fn lower_line(
    declaration: &LineDeclaration,
    id: StableId,
    document_order: u64,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<LoweredLine> {
    let mut fields = BTreeMap::<String, &EntityField>::new();
    let mut scroll_map = None;
    for item in &declaration.items {
        match item {
            LineBodyItem::Field(field) => {
                let path = field.path.segments.join(".");
                if let Some(previous) = fields.insert(path.clone(), field) {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                            DiagnosticStage::Canonical,
                            format!("Line field {path} is declared more than once"),
                            field.span,
                        )
                        .with_label(DiagnosticLabel::new(
                            previous.span,
                            "first field declaration",
                        )),
                    );
                }
            }
            LineBodyItem::ScrollTempoMap(map) => {
                if let Some(previous) = scroll_map.replace(map) {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::SCHEMA_DUPLICATE_FIELD,
                            DiagnosticStage::Canonical,
                            "scrollTempoMap is declared more than once",
                            map.keyword_span,
                        )
                        .with_label(DiagnosticLabel::new(
                            previous.keyword_span,
                            "first scrollTempoMap declaration",
                        )),
                    );
                }
            }
            LineBodyItem::Tracks(_) => {}
        }
    }

    let known = [
        "parent",
        "position",
        "rotation",
        "scale",
        "alpha",
        "transformOrigin",
        "textureAnchor",
        "floorScale",
        "integrationOrigin",
        "initialFloorPosition",
        "allowReverseScroll",
        "zOrder",
        "inherit.position",
        "inherit.rotation",
        "inherit.scale",
        "inherit.alpha",
        "inherit.scroll",
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    for (path, field) in &fields {
        if !known.contains(path.as_str()) {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::SCHEMA_UNKNOWN_FIELD,
                DiagnosticStage::Canonical,
                format!("unknown Line field {path}"),
                field.path.span,
            ));
        }
    }

    let parent_name = fields
        .get("parent")
        .and_then(|field| lower_parent(field, definitions, diagnostics));
    let position = field_or_default(
        &fields,
        "position",
        definitions,
        diagnostics,
        vec2_length(0.0, 0.0),
    );
    let rotation = field_or_default(
        &fields,
        "rotation",
        definitions,
        diagnostics,
        TypedValue::Angle(0.0),
    );
    let scale = field_or_default(
        &fields,
        "scale",
        definitions,
        diagnostics,
        vec2_float(1.0, 1.0),
    );
    let alpha = field_or_default(
        &fields,
        "alpha",
        definitions,
        diagnostics,
        TypedValue::Float(1.0),
    );
    let transform_origin = field_or_default(
        &fields,
        "transformOrigin",
        definitions,
        diagnostics,
        vec2_length(0.0, 0.0),
    );
    let texture_anchor = field_or_default(
        &fields,
        "textureAnchor",
        definitions,
        diagnostics,
        vec2_float(0.5, 0.5),
    );
    let floor_scale = field_or_default(
        &fields,
        "floorScale",
        definitions,
        diagnostics,
        TypedValue::Length(120.0),
    );
    let integration_origin = field_or_default(
        &fields,
        "integrationOrigin",
        definitions,
        diagnostics,
        TypedValue::Time(0.0),
    );
    let initial_floor_position = field_or_default(
        &fields,
        "initialFloorPosition",
        definitions,
        diagnostics,
        TypedValue::Float(0.0),
    );
    let allow_reverse_scroll = field_or_default(
        &fields,
        "allowReverseScroll",
        definitions,
        diagnostics,
        TypedValue::Bool(false),
    );
    let z_order = field_or_default(
        &fields,
        "zOrder",
        definitions,
        diagnostics,
        TypedValue::Int(0),
    );

    let mut inherit = CanonicalLineInherit::default();
    for (path, slot) in [
        ("inherit.position", 0_u8),
        ("inherit.rotation", 1),
        ("inherit.scale", 2),
        ("inherit.alpha", 3),
        ("inherit.scroll", 4),
    ] {
        if let Some(field) = fields.get(path)
            && let Some(value) = evaluate_field(field, definitions, diagnostics)
        {
            match value {
                TypedValue::Bool(value) => {
                    let current = inherit;
                    inherit = CanonicalLineInherit::new(
                        if slot == 0 { value } else { current.position() },
                        if slot == 1 { value } else { current.rotation() },
                        if slot == 2 { value } else { current.scale() },
                        if slot == 3 { value } else { current.alpha() },
                        if slot == 4 { value } else { current.scroll() },
                    );
                }
                other => type_mismatch(path, "bool", &other, field.span, diagnostics),
            }
        }
    }

    let base = match lower_base(
        position,
        rotation,
        scale,
        alpha,
        transform_origin,
        texture_anchor,
        floor_scale,
        integration_origin,
        initial_floor_position,
        allow_reverse_scroll,
        z_order,
    ) {
        Ok(base) => base,
        Err((code, message, span)) => {
            diagnostics.push(Diagnostic::new(
                code,
                DiagnosticStage::Canonical,
                message,
                span,
            ));
            CanonicalLineBase::identity()
        }
    };

    let scroll_tempo = scroll_map.and_then(|map| {
        lower_scroll_tempo_map(map, definitions, diagnostics).map(CanonicalScrollTempo::Override)
    });

    Some(LoweredLine {
        id,
        parent_name,
        base,
        inherit,
        scroll_tempo: scroll_tempo.unwrap_or(CanonicalScrollTempo::Global),
        document_order,
    })
}

fn lower_parent(
    field: &EntityField,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(String, SourceSpan)> {
    if matches!(
        field.value,
        SourceExpression::Literal {
            literal: SourceLiteral::Null,
            ..
        }
    ) {
        return None;
    }
    let value = evaluate_field(field, definitions, diagnostics)?;
    match value {
        TypedValue::Line(name) => Some((name, field.value.span())),
        other => {
            type_mismatch("parent", "Line or null", &other, field.span, diagnostics);
            None
        }
    }
}

fn field_or_default(
    fields: &BTreeMap<String, &EntityField>,
    path: &str,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
    default: TypedValue,
) -> TypedValue {
    fields
        .get(path)
        .and_then(|field| evaluate_field(field, definitions, diagnostics))
        .unwrap_or(default)
}

fn evaluate_field(
    field: &EntityField,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<TypedValue> {
    match crate::elaborator::evaluate_metadata_expression(&field.value, definitions) {
        Ok(value) => Some(value),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_base(
    position: TypedValue,
    rotation: TypedValue,
    scale: TypedValue,
    alpha: TypedValue,
    transform_origin: TypedValue,
    texture_anchor: TypedValue,
    floor_scale: TypedValue,
    integration_origin: TypedValue,
    initial_floor_position: TypedValue,
    allow_reverse_scroll: TypedValue,
    z_order: TypedValue,
) -> Result<CanonicalLineBase, (DiagnosticCode, String, SourceSpan)> {
    let position = vec2_of(position, "position", "vec2<length>")?;
    let rotation = angle_of(rotation, "rotation")?;
    let scale = vec2_of(scale, "scale", "vec2<float>")?;
    let alpha = float_of(alpha, "alpha")?;
    let transform_origin = vec2_of(transform_origin, "transformOrigin", "vec2<length>")?;
    let texture_anchor = vec2_of(texture_anchor, "textureAnchor", "vec2<float>")?;
    let floor_scale = length_of(floor_scale, "floorScale")?;
    let integration_origin = time_of(integration_origin, "integrationOrigin")?;
    let initial_floor_position = float_of(initial_floor_position, "initialFloorPosition")?;
    let allow_reverse_scroll = bool_of(allow_reverse_scroll, "allowReverseScroll")?;
    let z_order = int_of(z_order, "zOrder")?.try_into().map_err(|_| {
        (
            DiagnosticCode::NUMERIC_DOMAIN,
            "zOrder must fit a signed 32-bit integer".into(),
            SourceSpan::new(0, 0),
        )
    })?;
    CanonicalLineBase::new(
        position,
        rotation,
        scale,
        alpha,
        transform_origin,
        texture_anchor,
        floor_scale,
        integration_origin,
        initial_floor_position,
        allow_reverse_scroll,
        z_order,
    )
    .map_err(base_error)
}

fn lower_scroll_tempo_map(
    map: &crate::ast::ScrollTempoMap,
    definitions: Option<&crate::ast::DefinitionsBlock>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<CanonicalScrollTempoMap> {
    let mut points = Vec::new();
    for point in &map.points {
        let key = match crate::elaborator::evaluate_metadata_expression(&point.key, definitions) {
            Ok(TypedValue::Beat(value)) => {
                CanonicalBeat::new(value.numerator(), value.denominator())
                    .ok()
                    .map(ScrollTempoKey::Beat)
            }
            Ok(TypedValue::Time(value)) => Some(ScrollTempoKey::Time(value)),
            Ok(value) => {
                type_mismatch(
                    "scrollTempoMap key",
                    "beat or time",
                    &value,
                    point.key.span(),
                    diagnostics,
                );
                None
            }
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                None
            }
        };
        let Some(key) = key else { continue };
        match CanonicalScrollTempoPoint::new(key, point.bpm.get()) {
            Ok(point) => points.push(point),
            Err(error) => diagnostics.push(scroll_diagnostic(error, point.span)),
        }
    }
    match CanonicalScrollTempoMap::new(points) {
        Ok(map) => Some(map),
        Err(error) => {
            diagnostics.push(scroll_diagnostic(error, map.span));
            None
        }
    }
}

fn vec2_length(x: f64, y: f64) -> TypedValue {
    TypedValue::vec2(TypedValue::Length(x), TypedValue::Length(y)).expect("homogeneous length vec2")
}

fn vec2_float(x: f64, y: f64) -> TypedValue {
    TypedValue::vec2(TypedValue::Float(x), TypedValue::Float(y)).expect("homogeneous float vec2")
}

fn vec2_of(
    value: TypedValue,
    field: &'static str,
    expected: &'static str,
) -> Result<CanonicalVec2, (DiagnosticCode, String, SourceSpan)> {
    let TypedValue::Vec2(x, y) = value else {
        return Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type {expected}"),
            SourceSpan::new(0, 0),
        ));
    };
    let (x, y) = match expected {
        "vec2<length>" => match (*x, *y) {
            (TypedValue::Length(x), TypedValue::Length(y)) => (x, y),
            _ => {
                return Err((
                    DiagnosticCode::TYPE_MISMATCH,
                    format!("{field} must have type {expected}"),
                    SourceSpan::new(0, 0),
                ));
            }
        },
        "vec2<float>" => match (*x, *y) {
            (TypedValue::Float(x), TypedValue::Float(y)) => (x, y),
            _ => {
                return Err((
                    DiagnosticCode::TYPE_MISMATCH,
                    format!("{field} must have type {expected}"),
                    SourceSpan::new(0, 0),
                ));
            }
        },
        _ => unreachable!("unknown Line vec2 type"),
    };
    CanonicalVec2::new(x, y).map_err(base_error)
}

fn float_of(
    value: TypedValue,
    field: &'static str,
) -> Result<f64, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Float(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type float"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn length_of(
    value: TypedValue,
    field: &'static str,
) -> Result<f64, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Length(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type length"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn angle_of(
    value: TypedValue,
    field: &'static str,
) -> Result<f64, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Angle(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type angle"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn time_of(
    value: TypedValue,
    field: &'static str,
) -> Result<f64, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Time(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type time"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn bool_of(
    value: TypedValue,
    field: &'static str,
) -> Result<bool, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Bool(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type bool"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn int_of(
    value: TypedValue,
    field: &'static str,
) -> Result<i64, (DiagnosticCode, String, SourceSpan)> {
    match value {
        TypedValue::Int(value) => Ok(value),
        _ => Err((
            DiagnosticCode::TYPE_MISMATCH,
            format!("{field} must have type int"),
            SourceSpan::new(0, 0),
        )),
    }
}

fn type_mismatch(
    field: &str,
    expected: &str,
    value: &TypedValue,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    diagnostics.push(Diagnostic::new(
        DiagnosticCode::TYPE_MISMATCH,
        DiagnosticStage::Canonical,
        format!("{field} must have type {expected}, found {}", value.ty()),
        span,
    ));
}

fn base_error(error: LineBaseError) -> (DiagnosticCode, String, SourceSpan) {
    match error {
        LineBaseError::NonFinite { field } => (
            DiagnosticCode::NUMERIC_NON_FINITE,
            format!("Line field {field} must be finite"),
            SourceSpan::new(0, 0),
        ),
        LineBaseError::OutOfRange { field } => (
            DiagnosticCode::NUMERIC_DOMAIN,
            format!("Line field {field} is outside its allowed range"),
            SourceSpan::new(0, 0),
        ),
    }
}

fn scroll_diagnostic(error: ScrollTempoError, span: SourceSpan) -> Diagnostic {
    let code = match error {
        ScrollTempoError::NonMonotonic => DiagnosticCode::TEMPO_NON_MONOTONIC,
        ScrollTempoError::Empty
        | ScrollTempoError::FirstKeyNotZero
        | ScrollTempoError::MixedKeyDomain
        | ScrollTempoError::InvalidBpm
        | ScrollTempoError::NonFiniteKey => DiagnosticCode::TEMPO_INVALID,
    };
    Diagnostic::new(code, DiagnosticStage::Canonical, error.to_string(), span)
}

fn graph_diagnostic(error: LineGraphError, span: SourceSpan) -> Diagnostic {
    let (code, message) = match error {
        LineGraphError::WrongNamespace { id } => (
            DiagnosticCode::TYPE_INVALID_OPERATION,
            format!("stable ID {id} is not a Line ID"),
        ),
        LineGraphError::DuplicateId { id } => (
            DiagnosticCode::NAME_DUPLICATE,
            format!("canonical Line ID {id} is declared more than once"),
        ),
        LineGraphError::UnknownParent { line, parent } => (
            DiagnosticCode::GRAPH_UNKNOWN_PARENT,
            format!("Line {line} refers to unknown parent {parent}"),
        ),
        LineGraphError::SelfParent { line } => (
            DiagnosticCode::GRAPH_CYCLE,
            format!("Line {line} cannot parent itself"),
        ),
        LineGraphError::Cycle { lines } => (
            DiagnosticCode::GRAPH_CYCLE,
            format!("Line parent graph contains a cycle: {lines:?}"),
        ),
    };
    Diagnostic::new(code, DiagnosticStage::Canonical, message, span)
}

fn identity_diagnostic(message: String, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::NAME_DUPLICATE,
        DiagnosticStage::Canonical,
        message,
        span,
    )
}

fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        left.primary_span()
            .start
            .cmp(&right.primary_span().start)
            .then_with(|| left.primary_span().end.cmp(&right.primary_span().end))
            .then_with(|| left.code().cmp(&right.code()))
    });
}
