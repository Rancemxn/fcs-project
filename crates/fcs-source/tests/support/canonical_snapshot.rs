//! Test-only, human-readable projection of every current `CanonicalChart` field.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use fcs_model::{
    Beat, CanonicalChart, CanonicalColor, CanonicalCredit, CanonicalJudgeShape, CanonicalLine,
    CanonicalMetadata, CanonicalNote, CanonicalNoteKind, CanonicalNoteScorePolicy,
    CanonicalNoteSide, CanonicalNoteSoundPolicy, CanonicalObject, CanonicalProfile,
    CanonicalProfileFeature, CanonicalResource, CanonicalResourceKind, CanonicalScrollLine,
    CanonicalScrollTempo, CanonicalTime, CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill,
    CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackTarget, CanonicalTrackValue,
    CanonicalValue, CanonicalValueType, CanonicalVec2, EntityKind, ScrollTempoDomain,
    ScrollTempoKey, StableId,
};
use serde_json::{Map, Number, Value};

pub fn canonical_snapshot(chart: &CanonicalChart) -> String {
    let mut output = serde_json::to_string_pretty(&chart_value(chart))
        .expect("canonical snapshot projection must serialize");
    output.push('\n');
    output
}

pub fn chart_value(chart: &CanonicalChart) -> Value {
    object([
        ("sourceVersion", string(chart.source_version().as_str())),
        ("profile", string(profile(chart.profile()))),
        (
            "features",
            array(
                chart
                    .features()
                    .iter()
                    .copied()
                    .map(|feature| string(profile_feature(feature))),
            ),
        ),
        (
            "tempoMap",
            array(chart.time_map().segments().map(|(beat, chart_time, bpm)| {
                object([
                    ("beat", beat_value(beat)),
                    ("chartTimeSeconds", float(chart_time)),
                    ("bpm", float(bpm)),
                ])
            })),
        ),
        ("metadata", metadata_value(chart.metadata())),
        (
            "lines",
            object([
                ("values", array(chart.lines().lines().map(line_value))),
                (
                    "topologicalOrder",
                    array(
                        chart
                            .lines()
                            .topological_order()
                            .iter()
                            .map(stable_id_value),
                    ),
                ),
            ]),
        ),
        ("notes", array(chart.notes().notes().iter().map(note_value))),
        (
            "tracks",
            array(chart.tracks().tracks().iter().map(track_value)),
        ),
        (
            "scroll",
            array(chart.scroll().lines().iter().map(scroll_line_value)),
        ),
        (
            "requiredExtensions",
            array(chart.required_extensions().iter().map(|extension| {
                object([
                    ("namespace", string(extension.namespace())),
                    ("version", string(extension.version())),
                ])
            })),
        ),
    ])
}

fn profile(value: CanonicalProfile) -> &'static str {
    match value {
        CanonicalProfile::Fragment => "fragment",
        CanonicalProfile::Chart => "chart",
        CanonicalProfile::Playable => "playable",
        CanonicalProfile::Renderable => "renderable",
        CanonicalProfile::Publishable => "publishable",
    }
}

fn profile_feature(value: CanonicalProfileFeature) -> &'static str {
    match value {
        CanonicalProfileFeature::Playable => "playable",
        CanonicalProfileFeature::Renderable => "renderable",
    }
}

fn metadata_value(metadata: &CanonicalMetadata) -> Value {
    object([
        ("meta", optional(metadata.meta().map(canonical_value_map))),
        (
            "contributors",
            array(metadata.contributors().values().map(|contributor| {
                object([
                    ("id", string(contributor.id())),
                    ("name", string(contributor.name())),
                    ("aliases", array(contributor.aliases().iter().map(string))),
                    (
                        "identifiers",
                        canonical_object_entries(contributor.identifiers()),
                    ),
                ])
            })),
        ),
        (
            "credits",
            array(metadata.credits().iter().map(credit_value)),
        ),
        (
            "resources",
            array(metadata.resources().values().map(resource_value)),
        ),
        (
            "artwork",
            optional(
                metadata
                    .artwork()
                    .map(|artwork| object([("primary", optional(artwork.primary().map(string)))])),
            ),
        ),
        (
            "sync",
            optional(metadata.sync().map(|sync| {
                object([
                    ("primaryAudio", optional(sync.primary_audio().map(string))),
                    ("audioOffsetSeconds", float(sync.audio_offset().seconds())),
                    (
                        "preview",
                        optional(sync.preview().map(|preview| {
                            object([
                                ("startSeconds", float(preview.start_seconds())),
                                ("endSeconds", float(preview.end_seconds())),
                            ])
                        })),
                    ),
                ])
            })),
        ),
    ])
}

