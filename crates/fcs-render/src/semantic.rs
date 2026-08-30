//! Product Render semantic evaluation and reference raster surfaces (I9).

use std::collections::BTreeMap;

use fcs_fcbc::{
    EvaluationEnvironment, RuntimeValue, ValueType, query_descriptor, query_distance,
    query_scroll_coordinate,
};

use crate::{
    RenderLimits,
    loader::{
        DecodedRenderChart, GeometryData, NodeKind, PaintData, PaintRecord, PathCommand, PathRecord,
    },
};

/// One drawable operation after semantic attachment/visibility filtering.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawOp {
    pub node_id: u64,
    pub kind: NodeKind,
    pub layer_index: u32,
    pub z_order: i32,
    pub document_order: u32,
    pub fill_rgba: Option<[f64; 4]>,
    pub linear_gradient: Option<LinearGradientDrawOp>,
    pub radial_gradient: Option<RadialGradientDrawOp>,
    pub image_pattern: Option<ImagePatternDrawOp>,
    pub stroke: Option<StrokeDrawOp>,
    pub image: Option<ImageDrawOp>,
    pub opacity: f64,
    pub world_matrix: [f64; 9],
    pub composite: u16,
    pub clip_chain: Vec<u64>,
    pub isolation_chain: Vec<IsolationDrawOp>,
    pub bounds: [f64; 4],
}

/// One isolated Group/ClipGroup boundary surrounding a drawable operation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IsolationDrawOp {
    pub node_id: u64,
    pub opacity: f64,
    pub composite: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageDrawOp {
    pub resource_id: u64,
    pub destination: [f64; 4],
    pub source: [f64; 4],
    pub sampling: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePatternDrawOp {
    pub resource_id: u64,
    pub position: [f64; 2],
    pub origin: [f64; 2],
    pub rotation: f64,
    pub scale: [f64; 2],
    pub repeat: u16,
    pub sampling: u16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StrokeDrawOp {
    pub width: f64,
    pub cap: u16,
    pub join: u16,
    pub miter_limit: f64,
    pub dash_offset: f64,
    pub dash: Vec<f64>,
    pub fill_rgba: Option<[f64; 4]>,
    pub linear_gradient: Option<LinearGradientDrawOp>,
    pub radial_gradient: Option<RadialGradientDrawOp>,
    pub image_pattern: Option<ImagePatternDrawOp>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LinearGradientDrawOp {
    pub start: [f64; 2],
    pub end: [f64; 2],
    pub spread: u16,
    pub stops: Vec<GradientStopDrawOp>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RadialGradientDrawOp {
    pub start_center: [f64; 2],
    pub start_radius: f64,
    pub end_center: [f64; 2],
    pub end_radius: f64,
    pub spread: u16,
    pub stops: Vec<GradientStopDrawOp>,
}

type PaintParts = (
    Option<[f64; 4]>,
    Option<LinearGradientDrawOp>,
    Option<RadialGradientDrawOp>,
    Option<ImagePatternDrawOp>,
);

#[derive(Clone, Debug, PartialEq)]
pub struct GradientStopDrawOp {
    pub offset: f64,
    pub color: [f64; 4],
}

struct SubtreeState {
    inherited_opacity: f64,
    parent_matrix: [f64; 9],
    inherited_clips: Vec<u64>,
    isolation_chain: Vec<IsolationDrawOp>,
    attachment: Option<AttachmentState>,
}

struct EvaluatedScene {
    ops: Vec<DrawOp>,
    events: Vec<SceneEvent>,
    shapes: BTreeMap<u64, EvaluatedShape>,
    clips: BTreeMap<u64, EvaluatedClip>,
}

enum SceneEvent {
    BeginIsolation(IsolationDrawOp),
    Draw(usize),
    EndIsolation(u64),
}

#[derive(Clone)]
struct EvaluatedShape {
    shape: LocalShape,
    world_matrix: [f64; 9],
}

const GLYPH_FLATTEN_TOLERANCE: f64 = 1.0 / 1024.0;
const GLYPH_MAX_FLATTEN_DEPTH: u8 = 32;
type TextContours = Vec<Vec<[f64; 2]>>;

#[derive(Clone)]
struct PathSubpath {
    points: Vec<[f64; 2]>,
    segment_lengths: Vec<f64>,
    joins_after: Vec<bool>,
    closed: bool,
}

#[derive(Clone)]
enum LocalShape {
    Rect {
        bounds: [f64; 4],
    },
    RoundedRect {
        bounds: [f64; 4],
        radii: [f64; 4],
        stroke_path: Option<PathSubpath>,
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
    Ellipse {
        center: [f64; 2],
        radius_x: f64,
        radius_y: f64,
        rotation: f64,
        stroke_path: Option<PathSubpath>,
    },
    Line {
        start: [f64; 2],
        end: [f64; 2],
    },
    Polygon {
        points: Vec<[f64; 2]>,
        /// Render section 15.2 keeps a Polyline stroke open and closes a Polygon stroke. Fill
        /// uses the implicit closing segment either way, so only the stroke reads this.
        closed: bool,
    },
    Path {
        subpaths: Vec<PathSubpath>,
        fill_rule: u16,
    },
    Text {
        contours: Vec<Vec<[f64; 2]>>,
    },
    Image {
        bounds: [f64; 4],
    },
}

#[derive(Clone)]
struct EvaluatedClip {
    shape: Option<EvaluatedShape>,
}

#[derive(Clone, Copy)]
struct AttachmentState {
    matrix: [f64; 9],
    environment: EvaluationEnvironment,
}

/// Evaluate a deterministic draw-list for the loaded Render scene.
///
/// Drawable nodes follow the loader-validated layer and hierarchical storage order.
/// Group/ClipGroup containers are omitted from the draw list.
pub fn evaluate_semantic_draw_list(
    chart: &DecodedRenderChart,
) -> Result<Vec<DrawOp>, &'static str> {
    evaluate_semantic_draw_list_at(chart, 0.0)
}

/// Evaluate the Render scene at one chart-time query point.
///
/// Query-time gates are deliberately lazy: active is checked before any
/// descriptor, then visibility, then the remaining node properties. A hidden
/// subtree therefore cannot expose an execution error from a later descriptor.
pub fn evaluate_semantic_draw_list_at(
    chart: &DecodedRenderChart,
    chart_time: f64,
) -> Result<Vec<DrawOp>, &'static str> {
    Ok(evaluate_scene_at(chart, chart_time)?.ops)
}

fn evaluate_scene_at(
    chart: &DecodedRenderChart,
    chart_time: f64,
) -> Result<EvaluatedScene, &'static str> {
    if !chart_time.is_finite() {
        return Err("render.invalid-descriptor");
    }
    let mut scene = EvaluatedScene {
        ops: Vec::new(),
        events: Vec::new(),
        shapes: BTreeMap::new(),
        clips: BTreeMap::new(),
    };
    let mut children = vec![Vec::new(); chart.nodes.len()];
    for (index, node) in chart.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children
                .get_mut(parent as usize)
                .ok_or("render.invalid-graph")?
                .push(index);
        }
    }
    for siblings in &mut children {
        sort_node_indices(chart, siblings);
    }

    let mut layer_indices: Vec<_> = (0..chart.layers.len()).collect();
    layer_indices.sort_by(|left, right| {
        let left = &chart.layers[*left];
        let right = &chart.layers[*right];
        left.pass
            .cmp(&right.pass)
            .then(left.z_order.cmp(&right.z_order))
            .then(left.document_order.cmp(&right.document_order))
            .then(left.id.cmp(&right.id))
    });
    for layer_index in layer_indices {
        let layer = &chart.layers[layer_index];
        let mut roots: Vec<_> = if layer.root_count == 0 {
            Vec::new()
        } else {
            let first = layer.first_root as usize;
            let end = first
                .checked_add(layer.root_count as usize)
                .ok_or("render.invalid-graph")?;
            chart
                .nodes
                .get(first..end)
                .ok_or("render.invalid-graph")?
                .iter()
                .enumerate()
                .map(|(offset, _)| first + offset)
                .collect()
        };
        sort_node_indices(chart, &mut roots);
        for root in roots {
            emit_draw_subtree(
                chart,
                &children,
                root,
                chart_time,
                SubtreeState {
                    inherited_opacity: 1.0,
                    parent_matrix: identity_matrix(),
                    inherited_clips: Vec::new(),
                    isolation_chain: Vec::new(),
                    attachment: None,
                },
                &mut scene,
            )?;
        }
    }
    Ok(scene)
}

fn sort_node_indices(chart: &DecodedRenderChart, indices: &mut [usize]) {
    indices.sort_by(|left, right| {
        let left = &chart.nodes[*left];
        let right = &chart.nodes[*right];
        left.z_order
            .cmp(&right.z_order)
            .then(left.document_order.cmp(&right.document_order))
            .then(left.id.cmp(&right.id))
    });
}

fn emit_draw_subtree(
    chart: &DecodedRenderChart,
    children: &[Vec<usize>],
    node_index: usize,
    chart_time: f64,
    state: SubtreeState,
    scene: &mut EvaluatedScene,
) -> Result<(), &'static str> {
    let SubtreeState {
        inherited_opacity,
        parent_matrix,
        inherited_clips,
        isolation_chain,
        attachment,
    } = state;
    let node = chart.nodes.get(node_index).ok_or("render.invalid-graph")?;
    if !active_at(node, chart_time) {
        return Ok(());
    }
    if !query_attachment_gate(chart, node, chart_time)? {
        return Ok(());
    }
    let attachment = attachment.unwrap_or(query_attachment(chart, node, chart_time)?);
    if !query_visibility(chart, node, chart_time, attachment.environment)? {
        return Ok(());
    }
    let parent_matrix = if node.parent.is_none() {
        attachment.matrix
    } else {
        parent_matrix
    };
    let mut clip_chain = inherited_clips;
    let clip = if let Some(clip_index) = node.clip_ref {
        let clip = chart
            .clips
            .get(clip_index as usize)
            .ok_or("render.invalid-reference")?;
        clip_chain.push(clip.id);
        Some((clip.id, clip.geometry_ref))
    } else {
        None
    };
    let world_matrix = multiply_matrix(
        parent_matrix,
        node_local_matrix(chart, node, chart_time, attachment.environment)?,
    )?;
    if let Some((clip_id, geometry_ref)) = clip {
        let geometry = geometry_evaluation(
            chart,
            Some(geometry_ref),
            chart_time,
            attachment.environment,
            world_matrix,
            false,
        )?;
        scene.clips.insert(
            clip_id,
            EvaluatedClip {
                shape: geometry.shape.map(|shape| EvaluatedShape {
                    shape,
                    world_matrix,
                }),
            },
        );
    }
    let (fill_rgba, linear_gradient, radial_gradient, image_pattern, bounds, image, stroke) =
        if node.kind.is_drawable() {
            let geometry = geometry_evaluation(
                chart,
                node.geometry_ref,
                chart_time,
                attachment.environment,
                world_matrix,
                node.stroke_ref.is_some(),
            )?;
            if let Some(shape) = geometry.shape {
                scene.shapes.insert(
                    node.id,
                    EvaluatedShape {
                        shape,
                        world_matrix,
                    },
                );
            }
            let paint = match node.fill_paint {
                Some(index) => paint_rgba(
                    chart,
                    chart
                        .paints
                        .get(index as usize)
                        .ok_or("render.invalid-reference")?,
                    chart_time,
                    attachment.environment,
                )?,
                None => (None, None, None, None),
            };
            let stroke = match node.stroke_ref {
                Some(index) => {
                    let stroke_record = chart
                        .strokes
                        .get(index as usize)
                        .ok_or("render.invalid-reference")?;
                    let paint = paint_rgba(
                        chart,
                        chart
                            .paints
                            .get(stroke_record.paint_ref as usize)
                            .ok_or("render.invalid-reference")?,
                        chart_time,
                        attachment.environment,
                    )?;
                    let width = query_scalar_in(
                        chart,
                        stroke_record.width_descriptor,
                        chart_time,
                        ValueType::Length,
                        attachment.environment,
                    )?;
                    let dash_offset = query_scalar_in(
                        chart,
                        stroke_record.dash_offset_descriptor,
                        chart_time,
                        ValueType::Length,
                        attachment.environment,
                    )?;
                    let dash_total = stroke_record.dash.iter().sum::<f64>();
                    if !width.is_finite()
                        || width < 0.0
                        || !dash_offset.is_finite()
                        || !stroke_record.miter_limit.is_finite()
                        || stroke_record.miter_limit < 1.0
                        || !matches!(stroke_record.cap, 1..=3)
                        || !matches!(stroke_record.join, 1..=3)
                        || stroke_record
                            .dash
                            .iter()
                            .any(|value| !value.is_finite() || *value < 0.0)
                        || (!stroke_record.dash.is_empty()
                            && (!dash_total.is_finite() || dash_total <= 0.0))
                    {
                        return Err("render.invalid-stroke");
                    }
                    Some(StrokeDrawOp {
                        width,
                        cap: stroke_record.cap,
                        join: stroke_record.join,
                        miter_limit: stroke_record.miter_limit,
                        dash_offset,
                        dash: stroke_record.dash.clone(),
                        fill_rgba: paint.0,
                        linear_gradient: paint.1.clone(),
                        radial_gradient: paint.2.clone(),
                        image_pattern: paint.3,
                    })
                }
                None => None,
            };
            (
                paint.0,
                paint.1,
                paint.2,
                paint.3,
                geometry.world_bounds,
                geometry.image,
                stroke,
            )
        } else {
            (None, None, None, None, [0.0; 4], None, None)
        };
    let opacity = query_opacity(chart, node, chart_time, attachment.environment)?;
    let effective_opacity = inherited_opacity * opacity;
    if !effective_opacity.is_finite() {
        return Err("render.invalid-composite");
    }
    if node.kind.is_drawable() {
        let op_index = scene.ops.len();
        scene.ops.push(DrawOp {
            node_id: node.id,
            kind: node.kind,
            layer_index: node.layer_index,
            z_order: node.z_order,
            document_order: node.document_order,
            fill_rgba,
            linear_gradient,
            radial_gradient,
            image_pattern,
            stroke,
            image,
            opacity: effective_opacity,
            world_matrix,
            composite: node.composite,
            clip_chain: clip_chain.clone(),
            isolation_chain: isolation_chain.clone(),
            bounds,
        });
        scene.events.push(SceneEvent::Draw(op_index));
    }
    let mut child_isolation_chain = isolation_chain;
    let isolation = node.isolated().then_some(IsolationDrawOp {
        node_id: node.id,
        opacity,
        composite: node.composite,
    });
    if let Some(boundary) = isolation {
        scene.events.push(SceneEvent::BeginIsolation(boundary));
        child_isolation_chain.push(boundary);
    }
    for child_index in children.get(node_index).ok_or("render.invalid-graph")? {
        let child_opacity = if node.isolated() {
            inherited_opacity
        } else {
            effective_opacity
        };
        emit_draw_subtree(
            chart,
            children,
            *child_index,
            chart_time,
            SubtreeState {
                inherited_opacity: child_opacity,
                parent_matrix: world_matrix,
                inherited_clips: clip_chain.clone(),
                isolation_chain: child_isolation_chain.clone(),
                attachment: Some(attachment),
            },
            scene,
        )?;
    }
    if let Some(boundary) = isolation {
        scene
            .events
            .push(SceneEvent::EndIsolation(boundary.node_id));
    }
    Ok(())
}