fn credit_value(credit: &CanonicalCredit) -> Value {
    object([
        ("role", string(credit.role())),
        ("label", optional(credit.label().map(string))),
        (
            "contributors",
            array(
                credit
                    .contributors()
                    .iter()
                    .map(|contributor| string(contributor.as_str())),
            ),
        ),
    ])
}

fn resource_value(resource: &CanonicalResource) -> Value {
    object([
        ("id", string(resource.id())),
        ("kind", string(resource_kind(resource.kind()))),
        ("mediaType", string(resource.media_type())),
        (
            "declaredSha256",
            optional(resource.declared_sha256().map(|hash| {
                let mut hex = String::with_capacity(64);
                for byte in hash.as_bytes() {
                    write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                }
                string(hex)
            })),
        ),
        ("metadata", canonical_value_map(resource.metadata())),
    ])
}

fn resource_kind(value: CanonicalResourceKind) -> &'static str {
    match value {
        CanonicalResourceKind::Audio => "audio",
        CanonicalResourceKind::Image => "image",
        CanonicalResourceKind::Font => "font",
        CanonicalResourceKind::Texture => "texture",
        CanonicalResourceKind::Path => "path",
        CanonicalResourceKind::Shader => "shader",
        CanonicalResourceKind::Binary => "binary",
    }
}

fn canonical_value_map(values: &BTreeMap<String, CanonicalValue>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), canonical_value(value)))
            .collect(),
    )
}

fn canonical_value(value: &CanonicalValue) -> Value {
    match value {
        CanonicalValue::Null => object([("type", string("null")), ("value", Value::Null)]),
        CanonicalValue::Bool(value) => {
            object([("type", string("bool")), ("value", Value::Bool(*value))])
        }
        CanonicalValue::Int(value) => object([("type", string("int")), ("value", integer(*value))]),
        CanonicalValue::Float(value) => {
            object([("type", string("float")), ("value", float(*value))])
        }
        CanonicalValue::String(value) => {
            object([("type", string("string")), ("value", string(value))])
        }
        CanonicalValue::Time(value) => object([
            ("type", string("time")),
            ("value", object([("chartTimeSeconds", float(*value))])),
        ]),
        CanonicalValue::Beat(value) => {
            object([("type", string("beat")), ("value", beat_value(*value))])
        }
        CanonicalValue::Color(value) => {
            object([("type", string("color")), ("value", color_value(*value))])
        }
        CanonicalValue::ResourceReference(value) => object([
            ("type", string("resourceReference")),
            ("value", string(value)),
        ]),
        CanonicalValue::ContributorReference(value) => object([
            ("type", string("contributorReference")),
            ("value", string(value)),
        ]),
        CanonicalValue::Array {
            element_type,
            values,
        } => object([
            ("type", string("array")),
            ("elementType", canonical_value_type(element_type)),
            ("values", array(values.iter().map(canonical_value))),
        ]),
        CanonicalValue::Object(value) => object([
            ("type", string("object")),
            ("entries", canonical_object_entries(value)),
        ]),
    }
}

fn canonical_value_type(value: &CanonicalValueType) -> Value {
    match value {
        CanonicalValueType::Null => string("null"),
        CanonicalValueType::Bool => string("bool"),
        CanonicalValueType::Int => string("int"),
        CanonicalValueType::Float => string("float"),
        CanonicalValueType::String => string("string"),
        CanonicalValueType::Time => string("time"),
        CanonicalValueType::Beat => string("beat"),
        CanonicalValueType::Color => string("color"),
        CanonicalValueType::ResourceReference => string("resourceReference"),
        CanonicalValueType::ContributorReference => string("contributorReference"),
        CanonicalValueType::Array(element_type) => object([
            ("type", string("array")),
            ("elementType", canonical_value_type(element_type)),
        ]),
        CanonicalValueType::Object => string("object"),
    }
}