fn active_at(node: &crate::loader::NodeRecord, chart_time: f64) -> bool {
    (node.flags & 1 != 0 || chart_time >= node.active_start)
        && (node.flags & 2 != 0 || chart_time < node.active_end)
}

fn query_attachment_gate(
    chart: &DecodedRenderChart,
    node: &crate::loader::NodeRecord,
    chart_time: f64,
) -> Result<bool, &'static str> {
    if node.attachment.kind != 4 || node.flags & (1 << 3) != 0 {
        return Ok(true);
    }
    let note = chart
        .core
        .notes
        .iter()
        .find(|note| note.id == node.attachment.id)
        .ok_or("render.invalid-reference")?;
    if note.flags & (1 << 1) == 0 {
        return Ok(false);
    }
    match query_value(chart, note.property_descriptors[9], chart_time)? {
        RuntimeValue::Bool(value) => Ok(value),
        _ => Err("render.invalid-descriptor"),
    }
}

fn query_attachment(
    chart: &DecodedRenderChart,
    node: &crate::loader::NodeRecord,
    chart_time: f64,
) -> Result<AttachmentState, &'static str> {
    match node.attachment.kind {
        1 | 2 => Ok(AttachmentState {
            matrix: identity_matrix(),
            environment: attachment_environment(chart, chart_time, 0.0, 0.0)?,
        }),
        3 => {
            let (matrix, q) = line_world_matrix(chart, node.attachment.id, chart_time)?;
            Ok(AttachmentState {
                matrix,
                environment: attachment_environment(chart, chart_time, q, 0.0)?,
            })
        }
        4 => {
            let note = chart
                .core
                .notes
                .iter()
                .find(|note| note.id == node.attachment.id)
                .ok_or("render.invalid-reference")?;
            let line = chart
                .core
                .lines
                .iter()
                .find(|line| line.id == note.line_id)
                .ok_or("render.invalid-reference")?;
            let (line_matrix, q) = line_world_matrix(chart, line.id, chart_time)?;
            let base_environment = attachment_environment(chart, chart_time, q, 0.0)?;
            let scroll_factor = query_scalar_in(
                chart,
                note.property_descriptors[1],
                chart_time,
                ValueType::Float,
                base_environment,
            )?;
            let note_floor = line_floor_position(chart, line.id, note.time)?;
            let query_floor = line_floor_position(chart, line.id, chart_time)?;
            let distance = (note_floor - query_floor) * line.floor_scale * scroll_factor;
            if !distance.is_finite() {
                return Err("render.invalid-descriptor");
            }
            let environment = attachment_environment(chart, chart_time, q, distance)?;
            let position_x = query_scalar_in(
                chart,
                note.property_descriptors[0],
                chart_time,
                ValueType::Length,
                environment,
            )?;
            let x_offset = query_scalar_in(
                chart,
                note.property_descriptors[2],
                chart_time,
                ValueType::Length,
                environment,
            )?;
            let y_offset = query_scalar_in(
                chart,
                note.property_descriptors[3],
                chart_time,
                ValueType::Length,
                environment,
            )?;
            let translation = translation_matrix(position_x + x_offset, distance + y_offset);
            Ok(AttachmentState {
                matrix: multiply_matrix(line_matrix, translation)?,
                environment,
            })
        }
        _ => Err("render.invalid-reference"),
    }
}

fn attachment_environment(
    chart: &DecodedRenderChart,
    chart_time: f64,
    q: f64,
    d: f64,
) -> Result<EvaluationEnvironment, &'static str> {
    Ok(EvaluationEnvironment {
        s: chart_time,
        b: fcs_fcbc::chart_beat_at_time(&chart.core, chart_time)
            .map_err(|_| "render.invalid-descriptor")?,
        q,
        d,
        p: 0.0,
    })
}

#[derive(Clone, Copy)]
struct LineComponents {
    position: [f64; 2],
    rotation: f64,
    scale: [f64; 2],
}

fn line_world_matrix(
    chart: &DecodedRenderChart,
    line_id: u64,
    chart_time: f64,
) -> Result<([f64; 9], f64), &'static str> {
    let mut chain = Vec::new();
    let mut current_id = line_id;
    loop {
        let line = chart
            .core
            .lines
            .iter()
            .find(|line| line.id == current_id)
            .ok_or("render.invalid-reference")?;
        chain.push(line);
        if line.parent_id == 0 {
            break;
        }
        current_id = line.parent_id;
        if chain.len() > chart.core.lines.len() {
            return Err("render.invalid-reference");
        }
    }
    chain.reverse();

    let mut parent = LineComponents {
        position: [0.0, 0.0],
        rotation: 0.0,
        scale: [1.0, 1.0],
    };
    let mut world_matrix = identity_matrix();
    let mut target_q = 0.0;
    for line in chain {
        let q = query_scroll_coordinate(&chart.core, line.scroll_tempo_descriptor, chart_time)
            .map_err(|_| "render.invalid-descriptor")?;
        let environment = attachment_environment(chart, chart_time, q, 0.0)?;
        let local = LineComponents {
            position: query_vec2_in(
                chart,
                line.position_descriptor,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?,
            rotation: query_scalar_in(
                chart,
                line.rotation_descriptor,
                chart_time,
                ValueType::Angle,
                environment,
            )?,
            scale: query_vec2_in(
                chart,
                line.scale_descriptor,
                chart_time,
                ValueType::Vec2Float,
                environment,
            )?,
        };
        let origin = constant_vec2(chart, line.transform_origin_constant, ValueType::Vec2Length)?;
        let mut local_matrix = translation_matrix(local.position[0], local.position[1]);
        local_matrix = multiply_matrix(local_matrix, translation_matrix(origin[0], origin[1]))?;
        local_matrix = multiply_matrix(local_matrix, rotation_matrix(local.rotation))?;
        local_matrix = multiply_matrix(local_matrix, scale_matrix(local.scale[0], local.scale[1]))?;
        local_matrix = multiply_matrix(local_matrix, translation_matrix(-origin[0], -origin[1]))?;

        let inherited = if line.parent_id == 0 {
            LineComponents {
                position: [0.0, 0.0],
                rotation: 0.0,
                scale: [1.0, 1.0],
            }
        } else {
            LineComponents {
                position: if line.inherit_flags & 1 != 0 {
                    parent.position
                } else {
                    [0.0, 0.0]
                },
                rotation: if line.inherit_flags & (1 << 1) != 0 {
                    parent.rotation
                } else {
                    0.0
                },
                scale: if line.inherit_flags & (1 << 2) != 0 {
                    parent.scale
                } else {
                    [1.0, 1.0]
                },
            }
        };
        let mut inherited_matrix = translation_matrix(inherited.position[0], inherited.position[1]);
        inherited_matrix = multiply_matrix(inherited_matrix, rotation_matrix(inherited.rotation))?;
        inherited_matrix = multiply_matrix(
            inherited_matrix,
            scale_matrix(inherited.scale[0], inherited.scale[1]),
        )?;
        world_matrix = multiply_matrix(inherited_matrix, local_matrix)?;
        let world_position =
            transform_point(world_matrix, [0.0, 0.0]).map_err(|_| "render.invalid-descriptor")?;
        parent = LineComponents {
            position: world_position,
            rotation: inherited.rotation + local.rotation,
            scale: [
                inherited.scale[0] * local.scale[0],
                inherited.scale[1] * local.scale[1],
            ],
        };
        if !parent.rotation.is_finite() || parent.scale.iter().any(|value| !value.is_finite()) {
            return Err("render.invalid-descriptor");
        }
        target_q = q;
    }
    Ok((world_matrix, target_q))
}

fn line_floor_position(
    chart: &DecodedRenderChart,
    line_id: u64,
    chart_time: f64,
) -> Result<f64, &'static str> {
    let mut current_id = line_id;
    // ponytail: FCBC exposes binary64 local floors; use its future high-precision effective-distance
    // query when that ABI surface is available.
    let mut total = 0.0;
    loop {
        let line = chart
            .core
            .lines
            .iter()
            .find(|line| line.id == current_id)
            .ok_or("render.invalid-reference")?;
        let distance = chart
            .core
            .distances
            .get(line.distance_descriptor as usize)
            .ok_or("render.invalid-reference")?;
        let local = query_distance(&chart.core, line.distance_descriptor, chart_time)
            .map_err(|_| "render.invalid-descriptor")?
            .floor_position;
        total += local;
        if !total.is_finite() {
            return Err("render.invalid-descriptor");
        }
        if line.parent_id == 0 || line.inherit_flags & (1 << 4) == 0 {
            return Ok(total);
        }
        if distance.line_id != line.id {
            return Err("render.invalid-reference");
        }
        current_id = line.parent_id;
    }
}

fn constant_vec2(
    chart: &DecodedRenderChart,
    constant: u32,
    expected: ValueType,
) -> Result<[f64; 2], &'static str> {
    match chart.core.constants.get(constant as usize) {
        Some(RuntimeValue::Vec2 { ty, value })
            if *ty == expected && value.iter().all(|component| component.is_finite()) =>
        {
            Ok(*value)
        }
        _ => Err("render.invalid-descriptor"),
    }
}

fn query_visibility(
    chart: &DecodedRenderChart,
    node: &crate::loader::NodeRecord,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<bool, &'static str> {
    match query_value_in(chart, node.visibility_descriptor, chart_time, environment)? {
        RuntimeValue::Bool(value) => Ok(value),
        _ => Err("render.invalid-descriptor"),
    }
}

fn query_opacity(
    chart: &DecodedRenderChart,
    node: &crate::loader::NodeRecord,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<f64, &'static str> {
    let value = query_value_in(chart, node.opacity_descriptor, chart_time, environment)?;
    let RuntimeValue::Scalar {
        ty: ValueType::Float,
        value,
    } = value
    else {
        return Err("render.invalid-descriptor");
    };
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(value)
    } else {
        Err("render.invalid-composite")
    }
}

fn query_value(
    chart: &DecodedRenderChart,
    descriptor: u32,
    chart_time: f64,
) -> Result<RuntimeValue, &'static str> {
    query_value_in(
        chart,
        descriptor,
        chart_time,
        attachment_environment(chart, chart_time, 0.0, 0.0)?,
    )
}

fn query_value_in(
    chart: &DecodedRenderChart,
    descriptor: u32,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<RuntimeValue, &'static str> {
    query_descriptor(&chart.core, descriptor, chart_time, environment)
        .map(|evaluation| evaluation.value)
        .map_err(|_| "render.invalid-descriptor")
}

fn node_local_matrix(
    chart: &DecodedRenderChart,
    node: &crate::loader::NodeRecord,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<[f64; 9], &'static str> {
    let position = query_vec2_in(
        chart,
        node.position_descriptor,
        chart_time,
        ValueType::Vec2Length,
        environment,
    )?;
    let origin = query_vec2_in(
        chart,
        node.origin_descriptor,
        chart_time,
        ValueType::Vec2Length,
        environment,
    )?;
    let rotation = query_scalar_in(
        chart,
        node.rotation_descriptor,
        chart_time,
        ValueType::Angle,
        environment,
    )?;
    let scale = query_vec2_in(
        chart,
        node.scale_descriptor,
        chart_time,
        ValueType::Vec2Float,
        environment,
    )?;

    let mut matrix = translation_matrix(position[0], position[1]);
    matrix = multiply_matrix(matrix, translation_matrix(origin[0], origin[1]))?;
    matrix = multiply_matrix(matrix, rotation_matrix(rotation))?;
    matrix = multiply_matrix(matrix, scale_matrix(scale[0], scale[1]))?;
    multiply_matrix(matrix, translation_matrix(-origin[0], -origin[1]))
}

fn query_vec2_in(
    chart: &DecodedRenderChart,
    descriptor: u32,
    chart_time: f64,
    expected: ValueType,
    environment: EvaluationEnvironment,
) -> Result<[f64; 2], &'static str> {
    match query_value_in(chart, descriptor, chart_time, environment)? {
        RuntimeValue::Vec2 { ty, value }
            if ty == expected && value.iter().all(|component| component.is_finite()) =>
        {
            Ok(value)
        }
        _ => Err("render.invalid-descriptor"),
    }
}

fn query_scalar_in(
    chart: &DecodedRenderChart,
    descriptor: u32,
    chart_time: f64,
    expected: ValueType,
    environment: EvaluationEnvironment,
) -> Result<f64, &'static str> {
    match query_value_in(chart, descriptor, chart_time, environment)? {
        RuntimeValue::Scalar { ty, value } if ty == expected && value.is_finite() => Ok(value),
        _ => Err("render.invalid-descriptor"),
    }
}

fn identity_matrix() -> [f64; 9] {
    [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
}

fn translation_matrix(x: f64, y: f64) -> [f64; 9] {
    [1.0, 0.0, x, 0.0, 1.0, y, 0.0, 0.0, 1.0]
}

fn rotation_matrix(angle: f64) -> [f64; 9] {
    let (sin, cos) = angle.sin_cos();
    [cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0]
}

fn scale_matrix(x: f64, y: f64) -> [f64; 9] {
    [x, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 1.0]
}

fn multiply_matrix(left: [f64; 9], right: [f64; 9]) -> Result<[f64; 9], &'static str> {
    let mut result = [0.0; 9];
    for row in 0..3 {
        for column in 0..3 {
            result[row * 3 + column] = (left[row * 3] * right[column])
                + (left[row * 3 + 1] * right[column + 3])
                + (left[row * 3 + 2] * right[column + 6]);
        }
    }
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
        .ok_or("render.invalid-descriptor")
}

fn transform_point(matrix: [f64; 9], point: [f64; 2]) -> Result<[f64; 2], &'static str> {
    let x = (matrix[0] * point[0]) + (matrix[1] * point[1]) + matrix[2];
    let y = (matrix[3] * point[0]) + (matrix[4] * point[1]) + matrix[5];
    if x.is_finite() && y.is_finite() {
        Ok([x, y])
    } else {
        Err("render.invalid-geometry")
    }
}

fn transformed_bounds(matrix: [f64; 9], bounds: [f64; 4]) -> Result<[f64; 4], &'static str> {
    let corners = [
        transform_point(matrix, [bounds[0], bounds[1]])?,
        transform_point(matrix, [bounds[2], bounds[1]])?,
        transform_point(matrix, [bounds[0], bounds[3]])?,
        transform_point(matrix, [bounds[2], bounds[3]])?,
    ];
    let mut result = [corners[0][0], corners[0][1], corners[0][0], corners[0][1]];
    for [x, y] in corners.into_iter().skip(1) {
        result[0] = result[0].min(x);
        result[1] = result[1].min(y);
        result[2] = result[2].max(x);
        result[3] = result[3].max(y);
    }
    Ok(result)
}

#[derive(Clone)]
struct RasterShape {
    shape: LocalShape,
    inverse_world: Option<[f64; 9]>,
}

struct RasterOp {
    shape: RasterShape,
    clips: Vec<RasterShape>,
    source: Option<RasterSource>,
    stroke_source: Option<RasterSource>,
    stroke: Option<StrokeDrawOp>,
    opacity: f64,
    composite: u16,
}

struct RasterIsolationBuffer {
    boundary: IsolationDrawOp,
    color: [f64; 4],
}

fn composite_raster_sample(
    destination: &mut [f64; 4],
    stack: &mut [RasterIsolationBuffer],
    source: [f64; 4],
    composite: u16,
) -> Result<(), &'static str> {
    if let Some(buffer) = stack.last_mut() {
        composite_premultiplied(&mut buffer.color, source, composite)
    } else {
        composite_premultiplied(destination, source, composite)
    }
}

enum RasterSource {
    Solid([f64; 4]),
    LinearGradient(LinearGradientDrawOp),
    RadialGradient(RadialGradientDrawOp),
    ImagePattern(ImagePatternDrawOp),
    Image(ImageDrawOp),
}

fn raster_shape(shape: &EvaluatedShape) -> RasterShape {
    RasterShape {
        shape: shape.shape.clone(),
        inverse_world: inverse_affine(shape.world_matrix),
    }
}

fn inverse_affine(matrix: [f64; 9]) -> Option<[f64; 9]> {
    let determinant = (matrix[0] * matrix[4]) - (matrix[1] * matrix[3]);
    if !determinant.is_finite() || determinant == 0.0 {
        return None;
    }
    let inverse = 1.0 / determinant;
    let result = [
        matrix[4] * inverse,
        -matrix[1] * inverse,
        (matrix[1] * matrix[5] - matrix[4] * matrix[2]) * inverse,
        -matrix[3] * inverse,
        matrix[0] * inverse,
        (matrix[3] * matrix[2] - matrix[0] * matrix[5]) * inverse,
        0.0,
        0.0,
        1.0,
    ];
    result
        .iter()
        .all(|value| value.is_finite())
        .then_some(result)
}

fn raster_shape_contains(shape: &RasterShape, point: [f64; 2]) -> bool {
    raster_shape_local_point(shape, point)
        .is_some_and(|local| local_shape_contains(&shape.shape, local))
}

fn raster_shape_local_point(shape: &RasterShape, point: [f64; 2]) -> Option<[f64; 2]> {
    let inverse_world = shape.inverse_world?;
    transform_point(inverse_world, point).ok()
}

fn local_shape_contains(shape: &LocalShape, point: [f64; 2]) -> bool {
    match shape {
        LocalShape::Rect { bounds } => {
            bounds[2] > bounds[0]
                && bounds[3] > bounds[1]
                && point[0] >= bounds[0]
                && point[0] <= bounds[2]
                && point[1] >= bounds[1]
                && point[1] <= bounds[3]
        }
        LocalShape::RoundedRect { bounds, radii, .. } => {
            rounded_rect_contains(*bounds, *radii, point)
        }
        LocalShape::Circle { center, radius } => {
            *radius > 0.0
                && (point[0] - center[0]).mul_add(
                    point[0] - center[0],
                    (point[1] - center[1]) * (point[1] - center[1]),
                ) <= *radius * *radius
        }
        LocalShape::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
            ..
        } => ellipse_contains(*center, *radius_x, *radius_y, *rotation, point),
        LocalShape::Line { .. } => false,
        LocalShape::Polygon { points, .. } => polygon_contains(points, point),
        LocalShape::Path {
            subpaths,
            fill_rule,
        } => path_contains(subpaths, *fill_rule, point),
        LocalShape::Text { contours } => text_contains(contours, point),
        LocalShape::Image { bounds } => {
            bounds[2] > bounds[0]
                && bounds[3] > bounds[1]
                && point[0] >= bounds[0]
                && point[0] < bounds[2]
                && point[1] >= bounds[1]
                && point[1] < bounds[3]
        }
    }
}