fn canonical_object_entries(value: &CanonicalObject) -> Value {
    array(value.entries().iter().map(|entry| {
        object([
            ("key", string(entry.key())),
            ("value", canonical_value(entry.value())),
        ])
    }))
}

fn line_value(line: &CanonicalLine) -> Value {
    let base = line.base();
    let inherit = line.inherit();
    object([
        ("id", stable_id_value(line.id())),
        ("parent", optional(line.parent().map(stable_id_value))),
        ("documentOrder", unsigned(line.document_order())),
        (
            "base",
            object([
                ("position", vec2_value(base.position())),
                ("rotation", float(base.rotation())),
                ("scale", vec2_value(base.scale())),
                ("alpha", float(base.alpha())),
                ("transformOrigin", vec2_value(base.transform_origin())),
                ("textureAnchor", vec2_value(base.texture_anchor())),
                ("floorScale", float(base.floor_scale())),
                ("integrationOrigin", float(base.integration_origin())),
                ("initialFloorPosition", float(base.initial_floor_position())),
                (
                    "allowReverseScroll",
                    Value::Bool(base.allow_reverse_scroll()),
                ),
                ("zOrder", integer(i64::from(base.z_order()))),
            ]),
        ),
        (
            "inherit",
            object([
                ("position", Value::Bool(inherit.position())),
                ("rotation", Value::Bool(inherit.rotation())),
                ("scale", Value::Bool(inherit.scale())),
                ("alpha", Value::Bool(inherit.alpha())),
                ("scroll", Value::Bool(inherit.scroll())),
            ]),
        ),
        ("scrollTempo", scroll_tempo_value(line.scroll_tempo())),
    ])
}

fn scroll_tempo_value(value: &CanonicalScrollTempo) -> Value {
    match value {
        CanonicalScrollTempo::Global => object([("kind", string("global"))]),
        CanonicalScrollTempo::Override(map) => object([
            ("kind", string("override")),
            (
                "domain",
                string(match map.domain() {
                    ScrollTempoDomain::Beat => "beat",
                    ScrollTempoDomain::Time => "time",
                }),
            ),
            (
                "points",
                array(map.points().iter().map(|point| {
                    object([
                        (
                            "key",
                            match point.key() {
                                ScrollTempoKey::Beat(beat) => object([
                                    ("domain", string("beat")),
                                    ("value", beat_value(beat)),
                                ]),
                                ScrollTempoKey::Time(time) => {
                                    object([("domain", string("time")), ("value", float(time))])
                                }
                            },
                        ),
                        ("bpm", float(point.bpm())),
                    ])
                })),
            ),
        ]),
    }
}

fn note_value(note: &CanonicalNote) -> Value {
    let gameplay = note.gameplay();
    let presentation = note.presentation();
    object([
        ("id", stable_id_value(note.id())),
        ("kind", string(note_kind(note.kind()))),
        ("documentOrder", unsigned(note.document_order())),
        (
            "gameplay",
            object([
                ("kind", string(note_kind(gameplay.kind()))),
                ("line", stable_id_value(gameplay.line())),
                ("time", canonical_time_value(gameplay.time())),
                (
                    "endTime",
                    optional(gameplay.end_time().map(canonical_time_value)),
                ),
                ("side", string(note_side(gameplay.side()))),
                ("judgmentEnabled", Value::Bool(gameplay.judgment_enabled())),
                ("judgeShape", judge_shape_value(gameplay.judge_shape())),
                ("soundPolicy", sound_policy_value(gameplay.sound_policy())),
                ("scorePolicy", score_policy_value(gameplay.score_policy())),
            ]),
        ),
        (
            "presentation",
            object([
                ("positionX", float(presentation.position_x())),
                ("scrollFactor", float(presentation.scroll_factor())),
                ("xOffset", float(presentation.x_offset())),
                ("yOffset", float(presentation.y_offset())),
                ("alpha", float(presentation.alpha())),
                ("scaleX", float(presentation.scale_x())),
                ("scaleY", float(presentation.scale_y())),
                ("rotation", float(presentation.rotation())),
                ("color", color_value(presentation.color())),
                ("texture", optional(presentation.texture().map(string))),
                ("renderEnabled", Value::Bool(presentation.render_enabled())),
                (
                    "visibleFrom",
                    optional(presentation.visible_from().map(canonical_time_value)),
                ),
                (
                    "visibleUntil",
                    optional(presentation.visible_until().map(canonical_time_value)),
                ),
            ]),
        ),
    ])
}