fn text_contains(contours: &[Vec<[f64; 2]>], point: [f64; 2]) -> bool {
    let mut winding = 0i32;
    for contour in contours {
        // Glyph outlines use the same nonzero fill rule as Render 1.0 Text.
        // Keeping the winding accumulator across contours preserves holes.

        if contour.len() < 2 {
            continue;
        }
        for index in 0..contour.len() {
            let [x0, y0] = contour[index];
            let [x1, y1] = contour[(index + 1) % contour.len()];
            let cross = (x1 - x0) * (point[1] - y0) - (point[0] - x0) * (y1 - y0);
            if cross == 0.0
                && point[0] >= x0.min(x1)
                && point[0] <= x0.max(x1)
                && point[1] >= y0.min(y1)
                && point[1] <= y0.max(y1)
            {
                return true;
            }
            if y0 <= point[1] {
                if y1 > point[1] && cross > 0.0 {
                    winding += 1;
                }
            } else if y1 <= point[1] && cross < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}

fn stroke_contains(
    shape: &LocalShape,
    point: [f64; 2],
    stroke: &StrokeDrawOp,
) -> Result<bool, &'static str> {
    if !stroke.width.is_finite()
        || stroke.width < 0.0
        || !stroke.dash_offset.is_finite()
        || !stroke.miter_limit.is_finite()
        || stroke.miter_limit < 1.0
        || !matches!(stroke.cap, 1..=3)
        || !matches!(stroke.join, 1..=3)
        || stroke
            .dash
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err("render.invalid-stroke");
    }
    if stroke.width == 0.0 {
        return Ok(false);
    }
    if !point.iter().all(|value| value.is_finite()) {
        return Err("render.invalid-geometry");
    }
    match shape {
        LocalShape::Rect { bounds } => {
            // Section 15.2 starts at Rect origin, walks left/up, top/right, right/down,
            // then closes along the bottom edge: clockwise in FCS Y-up coordinates.
            let points = [
                [bounds[0], bounds[1]],
                [bounds[0], bounds[3]],
                [bounds[2], bounds[3]],
                [bounds[2], bounds[1]],
            ];
            stroke_polyline_contains(&points, true, None, None, point, stroke)
        }
        LocalShape::Line { start, end } => stroke_line_contains(*start, *end, point, stroke),
        LocalShape::Circle { center, radius } => {
            stroke_circle_contains(*center, *radius, point, stroke)
        }
        LocalShape::Polygon { points, closed } => {
            stroke_polyline_contains(points, *closed, None, None, point, stroke)
        }
        LocalShape::RoundedRect { stroke_path, .. } | LocalShape::Ellipse { stroke_path, .. } => {
            let subpath = stroke_path.as_ref().ok_or("render.invalid-geometry")?;
            stroke_polyline_contains(
                &subpath.points,
                subpath.closed,
                Some(&subpath.segment_lengths),
                Some(&subpath.joins_after),
                point,
                stroke,
            )
        }
        LocalShape::Path { subpaths, .. } => {
            for subpath in subpaths {
                if stroke_polyline_contains(
                    &subpath.points,
                    subpath.closed,
                    Some(&subpath.segment_lengths),
                    Some(&subpath.joins_after),
                    point,
                    stroke,
                )? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        LocalShape::Text { contours } => {
            for contour in contours {
                if stroke_polyline_contains(contour, true, None, None, point, stroke)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Err("render.invalid-geometry"),
    }
}

/// Render section 15.2 dilates the centre line by `width/2`, keeps a Polyline stroke open and
/// closes a Polygon stroke. Arc length is exact here, so the dash phase runs along the
/// accumulated segment length from the first declared point in declared order.
///
/// A zero-length segment produces no coverage, cap, join or tangent and does not advance the
/// dash phase, so it is dropped before anything else. `cap` applies at each dash segment's two
/// endpoints, and for an undashed stroke only at the two ends of an open Polyline. `join`
/// applies at every declared vertex the stroke covers on both sides; adaptive subdivision
/// points are not vertices.
fn stroke_polyline_contains(
    points: &[[f64; 2]],
    closed: bool,
    segment_lengths: Option<&[f64]>,
    joins_after: Option<&[bool]>,
    point: [f64; 2],
    stroke: &StrokeDrawOp,
) -> Result<bool, &'static str> {
    if points.len() < 2 || points.iter().flatten().any(|value| !value.is_finite()) {
        return Err("render.invalid-geometry");
    }
    if segment_lengths.is_some_and(|lengths| {
        lengths.len() != points.len() - 1
            || lengths
                .iter()
                .any(|length| !length.is_finite() || *length < 0.0)
    }) {
        return Err("render.invalid-geometry");
    }
    if joins_after.is_some_and(|joins| joins.len() != points.len() - 1) {
        return Err("render.invalid-geometry");
    }
    let half_width = stroke.width / 2.0;
    let mut segments = Vec::with_capacity(points.len());
    let mut total = 0.0;
    let segment_wrap = usize::from(closed && segment_lengths.is_none());
    for index in 0..points.len().saturating_sub(1) + segment_wrap {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let direction = [end[0] - start[0], end[1] - start[1]];
        let length = direction[0].hypot(direction[1]);
        if !length.is_finite() {
            return Err("render.invalid-geometry");
        }
        let metric_length = segment_lengths.map_or(length, |lengths| lengths[index]);
        if length == 0.0 || metric_length == 0.0 {
            continue;
        }
        segments.push(PolylineSegment {
            start,
            direction: [direction[0] / length, direction[1] / length],
            offset: total,
            length,
            metric_length,
            join_after: joins_after.map_or(true, |joins| joins[index]),
        });
        total += metric_length;
    }
    if segments.is_empty() || !total.is_finite() {
        return Ok(false);
    }

    let dash_intervals = stroke_segments(total, stroke.dash_offset, &stroke.dash)?;
    // An undashed stroke covers the whole path, so every vertex joins and only an open path's
    // two ends cap. Section 15.2 gives a closed undashed stroke no endpoint at all.
    let whole_path_on = stroke.dash.is_empty();
    let dash_wraps = closed
        && dash_intervals
            .first()
            .is_some_and(|interval| interval.0 == 0.0)
        && dash_intervals
            .last()
            .is_some_and(|interval| interval.1 == total);
    for (dash_start, dash_end) in &dash_intervals {
        for segment in &segments {
            let along = (point[0] - segment.start[0]) * segment.direction[0]
                + (point[1] - segment.start[1]) * segment.direction[1];
            let across = (point[0] - segment.start[0]) * segment.direction[1]
                - (point[1] - segment.start[1]) * segment.direction[0];
            if across.abs() > half_width {
                continue;
            }
            let low =
                (dash_start - segment.offset).max(0.0) / segment.metric_length * segment.length;
            let high = (dash_end - segment.offset).min(segment.metric_length)
                / segment.metric_length
                * segment.length;
            if low <= high && along >= low && along <= high {
                return Ok(true);
            }
        }
        if whole_path_on && closed {
            continue;
        }
        for (arc, forward) in [(*dash_start, false), (*dash_end, true)] {
            if dash_wraps && ((!forward && arc == 0.0) || (forward && arc == total)) {
                continue;
            }
            let segment = segments
                .iter()
                .find(|segment| arc <= segment.offset + segment.metric_length)
                .unwrap_or(&segments[segments.len() - 1]);
            let along = (arc - segment.offset).clamp(0.0, segment.metric_length)
                / segment.metric_length
                * segment.length;
            let base = [
                segment.start[0] + segment.direction[0] * along,
                segment.start[1] + segment.direction[1] * along,
            ];
            let sign = if forward { 1.0 } else { -1.0 };
            let tangent = [sign * segment.direction[0], sign * segment.direction[1]];
            if cap_contains(base, tangent, point, half_width, stroke.cap)? {
                return Ok(true);
            }
        }
    }

    let join_wrap = usize::from(closed);
    for index in 0..segments.len().saturating_sub(1) + join_wrap {
        let incoming = &segments[index];
        let outgoing = &segments[(index + 1) % segments.len()];
        let arc = incoming.offset + incoming.metric_length;
        let joined = whole_path_on
            || (dash_wraps && index + 1 == segments.len())
            || dash_intervals
                .iter()
                .any(|(start, end)| arc > *start && arc < *end);
        if !joined {
            continue;
        }
        if !incoming.join_after {
            let vertex = [
                incoming.start[0] + incoming.direction[0] * incoming.length,
                incoming.start[1] + incoming.direction[1] * incoming.length,
            ];
            if (point[0] - vertex[0]).hypot(point[1] - vertex[1]) <= half_width {
                return Ok(true);
            }
            continue;
        }
        if join_contains(incoming, outgoing, point, half_width, stroke)? {
            return Ok(true);
        }
    }
    Ok(false)
}

struct PolylineSegment {
    start: [f64; 2],
    direction: [f64; 2],
    offset: f64,
    length: f64,
    metric_length: f64,
    join_after: bool,
}

/// Render section 15.2: butt truncates at the endpoint, square extends `width/2` along the
/// outward tangent, round adds a half disc. `tangent` points out of the stroke.
fn cap_contains(
    base: [f64; 2],
    tangent: [f64; 2],
    point: [f64; 2],
    half_width: f64,
    cap: u16,
) -> Result<bool, &'static str> {
    let offset = [point[0] - base[0], point[1] - base[1]];
    let along = offset[0] * tangent[0] + offset[1] * tangent[1];
    let across = offset[0] * tangent[1] - offset[1] * tangent[0];
    match cap {
        1 => Ok(false),
        2 => Ok(offset[0].hypot(offset[1]) <= half_width),
        3 => Ok(along >= 0.0 && along <= half_width && across.abs() <= half_width),
        _ => Err("render.invalid-stroke"),
    }
}

/// Render section 15.2: bevel connects the two outer offset endpoints, round uses a sector
/// centred on the vertex, and miter uses the outer offset lines' intersection, degrading to
/// bevel once `miterLength/halfWidth` exceeds `miterLimit`.
fn join_contains(
    incoming: &PolylineSegment,
    outgoing: &PolylineSegment,
    point: [f64; 2],
    half_width: f64,
    stroke: &StrokeDrawOp,
) -> Result<bool, &'static str> {
    let vertex = [
        incoming.start[0] + incoming.direction[0] * incoming.length,
        incoming.start[1] + incoming.direction[1] * incoming.length,
    ];
    match stroke.join {
        1 | 3 => {}
        2 => {
            let offset = [point[0] - vertex[0], point[1] - vertex[1]];
            return Ok(offset[0].hypot(offset[1]) <= half_width);
        }
        _ => return Err("render.invalid-stroke"),
    }
    // The outer side of the turn is the one the left normals do not point into.
    let cross = incoming.direction[0] * outgoing.direction[1]
        - incoming.direction[1] * outgoing.direction[0];
    if cross == 0.0 {
        return Ok(false);
    }
    let sign = if cross > 0.0 { -1.0 } else { 1.0 };
    let outward = |direction: &[f64; 2]| [sign * -direction[1], sign * direction[0]];
    let incoming_outward = outward(&incoming.direction);
    let outgoing_outward = outward(&outgoing.direction);
    let corner = |normal: [f64; 2]| {
        [
            vertex[0] + normal[0] * half_width,
            vertex[1] + normal[1] * half_width,
        ]
    };
    let bevel = [vertex, corner(incoming_outward), corner(outgoing_outward)];
    if stroke.join == 3 {
        let bisector = [
            incoming_outward[0] + outgoing_outward[0],
            incoming_outward[1] + outgoing_outward[1],
        ];
        let magnitude = bisector[0].hypot(bisector[1]);
        // `2/|u+v|` is `miterLength/halfWidth` for unit outward normals `u` and `v`.
        if magnitude > 0.0 && 2.0 / magnitude <= stroke.miter_limit {
            let scale = 2.0 * half_width / (magnitude * magnitude);
            let tip = [
                vertex[0] + bisector[0] * scale,
                vertex[1] + bisector[1] * scale,
            ];
            return Ok(polygon_contains(&[vertex, bevel[1], tip, bevel[2]], point));
        }
    }
    Ok(polygon_contains(&bevel, point))
}

/// Render section 15.2 dilates the centre line by `width/2`. A Circle is a closed
/// parametric geometry, so with an empty dash array it has no endpoint and therefore no
/// cap, and no vertex and therefore no join: the stroke is exactly the closed annulus.
///
/// With a dash array the single subpath starts at the local `+X` crossing and winds
/// clockwise, which under FCS `Y-up` is the decreasing-angle direction. Each dash segment
/// is then the annular sector its arc-length range projects to radially, because butt cap
/// truncates on the endpoint's radial line; `round` adds a disc and `square` a tangential
/// rectangle at each of the segment's two endpoints.
fn stroke_circle_contains(
    center: [f64; 2],
    radius: f64,
    point: [f64; 2],
    stroke: &StrokeDrawOp,
) -> Result<bool, &'static str> {
    if !radius.is_finite() || radius < 0.0 || !center.iter().all(|value| value.is_finite()) {
        return Err("render.invalid-geometry");
    }
    if radius == 0.0 {
        return Ok(false);
    }
    let half_width = stroke.width / 2.0;
    let offset = [point[0] - center[0], point[1] - center[1]];
    let distance = offset[0].hypot(offset[1]);
    if !distance.is_finite() {
        return Err("render.invalid-geometry");
    }
    let inner = (radius - half_width).max(0.0);
    let outer = radius + half_width;
    let in_band = distance >= inner && distance <= outer;
    if stroke.dash.is_empty() {
        return Ok(in_band);
    }

    let circumference = std::f64::consts::TAU * radius;
    let segments = stroke_segments(circumference, stroke.dash_offset, &stroke.dash)?;
    if segments.is_empty() {
        return Ok(false);
    }
    let dash_wraps = segments.first().is_some_and(|interval| interval.0 == 0.0)
        && segments
            .last()
            .is_some_and(|interval| interval.1 == circumference);
    if distance == 0.0 {
        // The centre lies on the radial boundary of every sector, and section 15.2 counts a
        // boundary sample as inside, so any dash segment reaches it once the band does.
        return Ok(in_band);
    }
    // Clockwise from the three-o'clock start: arc length grows as the angle decreases.
    let arc = (-offset[1].atan2(offset[0])).rem_euclid(std::f64::consts::TAU) * radius;
    for (segment_start, segment_end) in segments {
        if in_band && arc >= segment_start && arc <= segment_end {
            return Ok(true);
        }
        match stroke.cap {
            1 => continue,
            2 | 3 => {}
            _ => return Err("render.invalid-stroke"),
        }
        for (end_arc, sign) in [(segment_start, -1.0), (segment_end, 1.0)] {
            if dash_wraps
                && ((sign < 0.0 && end_arc == 0.0) || (sign > 0.0 && end_arc == circumference))
            {
                continue;
            }
            let (sin, cos) = (-end_arc / radius).sin_cos();
            let to_point = [offset[0] - radius * cos, offset[1] - radius * sin];
            if stroke.cap == 2 {
                if to_point[0].hypot(to_point[1]) <= half_width {
                    return Ok(true);
                }
                continue;
            }
            // Clockwise travel is `(sin, -cos)`, so `sign` turns it outward at either end.
            let tangent = [sign * sin, -sign * cos];
            let along = to_point[0] * tangent[0] + to_point[1] * tangent[1];
            let across = to_point[0] * tangent[1] - to_point[1] * tangent[0];
            if along >= 0.0 && along <= half_width && across.abs() <= half_width {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn stroke_line_contains(
    start: [f64; 2],
    end: [f64; 2],
    point: [f64; 2],
    stroke: &StrokeDrawOp,
) -> Result<bool, &'static str> {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if !length.is_finite() {
        return Err("render.invalid-geometry");
    }
    if length == 0.0 {
        return Ok(false);
    }
    let half_width = stroke.width / 2.0;
    let direction = [dx / length, dy / length];
    let perpendicular = ((point[0] - start[0]) * dy - (point[1] - start[1]) * dx).abs() / length;
    let along = (point[0] - start[0]) * direction[0] + (point[1] - start[1]) * direction[1];
    for (segment_start, segment_end) in stroke_segments(length, stroke.dash_offset, &stroke.dash)? {
        if along >= segment_start && along <= segment_end && perpendicular <= half_width {
            return Ok(true);
        }
        match stroke.cap {
            1 => {}
            2 => {
                let endpoint = if along < segment_start {
                    [
                        start[0] + direction[0] * segment_start,
                        start[1] + direction[1] * segment_start,
                    ]
                } else {
                    [
                        start[0] + direction[0] * segment_end,
                        start[1] + direction[1] * segment_end,
                    ]
                };
                if along < segment_start || along > segment_end {
                    let dx = point[0] - endpoint[0];
                    let dy = point[1] - endpoint[1];
                    if dx.mul_add(dx, dy * dy) <= half_width * half_width {
                        return Ok(true);
                    }
                }
            }
            3 => {
                if along >= segment_start - half_width
                    && along <= segment_end + half_width
                    && perpendicular <= half_width
                {
                    return Ok(true);
                }
            }
            _ => return Err("render.invalid-stroke"),
        }
    }
    Ok(false)
}

fn stroke_segments(
    length: f64,
    dash_offset: f64,
    dash: &[f64],
) -> Result<Vec<(f64, f64)>, &'static str> {
    if !length.is_finite() || length < 0.0 || !dash_offset.is_finite() {
        return Err("render.invalid-stroke");
    }
    if dash.is_empty() {
        return Ok((length > 0.0)
            .then_some((0.0, length))
            .into_iter()
            .collect());
    }
    let total = dash.iter().try_fold(0.0, |total, value| {
        if !value.is_finite() || *value < 0.0 {
            None
        } else {
            Some(total + value)
        }
    });
    let Some(total) = total.filter(|value| value.is_finite() && *value > 0.0) else {
        return Err("render.invalid-stroke");
    };
    if length == 0.0 {
        return Ok(Vec::new());
    }

    let mut index = 0usize;
    let mut consumed = dash_offset.rem_euclid(total);
    loop {
        let element = dash[index];
        if element > 0.0 && consumed < element {
            break;
        }
        if element > 0.0 {
            consumed = (consumed - element).max(0.0);
        }
        index = (index + 1) % dash.len();
        if consumed == 0.0 && dash[index] > 0.0 {
            break;
        }
    }

    let mut result = Vec::new();
    let mut distance = 0.0;
    while distance < length {
        let element = dash[index];
        if element == 0.0 {
            index = (index + 1) % dash.len();
            consumed = 0.0;
            continue;
        }
        let span = element - consumed;
        let remaining = length - distance;
        if span <= remaining {
            let end = distance + span;
            if index.is_multiple_of(2) && end > distance {
                result.push((distance, end));
            }
            distance = end;
            index = (index + 1) % dash.len();
            consumed = 0.0;
        } else {
            if index.is_multiple_of(2) {
                result.push((distance, length));
            }
            break;
        }
    }
    Ok(result)
}

fn polygon_contains(points: &[[f64; 2]], point: [f64; 2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    winding_contains(std::iter::once(points), 1, point)
}

fn path_contains(subpaths: &[PathSubpath], fill_rule: u16, point: [f64; 2]) -> bool {
    if !matches!(fill_rule, 1 | 2) {
        return false;
    }
    winding_contains(
        subpaths.iter().map(|subpath| subpath.points.as_slice()),
        fill_rule,
        point,
    )
}

fn winding_contains<'a>(
    polygons: impl IntoIterator<Item = &'a [[f64; 2]]>,
    fill_rule: u16,
    point: [f64; 2],
) -> bool {
    if !point.iter().all(|value| value.is_finite()) {
        return false;
    }
    let mut winding = 0i32;
    let mut crossings = 0u32;
    for points in polygons {
        if points.len() < 2 {
            continue;
        }
        let has_area = path_has_area(points);
        for index in 0..points.len() {
            let [x0, y0] = points[index];
            let [x1, y1] = points[(index + 1) % points.len()];
            if x0 == x1 && y0 == y1 {
                continue;
            }
            let cross = (x1 - x0) * (point[1] - y0) - (point[0] - x0) * (y1 - y0);
            if has_area
                && cross == 0.0
                && point[0] >= x0.min(x1)
                && point[0] <= x0.max(x1)
                && point[1] >= y0.min(y1)
                && point[1] <= y0.max(y1)
            {
                return true;
            }
            if y0 <= point[1] {
                if y1 > point[1] && cross > 0.0 {
                    if fill_rule == 1 {
                        winding += 1;
                    } else {
                        crossings ^= 1;
                    }
                }
            } else if y1 <= point[1] && cross < 0.0 {
                if fill_rule == 1 {
                    winding -= 1;
                } else {
                    crossings ^= 1;
                }
            }
        }
    }
    if fill_rule == 1 {
        winding != 0
    } else {
        crossings != 0
    }
}

fn path_has_area(points: &[[f64; 2]]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let first = points[0];
    points[1..].windows(2).any(|pair| {
        (pair[0][0] - first[0]) * (pair[1][1] - first[1])
            - (pair[0][1] - first[1]) * (pair[1][0] - first[0])
            != 0.0
    })
}

fn rounded_rect_contains(bounds: [f64; 4], radii: [f64; 4], point: [f64; 2]) -> bool {
    let [left, bottom, right, top] = bounds;
    let [top_left, top_right, bottom_right, bottom_left] = radii;
    let [x, y] = point;
    if right <= left || top <= bottom || x < left || x > right || y < bottom || y > top {
        return false;
    }
    if x < left + top_left && y > top - top_left {
        return ellipse_contains(
            [left + top_left, top - top_left],
            top_left,
            top_left,
            0.0,
            point,
        );
    }
    if x > right - top_right && y > top - top_right {
        return ellipse_contains(
            [right - top_right, top - top_right],
            top_right,
            top_right,
            0.0,
            point,
        );
    }
    if x > right - bottom_right && y < bottom + bottom_right {
        return ellipse_contains(
            [right - bottom_right, bottom + bottom_right],
            bottom_right,
            bottom_right,
            0.0,
            point,
        );
    }
    if x < left + bottom_left && y < bottom + bottom_left {
        return ellipse_contains(
            [left + bottom_left, bottom + bottom_left],
            bottom_left,
            bottom_left,
            0.0,
            point,
        );
    }
    true
}

fn ellipse_contains(
    center: [f64; 2],
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    point: [f64; 2],
) -> bool {
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return false;
    }
    let (sin, cos) = rotation.sin_cos();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let local_x = (dx * cos) + (dy * sin);
    let local_y = (-dx * sin) + (dy * cos);
    let x = local_x / radius_x;
    let y = local_y / radius_y;
    (x * x) + (y * y) <= 1.0
}

fn composite_premultiplied(
    destination: &mut [f64; 4],
    source: [f64; 4],
    composite: u16,
) -> Result<(), &'static str> {
    match composite {
        1 => {
            let inverse_source_alpha = 1.0 - source[3];
            for component in 0..3 {
                destination[component] =
                    source[component] + destination[component] * inverse_source_alpha;
            }
            destination[3] = source[3] + destination[3] * inverse_source_alpha;
        }
        2 => *destination = source,
        3 => {
            for component in 0..4 {
                destination[component] =
                    (source[component] + destination[component]).clamp(0.0, 1.0);
            }
        }
        4 | 5 => {
            let source_alpha = source[3];
            let destination_alpha = destination[3];
            let alpha = source_alpha + destination_alpha - source_alpha * destination_alpha;
            for component in 0..3 {
                let source_color = if source_alpha == 0.0 {
                    0.0
                } else {
                    source[component] / source_alpha
                };
                let destination_color = if destination_alpha == 0.0 {
                    0.0
                } else {
                    destination[component] / destination_alpha
                };
                let blend = if composite == 4 {
                    source_color * destination_color
                } else {
                    source_color + destination_color - source_color * destination_color
                };
                destination[component] = source[component] * (1.0 - destination_alpha)
                    + destination[component] * (1.0 - source_alpha)
                    + source_alpha * destination_alpha * blend;
            }
            destination[3] = alpha;
        }
        _ => return Err("render.invalid-composite"),
    }
    for component in destination {
        *component = component.clamp(0.0, 1.0);
    }
    Ok(())
}

/// Rasterize supported fill and Line stroke geometry to tightly packed RGBA8 bytes.
///
/// The Render 1.0 reference sample grid is used for Rect, RoundedRect, Circle,
/// Ellipse, Line, Polyline, Polygon, and flattened Path geometry; text coverage remains
/// outside this bounded path.
pub fn rasterize_solid_rgba8(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_at(chart, 0.0, width, height)
}

/// Rasterize the bounded surface at one chart-time query point.
pub fn rasterize_solid_rgba8_at(
    chart: &DecodedRenderChart,
    chart_time: f64,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_with_limits_at(chart, chart_time, width, height, &RenderLimits::default())
}

pub fn rasterize_solid_rgba8_with_limits(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
    limits: &RenderLimits,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_with_limits_at(chart, 0.0, width, height, limits)
}

pub fn rasterize_solid_rgba8_with_limits_at(
    chart: &DecodedRenderChart,
    chart_time: f64,
    width: u32,
    height: u32,
    limits: &RenderLimits,
) -> Result<Vec<u8>, &'static str> {
    if width == 0
        || height == 0
        || width > limits.max_raster_dimension
        || height > limits.max_raster_dimension
    {
        return Err("render.limit-exceeded");
    }
    let scene = evaluate_scene_at(chart, chart_time)?;
    let mut raster_ops = Vec::with_capacity(scene.ops.len());
    for op in &scene.ops {
        if !matches!(
            op.kind,
            NodeKind::Rect
                | NodeKind::RoundedRect
                | NodeKind::Circle
                | NodeKind::Ellipse
                | NodeKind::Line
                | NodeKind::Polyline
                | NodeKind::Polygon
                | NodeKind::Path
                | NodeKind::Text
                | NodeKind::Image
        ) {
            raster_ops.push(None);
            continue;
        }
        if !op.opacity.is_finite() {
            return Err("render.invalid-descriptor");
        }
        if !matches!(op.composite, 1..=5) {
            return Err("render.invalid-composite");
        }
        let Some(shape) = scene.shapes.get(&op.node_id) else {
            raster_ops.push(None);
            continue;
        };
        let source = if op.kind == NodeKind::Image {
            Some(RasterSource::Image(
                op.image.ok_or("render.invalid-geometry")?,
            ))
        } else if op.kind == NodeKind::Line {
            // Render section 14.2 keeps a Line's fill paint null.
            None
        } else {
            raster_paint_source(
                op.fill_rgba,
                op.linear_gradient.clone(),
                op.radial_gradient.clone(),
                op.image_pattern,
            )?
        };
        // Render section 8.2 binds every stroke to a paint, so a node that declares a
        // stroke without a resolvable paint is invalid rather than merely unpainted.
        let stroke_source = if matches!(
            op.kind,
            NodeKind::Rect
                | NodeKind::Line
                | NodeKind::RoundedRect
                | NodeKind::Circle
                | NodeKind::Ellipse
                | NodeKind::Polyline
                | NodeKind::Polygon
                | NodeKind::Path
                | NodeKind::Text
        ) {
            match op.stroke.as_ref() {
                Some(stroke) => Some(
                    raster_paint_source(
                        stroke.fill_rgba,
                        stroke.linear_gradient.clone(),
                        stroke.radial_gradient.clone(),
                        stroke.image_pattern,
                    )?
                    .ok_or("render.invalid-stroke")?,
                ),
                // Render section 14.2 requires a Line stroke.
                None if op.kind == NodeKind::Line => return Err("render.invalid-stroke"),
                None => None,
            }
        } else {
            None
        };
        if source.is_none() && stroke_source.is_none() {
            raster_ops.push(None);
            continue;
        }
        let mut clips = Vec::new();
        for clip_id in &op.clip_chain {
            let clip = scene.clips.get(clip_id).ok_or("render.invalid-reference")?;
            if let Some(shape) = &clip.shape {
                clips.push(raster_shape(shape));
            }
        }
        raster_ops.push(Some(RasterOp {
            shape: raster_shape(shape),
            clips,
            source,
            stroke_source,
            stroke: op.stroke.clone(),
            opacity: op.opacity,
            composite: op.composite,
        }));
    }
    let capacity = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or("render.limit-exceeded")?;
    let mut out = Vec::with_capacity(capacity);
    let encode = |value: f64| {
        if chart.viewport_color_space == 2 {
            encode_srgb(value)
        } else {
            value
        }
    };
    for py in 0..height {
        for px in 0..width {
            let mut sum = [0.0; 4];
            for sy in 0..8 {
                for sx in 0..8 {
                    let device_x = f64::from(px) + (f64::from(sx) + 0.5) / 8.0;
                    let device_y = f64::from(py) + (f64::from(sy) + 0.5) / 8.0;
                    let logical_x = (device_x / f64::from(width) - 0.5) * chart.viewport_width;
                    let logical_y = (0.5 - device_y / f64::from(height)) * chart.viewport_height;
                    let point = [logical_x, logical_y];
                    let mut sample = [0.0; 4];
                    let mut isolation_stack = Vec::new();
                    for event in &scene.events {
                        match event {
                            SceneEvent::BeginIsolation(boundary) => {
                                if !boundary.opacity.is_finite()
                                    || !(0.0..=1.0).contains(&boundary.opacity)
                                    || !matches!(boundary.composite, 1..=5)
                                {
                                    return Err("render.invalid-composite");
                                }
                                isolation_stack.push(RasterIsolationBuffer {
                                    boundary: *boundary,
                                    color: [0.0; 4],
                                });
                            }
                            SceneEvent::EndIsolation(node_id) => {
                                let buffer =
                                    isolation_stack.pop().ok_or("render.invalid-composite")?;
                                if buffer.boundary.node_id != *node_id {
                                    return Err("render.invalid-composite");
                                }
                                let source = buffer
                                    .color
                                    .map(|component| component * buffer.boundary.opacity);
                                composite_raster_sample(
                                    &mut sample,
                                    &mut isolation_stack,
                                    source,
                                    buffer.boundary.composite,
                                )?;
                            }
                            SceneEvent::Draw(index) => {
                                let Some(op) = raster_ops
                                    .get(*index)
                                    .ok_or("render.invalid-graph")?
                                    .as_ref()
                                else {
                                    continue;
                                };
                                let Some(local_point) = raster_shape_local_point(&op.shape, point)
                                else {
                                    continue;
                                };
                                if !op
                                    .clips
                                    .iter()
                                    .all(|clip| raster_shape_contains(clip, point))
                                {
                                    continue;
                                }
                                // Render section 7 emits fill before stroke for the same node.
                                if let Some(source) = &op.source
                                    && local_shape_contains(&op.shape.shape, local_point)
                                    && let Some(value) =
                                        raster_source_at(chart, source, local_point, op.opacity)?
                                {
                                    composite_raster_sample(
                                        &mut sample,
                                        &mut isolation_stack,
                                        value,
                                        op.composite,
                                    )?;
                                }
                                if let Some(source) = &op.stroke_source
                                    && stroke_contains(
                                        &op.shape.shape,
                                        local_point,
                                        op.stroke.as_ref().ok_or("render.invalid-stroke")?,
                                    )?
                                    && let Some(value) =
                                        raster_source_at(chart, source, local_point, op.opacity)?
                                {
                                    composite_raster_sample(
                                        &mut sample,
                                        &mut isolation_stack,
                                        value,
                                        op.composite,
                                    )?;
                                }
                            }
                        }
                    }
                    if !isolation_stack.is_empty() {
                        return Err("render.invalid-composite");
                    }
                    for component in 0..4 {
                        sum[component] += sample[component];
                    }
                }
            }
            let alpha = (sum[3] / 64.0).clamp(0.0, 1.0);
            let straight = if alpha == 0.0 {
                [0.0; 3]
            } else {
                [
                    (sum[0] / 64.0 / alpha).clamp(0.0, 1.0),
                    (sum[1] / 64.0 / alpha).clamp(0.0, 1.0),
                    (sum[2] / 64.0 / alpha).clamp(0.0, 1.0),
                ]
            };
            out.extend_from_slice(&[
                round_ties_to_even(encode(straight[0]) * 255.0),
                round_ties_to_even(encode(straight[1]) * 255.0),
                round_ties_to_even(encode(straight[2]) * 255.0),
                round_ties_to_even(alpha * 255.0),
            ]);
        }
    }
    Ok(out)
}

fn raster_paint_source(
    fill_rgba: Option<[f64; 4]>,
    linear_gradient: Option<LinearGradientDrawOp>,
    radial_gradient: Option<RadialGradientDrawOp>,
    image_pattern: Option<ImagePatternDrawOp>,
) -> Result<Option<RasterSource>, &'static str> {
    if let Some(gradient) = linear_gradient {
        return Ok(Some(RasterSource::LinearGradient(gradient)));
    }
    if let Some(gradient) = radial_gradient {
        return Ok(Some(RasterSource::RadialGradient(gradient)));
    }
    if let Some(pattern) = image_pattern {
        return Ok(Some(RasterSource::ImagePattern(pattern)));
    }
    let Some(fill) = fill_rgba else {
        return Ok(None);
    };
    if fill.iter().any(|value| !value.is_finite()) {
        return Err("render.invalid-descriptor");
    }
    let alpha = fill[3].clamp(0.0, 1.0);
    Ok(Some(RasterSource::Solid([
        fill[0].clamp(0.0, 1.0) * alpha,
        fill[1].clamp(0.0, 1.0) * alpha,
        fill[2].clamp(0.0, 1.0) * alpha,
        alpha,
    ])))
}

fn raster_source_at(
    chart: &DecodedRenderChart,
    source: &RasterSource,
    local_point: [f64; 2],
    opacity: f64,
) -> Result<Option<[f64; 4]>, &'static str> {
    let mut value = match source {
        RasterSource::Solid(value) => *value,
        RasterSource::LinearGradient(gradient) => gradient_color(gradient, local_point)?,
        RasterSource::RadialGradient(gradient) => radial_gradient_color(gradient, local_point)?,
        RasterSource::ImagePattern(pattern) => {
            let Some(value) = sample_image_pattern(chart, *pattern, local_point)? else {
                return Ok(None);
            };
            value
        }
        RasterSource::Image(image) => {
            let Some(value) = sample_image(chart, *image, local_point)? else {
                return Ok(None);
            };
            value
        }
    };
    for component in &mut value {
        *component *= opacity;
    }
    Ok(Some(value))
}

fn gradient_color(
    gradient: &LinearGradientDrawOp,
    point: [f64; 2],
) -> Result<[f64; 4], &'static str> {
    let dx = gradient.end[0] - gradient.start[0];
    let dy = gradient.end[1] - gradient.start[1];
    let denominator = (dx * dx) + (dy * dy);
    let t = if denominator == 0.0 {
        0.0
    } else {
        ((point[0] - gradient.start[0]) * dx + (point[1] - gradient.start[1]) * dy) / denominator
    };
    if !t.is_finite() {
        return Err("render.invalid-paint");
    }
    gradient_color_at_t(t, gradient.spread, &gradient.stops)
}

fn radial_gradient_color(
    gradient: &RadialGradientDrawOp,
    point: [f64; 2],
) -> Result<[f64; 4], &'static str> {
    let ux = point[0] - gradient.start_center[0];
    let uy = point[1] - gradient.start_center[1];
    let vx = gradient.end_center[0] - gradient.start_center[0];
    let vy = gradient.end_center[1] - gradient.start_center[1];
    let dr = gradient.end_radius - gradient.start_radius;
    let a = ((vx * vx) + (vy * vy)) - (dr * dr);
    let b = -2.0 * (((ux * vx) + (uy * vy)) + (gradient.start_radius * dr));
    let c = ((ux * ux) + (uy * uy)) - (gradient.start_radius * gradient.start_radius);
    if !a.is_finite() || !b.is_finite() || !c.is_finite() {
        return Ok([0.0; 4]);
    }

    let mut selected = None;
    let mut consider = |candidate: f64| {
        if !candidate.is_finite() {
            return;
        }
        let radius = gradient.start_radius + (candidate * dr);
        if radius.is_finite() && radius >= 0.0 {
            selected = Some(selected.map_or(candidate, |current: f64| current.max(candidate)));
        }
    };
    if a == 0.0 {
        if b != 0.0 {
            consider(-c / b);
        } else if c == 0.0 {
            consider(0.0);
        }
    } else {
        let discriminant = (b * b) - ((4.0 * a) * c);
        if discriminant.is_finite() && discriminant >= 0.0 {
            if discriminant == 0.0 {
                consider((-b) / (2.0 * a));
            } else {
                let root = discriminant.sqrt();
                consider(((-b) - root) / (2.0 * a));
                consider(((-b) + root) / (2.0 * a));
            }
        }
    }
    let Some(t) = selected else {
        return Ok([0.0; 4]);
    };
    gradient_color_at_t(t, gradient.spread, &gradient.stops)
}

fn gradient_color_at_t(
    mut t: f64,
    spread: u16,
    stops: &[GradientStopDrawOp],
) -> Result<[f64; 4], &'static str> {
    t = match spread {
        1 => t.clamp(0.0, 1.0),
        2 => t.rem_euclid(1.0),
        3 => {
            let reflected = t.rem_euclid(2.0);
            if reflected > 1.0 {
                2.0 - reflected
            } else {
                reflected
            }
        }
        _ => return Err("render.invalid-paint"),
    };
    let first = stops.first().ok_or("render.invalid-paint")?;
    if t < first.offset {
        return Ok(premultiply(first.color));
    }
    let mut left = 0;
    for (index, stop) in stops.iter().enumerate() {
        if stop.offset <= t {
            left = index;
        } else {
            break;
        }
    }
    let mut right = left + 1;
    while right < stops.len() && stops[right].offset == stops[left].offset {
        right += 1;
    }
    if right == stops.len() {
        return Ok(premultiply(stops[left].color));
    }
    let left_stop = &stops[left];
    let right_stop = &stops[right];
    let ratio = (t - left_stop.offset) / (right_stop.offset - left_stop.offset);
    let mut color = [0.0; 4];
    for (component, value) in color.iter_mut().enumerate() {
        *value = left_stop.color[component]
            + (right_stop.color[component] - left_stop.color[component]) * ratio;
    }
    Ok(premultiply(color))
}

fn premultiply(color: [f64; 4]) -> [f64; 4] {
    [
        color[0] * color[3],
        color[1] * color[3],
        color[2] * color[3],
        color[3],
    ]
}

fn pattern_local_point(pattern: ImagePatternDrawOp, point: [f64; 2]) -> Option<[f64; 2]> {
    let mut matrix = translation_matrix(pattern.position[0], pattern.position[1]);
    matrix = multiply_matrix(
        matrix,
        translation_matrix(pattern.origin[0], pattern.origin[1]),
    )
    .ok()?;
    matrix = multiply_matrix(matrix, rotation_matrix(pattern.rotation)).ok()?;
    matrix = multiply_matrix(matrix, scale_matrix(pattern.scale[0], pattern.scale[1])).ok()?;
    matrix = multiply_matrix(
        matrix,
        translation_matrix(-pattern.origin[0], -pattern.origin[1]),
    )
    .ok()?;
    transform_point(inverse_affine(matrix)?, point).ok()
}

fn pattern_repeat_axes(repeat: u16) -> Result<(bool, bool), &'static str> {
    match repeat {
        1 => Ok((false, false)),
        2 => Ok((true, false)),
        3 => Ok((false, true)),
        4 => Ok((true, true)),
        _ => Err("render.invalid-paint"),
    }
}

fn pattern_texel_index(coordinate: f64, dimension: u32, repeat: bool) -> Option<u32> {
    if !coordinate.is_finite() || dimension == 0 {
        return None;
    }
    let extent = f64::from(dimension);
    if repeat {
        Some(coordinate.rem_euclid(extent).floor() as u32)
    } else if (0.0..extent).contains(&coordinate) {
        Some(coordinate as u32)
    } else {
        None
    }
}