fn note_kind(value: CanonicalNoteKind) -> &'static str {
    match value {
        CanonicalNoteKind::Tap => "tap",
        CanonicalNoteKind::Hold => "hold",
        CanonicalNoteKind::Flick => "flick",
        CanonicalNoteKind::Drag => "drag",
    }
}

fn note_side(value: CanonicalNoteSide) -> &'static str {
    match value {
        CanonicalNoteSide::Above => "above",
        CanonicalNoteSide::Below => "below",
    }
}

fn judge_shape_value(value: &CanonicalJudgeShape) -> Value {
    match value {
        CanonicalJudgeShape::LineDefault => object([("kind", string("lineDefault"))]),
        CanonicalJudgeShape::Rectangle {
            center,
            half_extents,
        } => object([
            ("kind", string("rectangle")),
            ("center", vec2_value(*center)),
            ("halfExtents", vec2_value(*half_extents)),
        ]),
        CanonicalJudgeShape::Circle { center, radius } => object([
            ("kind", string("circle")),
            ("center", vec2_value(*center)),
            ("radius", float(*radius)),
        ]),
    }
}

fn sound_policy_value(value: &CanonicalNoteSoundPolicy) -> Value {
    match value {
        CanonicalNoteSoundPolicy::Default => object([("kind", string("default"))]),
        CanonicalNoteSoundPolicy::None => object([("kind", string("none"))]),
        CanonicalNoteSoundPolicy::Resource(resource) => {
            object([("kind", string("resource")), ("resource", string(resource))])
        }
    }
}

fn score_policy_value(value: &CanonicalNoteScorePolicy) -> Value {
    match value {
        CanonicalNoteScorePolicy::Default => object([("kind", string("default"))]),
        CanonicalNoteScorePolicy::None => object([("kind", string("none"))]),
        CanonicalNoteScorePolicy::Custom(name) => {
            object([("kind", string("custom")), ("name", string(name))])
        }
    }
}

fn track_value(track: &CanonicalTrack) -> Value {
    object([
        ("owner", stable_id_value(track.owner())),
        ("name", string(track.name())),
        ("target", string(track_target(track.target()))),
        ("blend", string(track_blend(track.blend()))),
        ("priority", integer(track.priority())),
        ("fill", string(track_fill(track.fill()))),
        (
            "extrapolateBefore",
            string(track_fill(track.extrapolate_before())),
        ),
        (
            "extrapolateAfter",
            string(track_fill(track.extrapolate_after())),
        ),
        (
            "pieces",
            array(track.pieces().iter().map(track_piece_value)),
        ),
    ])
}

fn track_target(value: CanonicalTrackTarget) -> &'static str {
    match value {
        CanonicalTrackTarget::Position => "position",
        CanonicalTrackTarget::Rotation => "rotation",
        CanonicalTrackTarget::Scale => "scale",
        CanonicalTrackTarget::Alpha => "alpha",
        CanonicalTrackTarget::ScrollSpeed => "scrollSpeed",
    }
}

fn track_blend(value: CanonicalTrackBlend) -> &'static str {
    match value {
        CanonicalTrackBlend::Replace => "replace",
        CanonicalTrackBlend::Add => "add",
        CanonicalTrackBlend::Multiply => "multiply",
    }
}

fn track_fill(value: CanonicalTrackFill) -> &'static str {
    match value {
        CanonicalTrackFill::Base => "base",
        CanonicalTrackFill::Zero => "zero",
        CanonicalTrackFill::One => "one",
        CanonicalTrackFill::HoldBefore => "holdBefore",
        CanonicalTrackFill::HoldAfter => "holdAfter",
        CanonicalTrackFill::Error => "error",
    }
}