fn sample_image_pattern(
    chart: &DecodedRenderChart,
    pattern: ImagePatternDrawOp,
    point: [f64; 2],
) -> Result<Option<[f64; 4]>, &'static str> {
    let (repeat_x, repeat_y) = pattern_repeat_axes(pattern.repeat)?;
    let local_point = pattern_local_point(pattern, point);
    let Some([x, y]) = local_point else {
        return Ok(None);
    };
    if !x.is_finite() || !y.is_finite() {
        return Err("render.invalid-geometry");
    }
    let decoded = chart
        .decoded_images
        .get(&pattern.resource_id)
        .ok_or("render.resource-not-found")?;
    if (!repeat_x && !(0.0..f64::from(decoded.width)).contains(&x))
        || (!repeat_y && !(0.0..f64::from(decoded.height)).contains(&y))
    {
        return Ok(None);
    }
    match pattern.sampling {
        1 => {
            let Some(x) = pattern_texel_index(x.floor(), decoded.width, repeat_x) else {
                return Ok(None);
            };
            let Some(y) = pattern_texel_index(y.floor(), decoded.height, repeat_y) else {
                return Ok(None);
            };
            Ok(Some(image_texel(decoded, x, y)))
        }
        2 => {
            let fractional_x = x - 0.5;
            let fractional_y = y - 0.5;
            let base_x = fractional_x.floor();
            let base_y = fractional_y.floor();
            let tx = (fractional_x - base_x).clamp(0.0, 1.0);
            let ty = (fractional_y - base_y).clamp(0.0, 1.0);
            let x0 = pattern_texel_index(base_x, decoded.width, repeat_x);
            let x1 = pattern_texel_index(base_x + 1.0, decoded.width, repeat_x);
            let y0 = pattern_texel_index(base_y, decoded.height, repeat_y);
            let y1 = pattern_texel_index(base_y + 1.0, decoded.height, repeat_y);
            let texel = |x: Option<u32>, y: Option<u32>| {
                x.zip(y)
                    .map_or([0.0; 4], |(x, y)| image_texel(decoded, x, y))
            };
            let top_left = texel(x0, y0);
            let top_right = texel(x1, y0);
            let bottom_left = texel(x0, y1);
            let bottom_right = texel(x1, y1);
            let mut value = [0.0; 4];
            for component in 0..4 {
                let top = top_left[component] + (top_right[component] - top_left[component]) * tx;
                let bottom = bottom_left[component]
                    + (bottom_right[component] - bottom_left[component]) * tx;
                value[component] = top + (bottom - top) * ty;
            }
            Ok(Some(value))
        }
        _ => Err("render.invalid-geometry"),
    }
}

fn sample_image(
    chart: &DecodedRenderChart,
    image: ImageDrawOp,
    point: [f64; 2],
) -> Result<Option<[f64; 4]>, &'static str> {
    let [x, y, width, height] = image.destination;
    if width == 0.0
        || height == 0.0
        || image.source[2] == 0.0
        || image.source[3] == 0.0
        || point[0] < x
        || point[0] >= x + width
        || point[1] < y
        || point[1] >= y + height
    {
        return Ok(None);
    }
    let u = (point[0] - x) / width;
    let v = (point[1] - y) / height;
    let source_x = image.source[0] + (u * image.source[2]);
    let source_y = image.source[1] + ((1.0 - v) * image.source[3]);
    let decoded = chart
        .decoded_images
        .get(&image.resource_id)
        .ok_or("render.resource-not-found")?;
    match image.sampling {
        1 => {
            let x_range = source_texel_range(image.source[0], image.source[2], decoded.width)?;
            let y_range = source_texel_range(image.source[1], image.source[3], decoded.height)?;
            Ok(Some(image_texel(
                decoded,
                (source_x.floor() as i64).clamp(i64::from(x_range.0), i64::from(x_range.1)) as u32,
                (source_y.floor() as i64).clamp(i64::from(y_range.0), i64::from(y_range.1)) as u32,
            )))
        }
        2 => {
            let (x0, x1, tx) =
                linear_axis(source_x, image.source[0], image.source[2], decoded.width)?;
            let (y0, y1, ty) =
                linear_axis(source_y, image.source[1], image.source[3], decoded.height)?;
            let top_left = image_texel(decoded, x0, y0);
            let top_right = image_texel(decoded, x1, y0);
            let bottom_left = image_texel(decoded, x0, y1);
            let bottom_right = image_texel(decoded, x1, y1);
            let mut value = [0.0; 4];
            for component in 0..4 {
                let top = top_left[component] + (top_right[component] - top_left[component]) * tx;
                let bottom = bottom_left[component]
                    + (bottom_right[component] - bottom_left[component]) * tx;
                value[component] = top + (bottom - top) * ty;
            }
            Ok(Some(value))
        }
        _ => Err("render.invalid-geometry"),
    }
}

fn source_texel_range(origin: f64, size: f64, dimension: u32) -> Result<(u32, u32), &'static str> {
    if dimension == 0 || size <= 0.0 {
        return Err("render.invalid-geometry");
    }
    let first = (origin - 0.5).ceil().max(0.0) as u32;
    let last = (origin + size - 0.5).floor().min(f64::from(dimension - 1)) as u32;
    (first <= last)
        .then_some((first, last))
        .ok_or("render.invalid-geometry")
}

fn linear_axis(
    coordinate: f64,
    origin: f64,
    size: f64,
    dimension: u32,
) -> Result<(u32, u32, f64), &'static str> {
    let (first, last) = source_texel_range(origin, size, dimension)?;
    let fractional = coordinate - 0.5;
    let raw_base = fractional.floor() as i64;
    let fraction = (fractional - raw_base as f64).clamp(0.0, 1.0);
    let first = i64::from(first);
    let last = i64::from(last);
    let base = raw_base.clamp(first, last) as u32;
    let next = raw_base.saturating_add(1).clamp(first, last) as u32;
    Ok((base, next, fraction))
}

fn image_texel(image: &crate::assets::DecodedImage, x: u32, y: u32) -> [f64; 4] {
    image.linear_premultiplied[(y as usize * image.width as usize) + x as usize]
}

fn encode_srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn round_ties_to_even(value: f64) -> u8 {
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    rounded.clamp(0.0, 255.0) as u8
}

fn paint_rgba(
    chart: &DecodedRenderChart,
    paint: &PaintRecord,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<PaintParts, &'static str> {
    match &paint.data {
        // `colorDescriptor` is an FCBC descriptor index, not a constant-pool slot
        // (fcs-render.md sections 14.5 and 15.3); the loader already validated it as a
        // Color descriptor, so an unresolvable or wrongly-typed result is an invariant
        // violation. ImagePattern uses the same validated ResourceData binding as Image.
        PaintData::Solid { color } => {
            let evaluation = query_descriptor(&chart.core, *color, chart_time, environment)
                .map_err(|_| "render.invalid-descriptor")?;
            match evaluation.value {
                RuntimeValue::Color(rgba) => Ok((Some(rgba), None, None, None)),
                _ => Err("render.invalid-descriptor"),
            }
        }
        PaintData::LinearGradient {
            start,
            end,
            spread,
            stops,
        } => {
            let start = query_vec2_in(
                chart,
                *start,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let end = query_vec2_in(chart, *end, chart_time, ValueType::Vec2Length, environment)?;
            let stops = stops
                .iter()
                .map(|stop| {
                    let value =
                        query_value_in(chart, stop.color_descriptor, chart_time, environment)?;
                    let RuntimeValue::Color(color) = value else {
                        return Err("render.invalid-descriptor");
                    };
                    Ok(GradientStopDrawOp {
                        offset: stop.offset,
                        color,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                None,
                Some(LinearGradientDrawOp {
                    start,
                    end,
                    spread: *spread,
                    stops,
                }),
                None,
                None,
            ))
        }
        PaintData::RadialGradient {
            start_center,
            start_radius,
            end_center,
            end_radius,
            spread,
            stops,
        } => {
            let start_center = query_vec2_in(
                chart,
                *start_center,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let start_radius = query_scalar_in(
                chart,
                *start_radius,
                chart_time,
                ValueType::Length,
                environment,
            )?;
            let end_center = query_vec2_in(
                chart,
                *end_center,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let end_radius = query_scalar_in(
                chart,
                *end_radius,
                chart_time,
                ValueType::Length,
                environment,
            )?;
            if start_radius < 0.0 || end_radius < 0.0 {
                return Err("render.invalid-paint");
            }
            let stops = stops
                .iter()
                .map(|stop| {
                    let value =
                        query_value_in(chart, stop.color_descriptor, chart_time, environment)?;
                    let RuntimeValue::Color(color) = value else {
                        return Err("render.invalid-descriptor");
                    };
                    Ok(GradientStopDrawOp {
                        offset: stop.offset,
                        color,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                None,
                None,
                Some(RadialGradientDrawOp {
                    start_center,
                    start_radius,
                    end_center,
                    end_radius,
                    spread: *spread,
                    stops,
                }),
                None,
            ))
        }
        PaintData::ImagePattern {
            resource_id,
            position,
            origin,
            rotation,
            scale,
            repeat,
            sampling,
        } => Ok((
            None,
            None,
            None,
            Some(ImagePatternDrawOp {
                resource_id: *resource_id,
                position: query_vec2_in(
                    chart,
                    *position,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?,
                origin: query_vec2_in(
                    chart,
                    *origin,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?,
                rotation: query_scalar_in(
                    chart,
                    *rotation,
                    chart_time,
                    ValueType::Angle,
                    environment,
                )?,
                scale: query_vec2_in(chart, *scale, chart_time, ValueType::Vec2Float, environment)?,
                repeat: *repeat,
                sampling: *sampling,
            }),
        )),
    }
}

const PATH_FLATTEN_TOLERANCE: f64 = 1.0 / 1024.0;
const PATH_FLATTEN_MAX_DEPTH: u8 = 32;
// ponytail: cap each Path's adaptive output; raise only with measured large-path fixtures.
const PATH_FLATTEN_MAX_POINTS: usize = 1 << 16;

type Point = [f64; 2];

#[derive(Clone, Copy)]
enum PathCurve {
    Quadratic {
        start: Point,
        control: Point,
        end: Point,
    },
    Cubic {
        start: Point,
        control1: Point,
        control2: Point,
        end: Point,
    },
    Arc {
        center: Point,
        radius: f64,
        start_angle: f64,
        end_angle: f64,
    },
    EllipseArc {
        center: Point,
        radius_x: f64,
        radius_y: f64,
        rotation: f64,
        start_angle: f64,
        end_angle: f64,
    },
}

impl PathCurve {
    fn point(self, parameter: f64) -> Result<Point, &'static str> {
        let point = match self {
            Self::Quadratic {
                start,
                control,
                end,
            } => {
                let first = lerp(start, control, parameter)?;
                let second = lerp(control, end, parameter)?;
                lerp(first, second, parameter)?
            }
            Self::Cubic {
                start,
                control1,
                control2,
                end,
            } => {
                let first = lerp(start, control1, parameter)?;
                let second = lerp(control1, control2, parameter)?;
                let third = lerp(control2, end, parameter)?;
                let first = lerp(first, second, parameter)?;
                let second = lerp(second, third, parameter)?;
                lerp(first, second, parameter)?
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => circle_arc_point(center, radius, start_angle, end_angle, parameter)?,
            Self::EllipseArc {
                center,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
            } => ellipse_arc_point(
                center,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
                parameter,
            )?,
        };
        finite_point(point)
    }

    fn flat_enough(self, world_matrix: [f64; 9]) -> Result<bool, &'static str> {
        let start = transform_point(world_matrix, self.point(0.0)?)?;
        let end = transform_point(world_matrix, self.point(1.0)?)?;
        let flatness = match self {
            Self::Quadratic { control, .. } => {
                distance_to_chord(transform_point(world_matrix, control)?, start, end)?
            }
            Self::Cubic {
                control1, control2, ..
            } => distance_to_chord(transform_point(world_matrix, control1)?, start, end)?.max(
                distance_to_chord(transform_point(world_matrix, control2)?, start, end)?,
            ),
            Self::Arc {
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                let sweep = (end_angle - start_angle).abs();
                if sweep == 0.0 || radius == 0.0 {
                    0.0
                } else if sweep > std::f64::consts::PI {
                    return Ok(false);
                } else {
                    distance_to_chord(transform_point(world_matrix, self.point(0.5)?)?, start, end)?
                }
            }
            Self::EllipseArc {
                radius_x,
                radius_y,
                start_angle,
                end_angle,
                ..
            } => {
                let sweep = (end_angle - start_angle).abs();
                if sweep == 0.0 || (radius_x == 0.0 && radius_y == 0.0) {
                    0.0
                } else if sweep > std::f64::consts::PI {
                    return Ok(false);
                } else {
                    distance_to_chord(transform_point(world_matrix, self.point(0.5)?)?, start, end)?
                }
            }
        };
        if flatness.is_finite() {
            Ok(flatness <= PATH_FLATTEN_TOLERANCE)
        } else {
            Err("render.invalid-geometry")
        }
    }

    fn stroke_length(self) -> Result<f64, &'static str> {
        let length = match self {
            Self::Arc {
                radius,
                start_angle,
                end_angle,
                ..
            } => (end_angle - start_angle).abs() * radius,
            _ => {
                let start = self.point(0.0)?;
                let end = self.point(1.0)?;
                (end[0] - start[0]).hypot(end[1] - start[1])
            }
        };
        if length.is_finite() && length >= 0.0 {
            Ok(length)
        } else {
            Err("render.invalid-geometry")
        }
    }

    fn split(self) -> Result<(Self, Self), &'static str> {
        let halves = match self {
            Self::Quadratic {
                start,
                control,
                end,
            } => {
                let first = midpoint(start, control)?;
                let second = midpoint(control, end)?;
                let middle = midpoint(first, second)?;
                (
                    Self::Quadratic {
                        start,
                        control: first,
                        end: middle,
                    },
                    Self::Quadratic {
                        start: middle,
                        control: second,
                        end,
                    },
                )
            }
            Self::Cubic {
                start,
                control1,
                control2,
                end,
            } => {
                let first = midpoint(start, control1)?;
                let second = midpoint(control1, control2)?;
                let third = midpoint(control2, end)?;
                let fourth = midpoint(first, second)?;
                let fifth = midpoint(second, third)?;
                let middle = midpoint(fourth, fifth)?;
                (
                    Self::Cubic {
                        start,
                        control1: first,
                        control2: fourth,
                        end: middle,
                    },
                    Self::Cubic {
                        start: middle,
                        control1: fifth,
                        control2: third,
                        end,
                    },
                )
            }
            Self::Arc {
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                let middle = midpoint_scalar(start_angle, end_angle)?;
                (
                    Self::Arc {
                        center,
                        radius,
                        start_angle,
                        end_angle: middle,
                    },
                    Self::Arc {
                        center,
                        radius,
                        start_angle: middle,
                        end_angle,
                    },
                )
            }
            Self::EllipseArc {
                center,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
            } => {
                let middle = midpoint_scalar(start_angle, end_angle)?;
                (
                    Self::EllipseArc {
                        center,
                        radius_x,
                        radius_y,
                        rotation,
                        start_angle,
                        end_angle: middle,
                    },
                    Self::EllipseArc {
                        center,
                        radius_x,
                        radius_y,
                        rotation,
                        start_angle: middle,
                        end_angle,
                    },
                )
            }
        };
        Ok(halves)
    }
}

fn flatten_curve(
    curve: PathCurve,
    points: &mut Vec<Point>,
    segment_lengths: &mut Vec<f64>,
    depth: u8,
    world_matrix: [f64; 9],
    point_count: &mut usize,
) -> Result<(), &'static str> {
    if *point_count >= PATH_FLATTEN_MAX_POINTS {
        return Err("render.limit-exceeded");
    }
    if curve.flat_enough(world_matrix)? {
        let point = curve.point(1.0)?;
        let segment_length = curve.stroke_length()?;
        points.push(point);
        segment_lengths.push(segment_length);
        *point_count += 1;
        return Ok(());
    }
    if depth >= PATH_FLATTEN_MAX_DEPTH {
        return Err("render.limit-exceeded");
    }
    let (left, right) = curve.split()?;
    flatten_curve(
        left,
        points,
        segment_lengths,
        depth + 1,
        world_matrix,
        point_count,
    )?;
    flatten_curve(
        right,
        points,
        segment_lengths,
        depth + 1,
        world_matrix,
        point_count,
    )
}

fn evaluate_path(
    chart: &DecodedRenderChart,
    path: &PathRecord,
    chart_time: f64,
    environment: EvaluationEnvironment,
    world_matrix: [f64; 9],
) -> Result<(Vec<PathSubpath>, [f64; 4]), &'static str> {
    if !matches!(path.fill_rule, 1 | 2) {
        return Err("render.invalid-geometry");
    }
    let mut subpaths = Vec::new();
    let mut active = None;
    let mut current = [0.0; 2];
    let mut start = [0.0; 2];
    let mut bounds = None;
    let mut point_count = 0;
    let mut has_drawing = false;

    for command in &path.commands {
        match command {
            PathCommand::MoveTo(point) => {
                if let Some(subpath) = active.take() {
                    subpaths.push(subpath);
                }
                let point = query_vec2_in(
                    chart,
                    *point,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                update_bounds(&mut bounds, point);
                claim_path_point(&mut point_count)?;
                active = Some(PathSubpath {
                    points: vec![point],
                    segment_lengths: Vec::new(),
                    joins_after: Vec::new(),
                    closed: false,
                });
                current = point;
                start = point;
                has_drawing = false;
            }
            PathCommand::LineTo(point) => {
                let point = query_vec2_in(
                    chart,
                    *point,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                append_path_point(active.as_mut(), point, &mut bounds, &mut point_count)?;
                current = point;
                has_drawing = true;
            }
            PathCommand::QuadraticTo(control, end) => {
                let control = query_vec2_in(
                    chart,
                    *control,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                let end =
                    query_vec2_in(chart, *end, chart_time, ValueType::Vec2Length, environment)?;
                append_curve(
                    active.as_mut(),
                    PathCurve::Quadratic {
                        start: current,
                        control,
                        end,
                    },
                    &mut bounds,
                    world_matrix,
                    &mut point_count,
                )?;
                current = end;
                has_drawing = true;
            }
            PathCommand::CubicTo(control1, control2, end) => {
                let control1 = query_vec2_in(
                    chart,
                    *control1,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                let control2 = query_vec2_in(
                    chart,
                    *control2,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                let end =
                    query_vec2_in(chart, *end, chart_time, ValueType::Vec2Length, environment)?;
                append_curve(
                    active.as_mut(),
                    PathCurve::Cubic {
                        start: current,
                        control1,
                        control2,
                        end,
                    },
                    &mut bounds,
                    world_matrix,
                    &mut point_count,
                )?;
                current = end;
                has_drawing = true;
            }
            PathCommand::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                direction,
            } => {
                let center = query_vec2_in(
                    chart,
                    *center,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                let radius =
                    query_scalar_in(chart, *radius, chart_time, ValueType::Length, environment)?;
                let start_angle = query_scalar_in(
                    chart,
                    *start_angle,
                    chart_time,
                    ValueType::Angle,
                    environment,
                )?;
                let end_angle =
                    query_scalar_in(chart, *end_angle, chart_time, ValueType::Angle, environment)?;
                validate_arc_angles(start_angle, end_angle, *direction, radius)?;
                let curve = PathCurve::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                };
                append_path_point(
                    active.as_mut(),
                    curve.point(0.0)?,
                    &mut bounds,
                    &mut point_count,
                )?;
                append_curve(
                    active.as_mut(),
                    curve,
                    &mut bounds,
                    world_matrix,
                    &mut point_count,
                )?;
                current = curve.point(1.0)?;
                has_drawing = true;
            }
            PathCommand::EllipseArc {
                center,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
                direction,
            } => {
                let center = query_vec2_in(
                    chart,
                    *center,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?;
                let radius_x =
                    query_scalar_in(chart, *radius_x, chart_time, ValueType::Length, environment)?;
                let radius_y =
                    query_scalar_in(chart, *radius_y, chart_time, ValueType::Length, environment)?;
                let rotation =
                    query_scalar_in(chart, *rotation, chart_time, ValueType::Angle, environment)?;
                let start_angle = query_scalar_in(
                    chart,
                    *start_angle,
                    chart_time,
                    ValueType::Angle,
                    environment,
                )?;
                let end_angle =
                    query_scalar_in(chart, *end_angle, chart_time, ValueType::Angle, environment)?;
                if radius_x < 0.0 || radius_y < 0.0 {
                    return Err("render.invalid-geometry");
                }
                validate_arc_angles(start_angle, end_angle, *direction, radius_x.max(radius_y))?;
                let curve = PathCurve::EllipseArc {
                    center,
                    radius_x,
                    radius_y,
                    rotation,
                    start_angle,
                    end_angle,
                };
                append_path_point(
                    active.as_mut(),
                    curve.point(0.0)?,
                    &mut bounds,
                    &mut point_count,
                )?;
                append_curve(
                    active.as_mut(),
                    curve,
                    &mut bounds,
                    world_matrix,
                    &mut point_count,
                )?;
                current = curve.point(1.0)?;
                has_drawing = true;
            }
            PathCommand::Close => {
                if !has_drawing || active.as_ref().is_some_and(|subpath| subpath.closed) {
                    return Err("render.invalid-geometry");
                }
                append_path_point(active.as_mut(), start, &mut bounds, &mut point_count)?;
                active.as_mut().ok_or("render.invalid-geometry")?.closed = true;
                current = start;
            }
        }
    }
    if let Some(subpath) = active {
        subpaths.push(subpath);
    }
    Ok((subpaths, bounds.unwrap_or([0.0; 4])))
}

fn append_curve(
    active: Option<&mut PathSubpath>,
    curve: PathCurve,
    bounds: &mut Option<[f64; 4]>,
    world_matrix: [f64; 9],
    point_count: &mut usize,
) -> Result<(), &'static str> {
    let subpath = active.ok_or("render.invalid-geometry")?;
    subpath.closed = false;
    let point_start = subpath.points.len();
    let segment_start = subpath.segment_lengths.len();
    flatten_curve(
        curve,
        &mut subpath.points,
        &mut subpath.segment_lengths,
        0,
        world_matrix,
        point_count,
    )?;
    subpath
        .joins_after
        .resize(subpath.segment_lengths.len(), false);
    if subpath.segment_lengths.len() > segment_start {
        *subpath.joins_after.last_mut().expect("curve segment") = true;
    }
    for point in &subpath.points[point_start..] {
        update_bounds(bounds, *point);
    }
    Ok(())
}

fn append_path_point(
    active: Option<&mut PathSubpath>,
    point: Point,
    bounds: &mut Option<[f64; 4]>,
    point_count: &mut usize,
) -> Result<(), &'static str> {
    let subpath = active.ok_or("render.invalid-geometry")?;
    subpath.closed = false;
    let point = finite_point(point)?;
    let start = *subpath.points.last().ok_or("render.invalid-geometry")?;
    let segment_length = (point[0] - start[0]).hypot(point[1] - start[1]);
    if !segment_length.is_finite() {
        return Err("render.invalid-geometry");
    }
    claim_path_point(point_count)?;
    subpath.points.push(point);
    subpath.segment_lengths.push(segment_length);
    subpath.joins_after.push(true);
    update_bounds(bounds, point);
    Ok(())
}

fn ellipse_stroke_path(
    center: Point,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    world_matrix: [f64; 9],
) -> Result<PathSubpath, &'static str> {
    let mut points = vec![ellipse_arc_point(
        center, radius_x, radius_y, rotation, 0.0, 0.0, 0.0,
    )?];
    let mut segment_lengths = Vec::new();
    let mut point_count = 1;
    for (start_angle, end_angle) in [
        (0.0, -std::f64::consts::FRAC_PI_2),
        (-std::f64::consts::FRAC_PI_2, -std::f64::consts::PI),
        (-std::f64::consts::PI, -3.0 * std::f64::consts::FRAC_PI_2),
        (-3.0 * std::f64::consts::FRAC_PI_2, -std::f64::consts::TAU),
    ] {
        flatten_curve(
            PathCurve::EllipseArc {
                center,
                radius_x,
                radius_y,
                rotation,
                start_angle,
                end_angle,
            },
            &mut points,
            &mut segment_lengths,
            0,
            world_matrix,
            &mut point_count,
        )?;
    }
    Ok(PathSubpath {
        joins_after: vec![false; segment_lengths.len()],
        points,
        segment_lengths,
        closed: true,
    })
}

fn rounded_rect_stroke_path(
    bounds: [f64; 4],
    radii: [f64; 4],
    world_matrix: [f64; 9],
) -> Result<PathSubpath, &'static str> {
    let [left, bottom, right, top] = bounds;
    let [top_left, top_right, bottom_right, bottom_left] = radii;
    let mut subpath = PathSubpath {
        points: vec![[left + top_left, top]],
        segment_lengths: Vec::new(),
        joins_after: Vec::new(),
        closed: false,
    };
    let mut bounds = None;
    let mut point_count = 1;
    for (line_end, arc) in [
        (
            [right - top_right, top],
            PathCurve::EllipseArc {
                center: [right - top_right, top - top_right],
                radius_x: top_right,
                radius_y: top_right,
                rotation: 0.0,
                start_angle: std::f64::consts::FRAC_PI_2,
                end_angle: 0.0,
            },
        ),
        (
            [right, bottom + bottom_right],
            PathCurve::EllipseArc {
                center: [right - bottom_right, bottom + bottom_right],
                radius_x: bottom_right,
                radius_y: bottom_right,
                rotation: 0.0,
                start_angle: 0.0,
                end_angle: -std::f64::consts::FRAC_PI_2,
            },
        ),
        (
            [left + bottom_left, bottom],
            PathCurve::EllipseArc {
                center: [left + bottom_left, bottom + bottom_left],
                radius_x: bottom_left,
                radius_y: bottom_left,
                rotation: 0.0,
                start_angle: -std::f64::consts::FRAC_PI_2,
                end_angle: -std::f64::consts::PI,
            },
        ),
        (
            [left, top - top_left],
            PathCurve::EllipseArc {
                center: [left + top_left, top - top_left],
                radius_x: top_left,
                radius_y: top_left,
                rotation: 0.0,
                start_angle: std::f64::consts::PI,
                end_angle: std::f64::consts::FRAC_PI_2,
            },
        ),
    ] {
        append_path_point(Some(&mut subpath), line_end, &mut bounds, &mut point_count)?;
        append_curve(
            Some(&mut subpath),
            arc,
            &mut bounds,
            world_matrix,
            &mut point_count,
        )?;
    }
    subpath.closed = true;
    Ok(subpath)
}

fn claim_path_point(point_count: &mut usize) -> Result<(), &'static str> {
    if *point_count >= PATH_FLATTEN_MAX_POINTS {
        return Err("render.limit-exceeded");
    }
    *point_count += 1;
    Ok(())
}

fn update_bounds(bounds: &mut Option<[f64; 4]>, point: Point) {
    let [x, y] = point;
    if let Some(bounds) = bounds {
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
    } else {
        *bounds = Some([x, y, x, y]);
    }
}

fn validate_arc_angles(
    start_angle: f64,
    end_angle: f64,
    direction: u16,
    radius: f64,
) -> Result<(), &'static str> {
    let sweep = end_angle - start_angle;
    if !matches!(direction, 1 | 2)
        || radius < 0.0
        || !sweep.is_finite()
        || (direction == 1 && sweep > 0.0)
        || (direction == 2 && sweep < 0.0)
    {
        return Err("render.invalid-geometry");
    }
    Ok(())
}

fn circle_arc_point(
    center: Point,
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    parameter: f64,
) -> Result<Point, &'static str> {
    let angle = start_angle + (end_angle - start_angle) * parameter;
    let (sin, cos) = angle.sin_cos();
    finite_point([center[0] + radius * cos, center[1] + radius * sin])
}

fn ellipse_arc_point(
    center: Point,
    radius_x: f64,
    radius_y: f64,
    rotation: f64,
    start_angle: f64,
    end_angle: f64,
    parameter: f64,
) -> Result<Point, &'static str> {
    let angle = start_angle + (end_angle - start_angle) * parameter;
    let (sin, cos) = angle.sin_cos();
    let (rotation_sin, rotation_cos) = rotation.sin_cos();
    let x = (radius_x * cos) * rotation_cos - (radius_y * sin) * rotation_sin;
    let y = (radius_x * cos) * rotation_sin + (radius_y * sin) * rotation_cos;
    finite_point([center[0] + x, center[1] + y])
}

fn finite_point(point: Point) -> Result<Point, &'static str> {
    point
        .iter()
        .all(|value| value.is_finite())
        .then_some(point)
        .ok_or("render.invalid-geometry")
}

fn lerp(start: Point, end: Point, parameter: f64) -> Result<Point, &'static str> {
    finite_point([
        start[0] + (end[0] - start[0]) * parameter,
        start[1] + (end[1] - start[1]) * parameter,
    ])
}

fn midpoint(start: Point, end: Point) -> Result<Point, &'static str> {
    finite_point([
        start[0] + (end[0] - start[0]) * 0.5,
        start[1] + (end[1] - start[1]) * 0.5,
    ])
}

fn midpoint_scalar(start: f64, end: f64) -> Result<f64, &'static str> {
    let middle = start + (end - start) * 0.5;
    middle
        .is_finite()
        .then_some(middle)
        .ok_or("render.invalid-geometry")
}

fn distance_to_chord(point: Point, start: Point, end: Point) -> Result<f64, &'static str> {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if !length.is_finite() {
        return Err("render.invalid-geometry");
    }
    if length == 0.0 {
        let distance = (point[0] - start[0]).hypot(point[1] - start[1]);
        return distance
            .is_finite()
            .then_some(distance)
            .ok_or("render.invalid-geometry");
    }
    let projection = (((point[0] - start[0]) * dx) + ((point[1] - start[1]) * dy)) / length;
    if !projection.is_finite() {
        return Err("render.invalid-geometry");
    }
    let along = projection.clamp(0.0, length);
    let nearest = [
        start[0] + along * dx / length,
        start[1] + along * dy / length,
    ];
    let distance = (point[0] - nearest[0]).hypot(point[1] - nearest[1]);
    distance
        .is_finite()
        .then_some(distance)
        .ok_or("render.invalid-geometry")
}

struct GeometryEvaluation {
    world_bounds: [f64; 4],
    shape: Option<LocalShape>,
    image: Option<ImageDrawOp>,
}