fn track_piece_value(value: &CanonicalTrackPiece) -> Value {
    match value {
        CanonicalTrackPiece::Segment(segment) => object([
            ("kind", string("segment")),
            ("start", canonical_time_value(segment.start())),
            ("end", canonical_time_value(segment.end())),
            ("startValue", track_typed_value(segment.start_value())),
            ("endValue", track_typed_value(segment.end_value())),
            (
                "interpolation",
                track_interpolation_value(segment.interpolation()),
            ),
            ("documentOrder", unsigned(segment.document_order())),
        ]),
        CanonicalTrackPiece::Point(point) => object([
            ("kind", string("point")),
            ("time", canonical_time_value(point.time())),
            ("value", track_typed_value(point.value())),
            ("documentOrder", unsigned(point.document_order())),
        ]),
    }
}

fn track_typed_value(value: CanonicalTrackValue) -> Value {
    match value {
        CanonicalTrackValue::Float(value) => {
            object([("type", string("float")), ("value", float(value))])
        }
        CanonicalTrackValue::Angle(value) => {
            object([("type", string("angle")), ("value", float(value))])
        }
        CanonicalTrackValue::Vec2Float(value) => {
            object([("type", string("vec2Float")), ("value", vec2_value(value))])
        }
        CanonicalTrackValue::Vec2Length(value) => {
            object([("type", string("vec2Length")), ("value", vec2_value(value))])
        }
    }
}

fn track_interpolation_value(value: &CanonicalTrackInterpolation) -> Value {
    match value {
        CanonicalTrackInterpolation::Step => object([("kind", string("step"))]),
        CanonicalTrackInterpolation::Linear => object([("kind", string("linear"))]),
        CanonicalTrackInterpolation::Easing(name) => {
            object([("kind", string("easing")), ("name", string(name))])
        }
        CanonicalTrackInterpolation::CubicBezier(control) => object([
            ("kind", string("cubicBezier")),
            ("x1", float(control[0])),
            ("y1", float(control[1])),
            ("x2", float(control[2])),
            ("y2", float(control[3])),
        ]),
    }
}

fn scroll_line_value(line: &CanonicalScrollLine) -> Value {
    object([
        ("line", stable_id_value(line.line_id())),
        (
            "coordinate",
            object([(
                "points",
                array(line.coordinate().points().iter().map(|point| {
                    object([
                        ("chartTimeSeconds", float(point.chart_time())),
                        ("bpm", float(point.bpm())),
                    ])
                })),
            )]),
        ),
        ("speed", float(line.speed())),
        (
            "allowReverseScroll",
            Value::Bool(line.allow_reverse_scroll()),
        ),
        ("floorScale", float(line.floor_scale())),
        ("integrationOrigin", float(line.integration_origin())),
        ("initialFloorPosition", float(line.initial_floor_position())),
    ])
}

fn stable_id_value(id: &StableId) -> Value {
    object([
        ("namespace", string(entity_namespace(id.namespace()))),
        ("textual", string(id.textual().as_str())),
        ("value", unsigned(id.value())),
    ])
}

fn entity_namespace(kind: EntityKind) -> &'static str {
    kind.namespace()
}

fn canonical_time_value(value: CanonicalTime) -> Value {
    object([
        ("chartTimeSeconds", float(value.chart_time_seconds())),
        ("sourceBeat", optional(value.source_beat().map(beat_value))),
    ])
}

fn beat_value(value: Beat) -> Value {
    object([
        ("numerator", integer(value.numerator())),
        ("denominator", integer(value.denominator())),
    ])
}

fn color_value(value: CanonicalColor) -> Value {
    object([
        ("red", float(value.red())),
        ("green", float(value.green())),
        ("blue", float(value.blue())),
        ("alpha", float(value.alpha())),
    ])
}

fn vec2_value(value: CanonicalVec2) -> Value {
    object([("x", float(value.x())), ("y", float(value.y()))])
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<Map<_, _>>(),
    )
}

fn array(values: impl IntoIterator<Item = Value>) -> Value {
    Value::Array(values.into_iter().collect())
}

fn optional(value: Option<Value>) -> Value {
    value.unwrap_or(Value::Null)
}

fn string(value: impl AsRef<str>) -> Value {
    Value::String(value.as_ref().to_owned())
}

fn float(value: f64) -> Value {
    Value::Number(Number::from_f64(value).expect("canonical Float64 values must be finite"))
}

fn integer(value: i64) -> Value {
    Value::Number(Number::from(value))
}

fn unsigned(value: u64) -> Value {
    Value::Number(Number::from(value))
}