fn geometry_evaluation(
    chart: &DecodedRenderChart,
    geometry_ref: Option<u32>,
    chart_time: f64,
    environment: EvaluationEnvironment,
    world_matrix: [f64; 9],
    needs_stroke: bool,
) -> Result<GeometryEvaluation, &'static str> {
    let Some(index) = geometry_ref else {
        return Ok(GeometryEvaluation {
            world_bounds: [0.0, 0.0, 0.0, 0.0],
            shape: None,
            image: None,
        });
    };
    let Some(geometry) = chart.geometries.get(index as usize) else {
        return Err("render.invalid-reference");
    };
    let (local_bounds, shape, image) = match &geometry.data {
        GeometryData::Rect { origin, size } => {
            let [x, y] = query_vec2_in(
                chart,
                *origin,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let [width, height] =
                query_vec2_in(chart, *size, chart_time, ValueType::Vec2Length, environment)?;
            let right = x + width;
            let bottom = y + height;
            if width < 0.0 || height < 0.0 || !right.is_finite() || !bottom.is_finite() {
                return Err("render.invalid-geometry");
            }
            (
                [x, y, right, bottom],
                Some(LocalShape::Rect {
                    bounds: [x, y, right, bottom],
                }),
                None,
            )
        }
        GeometryData::RoundedRect {
            origin,
            size,
            radii,
        } => {
            let [x, y] = query_vec2_in(
                chart,
                *origin,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let [width, height] =
                query_vec2_in(chart, *size, chart_time, ValueType::Vec2Length, environment)?;
            if width < 0.0 || height < 0.0 {
                return Err("render.invalid-geometry");
            }
            let mut values = [0.0; 4];
            for (value, descriptor) in values.iter_mut().zip(radii) {
                *value = query_scalar_in(
                    chart,
                    *descriptor,
                    chart_time,
                    ValueType::Length,
                    environment,
                )?;
                if *value < 0.0 {
                    return Err("render.invalid-geometry");
                }
            }
            let scale = rounded_rect_scale(width, height, values);
            values.iter_mut().for_each(|value| *value *= scale);
            let right = x + width;
            let top = y + height;
            if !right.is_finite() || !top.is_finite() || !scale.is_finite() {
                return Err("render.invalid-geometry");
            }
            (
                [x, y, right, top],
                Some(LocalShape::RoundedRect {
                    bounds: [x, y, right, top],
                    radii: values,
                    stroke_path: needs_stroke
                        .then(|| rounded_rect_stroke_path([x, y, right, top], values, world_matrix))
                        .transpose()?,
                }),
                None,
            )
        }
        GeometryData::Circle { center, radius } => {
            let center = query_vec2_in(
                chart,
                *center,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let radius =
                query_scalar_in(chart, *radius, chart_time, ValueType::Length, environment)?;
            if radius < 0.0 {
                return Err("render.invalid-geometry");
            }
            let bounds = [
                center[0] - radius,
                center[1] - radius,
                center[0] + radius,
                center[1] + radius,
            ];
            (bounds, Some(LocalShape::Circle { center, radius }), None)
        }
        GeometryData::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => {
            let center = query_vec2_in(
                chart,
                *center,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let radius_x =
                query_scalar_in(chart, *radius_x, chart_time, ValueType::Length, environment)?;
            let radius_y =
                query_scalar_in(chart, *radius_y, chart_time, ValueType::Length, environment)?;
            let rotation =
                query_scalar_in(chart, *rotation, chart_time, ValueType::Angle, environment)?;
            if radius_x < 0.0 || radius_y < 0.0 {
                return Err("render.invalid-geometry");
            }
            let (sin, cos) = rotation.sin_cos();
            let extent_x = ((radius_x * cos).powi(2) + (radius_y * sin).powi(2)).sqrt();
            let extent_y = ((radius_x * sin).powi(2) + (radius_y * cos).powi(2)).sqrt();
            let bounds = [
                center[0] - extent_x,
                center[1] - extent_y,
                center[0] + extent_x,
                center[1] + extent_y,
            ];
            (
                bounds,
                Some(LocalShape::Ellipse {
                    center,
                    radius_x,
                    radius_y,
                    rotation,
                    stroke_path: needs_stroke
                        .then(|| {
                            ellipse_stroke_path(center, radius_x, radius_y, rotation, world_matrix)
                        })
                        .transpose()?,
                }),
                None,
            )
        }
        GeometryData::Line { start, end } => {
            let start = query_vec2_in(
                chart,
                *start,
                chart_time,
                ValueType::Vec2Length,
                environment,
            )?;
            let end = query_vec2_in(chart, *end, chart_time, ValueType::Vec2Length, environment)?;
            if !start
                .iter()
                .chain(end.iter())
                .all(|value| value.is_finite())
            {
                return Err("render.invalid-geometry");
            }
            let bounds = [
                start[0].min(end[0]),
                start[1].min(end[1]),
                start[0].max(end[0]),
                start[1].max(end[1]),
            ];
            (bounds, Some(LocalShape::Line { start, end }), None)
        }
        GeometryData::Polyline { points } | GeometryData::Polygon { points } => {
            let closed = matches!(geometry.data, GeometryData::Polygon { .. });
            let mut values = Vec::with_capacity(points.len());
            for descriptor in points {
                values.push(query_vec2_in(
                    chart,
                    *descriptor,
                    chart_time,
                    ValueType::Vec2Length,
                    environment,
                )?);
            }
            if values.len() < 2 {
                return Err("render.invalid-geometry");
            }
            let mut bounds = [values[0][0], values[0][1], values[0][0], values[0][1]];
            for [x, y] in values.iter().copied().skip(1) {
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x);
                bounds[3] = bounds[3].max(y);
            }
            (
                bounds,
                Some(LocalShape::Polygon {
                    points: values,
                    closed,
                }),
                None,
            )
        }
        GeometryData::Path { path_ref } => {
            let path = chart
                .paths
                .get(*path_ref as usize)
                .ok_or("render.invalid-reference")?;
            let (subpaths, bounds) =
                evaluate_path(chart, path, chart_time, environment, world_matrix)?;
            (
                bounds,
                Some(LocalShape::Path {
                    subpaths,
                    fill_rule: path.fill_rule,
                }),
                None,
            )
        }
        GeometryData::Text { glyph_runs, origin } => {
            let (bounds, contours) =
                evaluate_text(chart, glyph_runs, *origin, chart_time, environment)?;
            (bounds, Some(LocalShape::Text { contours }), None)
        }
        GeometryData::Image {
            resource_id,
            destination,
            source,
            sampling,
        } => {
            let image = chart
                .decoded_images
                .get(resource_id)
                .ok_or("render.resource-not-found")?;
            let mut values = [0.0; 4];
            for (value, descriptor) in values.iter_mut().zip(destination) {
                *value = query_scalar_in(
                    chart,
                    *descriptor,
                    chart_time,
                    ValueType::Length,
                    environment,
                )?;
            }
            let [x, y, width, height] = values;
            let right = x + width;
            let top = y + height;
            if width < 0.0 || height < 0.0 || !right.is_finite() || !top.is_finite() {
                return Err("render.invalid-geometry");
            }
            let source = if let Some(descriptors) = source {
                let mut values = [0.0; 4];
                for (value, descriptor) in values.iter_mut().zip(descriptors) {
                    *value = query_scalar_in(
                        chart,
                        *descriptor,
                        chart_time,
                        ValueType::Float,
                        environment,
                    )?;
                }
                values
            } else {
                [0.0, 0.0, f64::from(image.width), f64::from(image.height)]
            };
            validate_source_rect(source, image.width, image.height)?;
            let bounds = [x, y, right, top];
            (
                bounds,
                Some(LocalShape::Image { bounds }),
                Some(ImageDrawOp {
                    resource_id: *resource_id,
                    destination: values,
                    source,
                    sampling: *sampling,
                }),
            )
        }
    };
    Ok(GeometryEvaluation {
        world_bounds: transformed_bounds(world_matrix, local_bounds)?,
        shape,
        image,
    })
}

fn validate_source_rect(
    source: [f64; 4],
    image_width: u32,
    image_height: u32,
) -> Result<(), &'static str> {
    let [x, y, width, height] = source;
    let right = x + width;
    let bottom = y + height;
    if x < 0.0
        || y < 0.0
        || width < 0.0
        || height < 0.0
        || !right.is_finite()
        || !bottom.is_finite()
        || right > f64::from(image_width)
        || bottom > f64::from(image_height)
    {
        return Err("render.invalid-geometry");
    }
    if width > 0.0
        && height > 0.0
        && (!source_axis_has_texel_center(x, width, image_width)
            || !source_axis_has_texel_center(y, height, image_height))
    {
        return Err("render.invalid-geometry");
    }
    Ok(())
}

fn source_axis_has_texel_center(origin: f64, size: f64, dimension: u32) -> bool {
    if dimension == 0 {
        return false;
    }
    let first = (origin - 0.5).ceil().max(0.0);
    let last = (origin + size - 0.5)
        .floor()
        .min(f64::from(dimension.saturating_sub(1)));
    first <= last
}

fn evaluate_text(
    chart: &DecodedRenderChart,
    glyph_run_refs: &[u32],
    origin_descriptor: u32,
    chart_time: f64,
    environment: EvaluationEnvironment,
) -> Result<([f64; 4], TextContours), &'static str> {
    let origin = query_vec2_in(
        chart,
        origin_descriptor,
        chart_time,
        ValueType::Vec2Length,
        environment,
    )?;
    let mut contours = Vec::new();
    for glyph_run_ref in glyph_run_refs {
        let run = chart
            .glyph_runs
            .get(*glyph_run_ref as usize)
            .ok_or("render.invalid-reference")?;
        let size = query_scalar_in(
            chart,
            run.size_descriptor,
            chart_time,
            ValueType::Length,
            environment,
        )?;
        if size <= 0.0 {
            return Err("render.invalid-geometry");
        }
        let font = chart
            .decoded_fonts
            .get(&run.font_resource_id)
            .ok_or("render.resource-not-found")?;
        if font.units_per_em == 0 || !run.run_offset.iter().all(|value| value.is_finite()) {
            return Err("render.invalid-geometry");
        }
        let scale = size / f64::from(font.units_per_em);
        let mut pen = [run.run_offset[0] * size, run.run_offset[1] * size];
        if !pen.iter().all(|value| value.is_finite()) {
            return Err("render.invalid-geometry");
        }
        for placement in &run.glyphs {
            if ![
                placement.x_advance,
                placement.y_advance,
                placement.x_offset,
                placement.y_offset,
            ]
            .iter()
            .all(|value| value.is_finite())
            {
                return Err("render.invalid-geometry");
            }
            let glyph = font
                .glyphs
                .get(placement.glyph_id as usize)
                .ok_or("render.invalid-geometry")?;
            let glyph_origin = [
                origin[0] + pen[0] + placement.x_offset * size,
                origin[1] + pen[1] + placement.y_offset * size,
            ];
            if !glyph_origin.iter().all(|value| value.is_finite()) {
                return Err("render.invalid-geometry");
            }
            for contour in &glyph.contours {
                let points = glyph_contour(contour, glyph_origin, scale)?;
                if points.len() >= 3 {
                    contours.push(points);
                }
            }
            pen[0] += placement.x_advance * size;
            pen[1] += placement.y_advance * size;
            if !pen.iter().all(|value| value.is_finite()) {
                return Err("render.invalid-geometry");
            }
        }
    }
    let mut bounds = [origin[0], origin[1], origin[0], origin[1]];
    for [x, y] in contours.iter().flat_map(|contour| contour.iter()) {
        bounds[0] = bounds[0].min(*x);
        bounds[1] = bounds[1].min(*y);
        bounds[2] = bounds[2].max(*x);
        bounds[3] = bounds[3].max(*y);
    }
    if !bounds.iter().all(|value| value.is_finite()) {
        return Err("render.invalid-geometry");
    }
    Ok((bounds, contours))
}

fn glyph_contour(
    contour: &[crate::assets::OutlinePoint],
    origin: [f64; 2],
    scale: f64,
) -> Result<Vec<[f64; 2]>, &'static str> {
    if contour.len() < 2 || !scale.is_finite() {
        return Err("render.invalid-geometry");
    }
    let points: Vec<_> = contour
        .iter()
        .map(|point| {
            (
                [
                    origin[0] + f64::from(point.x) * scale,
                    origin[1] + f64::from(point.y) * scale,
                ],
                point.on_curve,
            )
        })
        .collect();
    if points
        .iter()
        .any(|(point, _)| !point.iter().all(|value| value.is_finite()))
    {
        return Err("render.invalid-geometry");
    }
    let last = points.len() - 1;
    let (start_index, start) = if points[0].1 {
        (1, points[0].0)
    } else if points[last].1 {
        (0, points[last].0)
    } else {
        (0, midpoint(points[last].0, points[0].0)?)
    };
    let mut output = vec![start];
    let mut current = start;
    let mut pending_control = None;
    for offset in 0..points.len() {
        let (point, on_curve) = points[(start_index + offset) % points.len()];
        if on_curve {
            if let Some(control) = pending_control.take() {
                append_quadratic(&mut output, current, control, point, 0)?;
            } else if point != current {
                output.push(point);
            }
            current = point;
        } else if let Some(control) = pending_control.replace(point) {
            let implied = midpoint(control, point)?;
            append_quadratic(&mut output, current, control, implied, 0)?;
            current = implied;
        }
    }
    if let Some(control) = pending_control {
        append_quadratic(&mut output, current, control, start, 0)?;
    } else if output.last().copied() != Some(start) {
        output.push(start);
    }
    Ok(output)
}

fn append_quadratic(
    points: &mut Vec<[f64; 2]>,
    start: [f64; 2],
    control: [f64; 2],
    end: [f64; 2],
    depth: u8,
) -> Result<(), &'static str> {
    let flatness = distance_to_segment(control, start, end);
    if !flatness.is_finite() {
        return Err("render.invalid-geometry");
    }
    if flatness <= GLYPH_FLATTEN_TOLERANCE {
        points.push(end);
        return Ok(());
    }
    if depth >= GLYPH_MAX_FLATTEN_DEPTH {
        return Err("render.limit-exceeded");
    }
    let first = midpoint(start, control)?;
    let second = midpoint(control, end)?;
    let middle = midpoint(first, second)?;
    append_quadratic(points, start, first, middle, depth + 1)?;
    append_quadratic(points, middle, second, end, depth + 1)
}

fn distance_to_segment(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let length = dx.hypot(dy);
    if length == 0.0 {
        return (point[0] - start[0]).hypot(point[1] - start[1]);
    }
    if !length.is_finite() {
        return f64::INFINITY;
    }
    let projection = (((point[0] - start[0]) * dx) + ((point[1] - start[1]) * dy)) / length;
    if !projection.is_finite() {
        return f64::INFINITY;
    }
    let along = projection.clamp(0.0, length);
    let nearest = [
        start[0] + along * dx / length,
        start[1] + along * dy / length,
    ];
    (point[0] - nearest[0]).hypot(point[1] - nearest[1])
}

fn rounded_rect_scale(width: f64, height: f64, radii: [f64; 4]) -> f64 {
    let mut scale: f64 = 1.0;
    for (available, sum) in [
        (width, radii[0] + radii[1]),
        (width, radii[3] + radii[2]),
        (height, radii[0] + radii[3]),
        (height, radii[1] + radii[2]),
    ] {
        if sum > 0.0 {
            scale = scale.min(available / sum);
        }
    }
    scale.clamp(0.0, 1.0)
}

#[cfg(test)]
#[path = "semantic_tests.rs"]
mod semantic_tests;
