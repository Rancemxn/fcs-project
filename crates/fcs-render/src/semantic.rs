//! Product Render semantic evaluation and reference raster surfaces (I9).

use std::collections::BTreeMap;

use fcs_fcbc::{
    EvaluationEnvironment, RuntimeValue, ValueType, query_descriptor, query_distance,
    query_scroll_coordinate,
};

use crate::{
    RenderLimits,
    loader::{DecodedRenderChart, GeometryData, NodeKind, PaintData, PaintRecord},
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
    pub image: Option<ImageDrawOp>,
    pub opacity: f64,
    pub world_matrix: [f64; 9],
    pub composite: u16,
    pub clip_chain: Vec<u64>,
    pub bounds: [f64; 4],
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
    attachment: Option<AttachmentState>,
}

struct EvaluatedScene {
    ops: Vec<DrawOp>,
    shapes: BTreeMap<u64, EvaluatedShape>,
    clips: BTreeMap<u64, EvaluatedClip>,
}

#[derive(Clone)]
struct EvaluatedShape {
    shape: LocalShape,
    world_matrix: [f64; 9],
}

#[derive(Clone)]
enum LocalShape {
    Rect {
        bounds: [f64; 4],
    },
    RoundedRect {
        bounds: [f64; 4],
        radii: [f64; 4],
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
    },
    Polygon {
        points: Vec<[f64; 2]>,
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
    let (fill_rgba, linear_gradient, radial_gradient, image_pattern, bounds, image) =
        if node.kind.is_drawable() {
            let geometry = geometry_evaluation(
                chart,
                node.geometry_ref,
                chart_time,
                attachment.environment,
                world_matrix,
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
            (
                paint.0,
                paint.1,
                paint.2,
                paint.3,
                geometry.world_bounds,
                geometry.image,
            )
        } else {
            (None, None, None, None, [0.0; 4], None)
        };
    let opacity = query_opacity(chart, node, chart_time, attachment.environment)?;
    let effective_opacity = inherited_opacity * opacity;
    if !effective_opacity.is_finite() {
        return Err("render.invalid-composite");
    }
    if node.kind.is_drawable() {
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
            image,
            opacity: effective_opacity,
            world_matrix,
            composite: node.composite,
            clip_chain: clip_chain.clone(),
            bounds,
        });
    }
    for child_index in children.get(node_index).ok_or("render.invalid-graph")? {
        // ponytail: isolated group opacity is dropped from descendants because
        // this DrawOp protocol carries no group boundary; full isolated
        // compositing (offscreen render + atomic boundary composite) requires
        // either a Group marker on DrawOp or an offscreen buffer and is
        // explicitly out of scope for Issue #448 (fcs-render.md §3.4 / §5).
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
                attachment: Some(attachment),
            },
            scene,
        )?;
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
    source: RasterSource,
    opacity: f64,
    composite: u16,
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
        LocalShape::RoundedRect { bounds, radii } => rounded_rect_contains(*bounds, *radii, point),
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
        } => ellipse_contains(*center, *radius_x, *radius_y, *rotation, point),
        LocalShape::Polygon { points } => polygon_contains(points, point),
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

fn polygon_contains(points: &[[f64; 2]], point: [f64; 2]) -> bool {
    if points.len() < 3 {
        return false;
    }
    let mut winding = 0i32;
    for index in 0..points.len() {
        let [x0, y0] = points[index];
        let [x1, y1] = points[(index + 1) % points.len()];
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
    winding != 0
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

/// Rasterize supported fill geometry to tightly packed RGBA8 bytes.
///
/// The Render 1.0 reference sample grid is used for Rect, RoundedRect, Circle,
/// Ellipse, Polyline, and Polygon geometry; stroke/path/text coverage remains
/// outside this bounded fill path.
pub fn rasterize_solid_rgba8(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_at(chart, 0.0, width, height)
}

/// Rasterize the bounded fill surface at one chart-time query point.
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
    let mut raster_ops = Vec::new();
    for op in &scene.ops {
        if !matches!(
            op.kind,
            NodeKind::Rect
                | NodeKind::RoundedRect
                | NodeKind::Circle
                | NodeKind::Ellipse
                | NodeKind::Polyline
                | NodeKind::Polygon
                | NodeKind::Image
        ) {
            continue;
        }
        if !op.opacity.is_finite() {
            return Err("render.invalid-descriptor");
        }
        if !matches!(op.composite, 1..=5) {
            return Err("render.invalid-composite");
        }
        let Some(shape) = scene.shapes.get(&op.node_id) else {
            continue;
        };
        let source = if op.kind == NodeKind::Image {
            RasterSource::Image(op.image.ok_or("render.invalid-geometry")?)
        } else if let Some(gradient) = op.linear_gradient.clone() {
            RasterSource::LinearGradient(gradient)
        } else if let Some(gradient) = op.radial_gradient.clone() {
            RasterSource::RadialGradient(gradient)
        } else if let Some(pattern) = op.image_pattern {
            RasterSource::ImagePattern(pattern)
        } else {
            let Some(fill) = op.fill_rgba else {
                continue;
            };
            if fill.iter().any(|value| !value.is_finite()) {
                return Err("render.invalid-descriptor");
            }
            let alpha = fill[3].clamp(0.0, 1.0);
            RasterSource::Solid([
                fill[0].clamp(0.0, 1.0) * alpha,
                fill[1].clamp(0.0, 1.0) * alpha,
                fill[2].clamp(0.0, 1.0) * alpha,
                alpha,
            ])
        };
        let mut clips = Vec::new();
        for clip_id in &op.clip_chain {
            let clip = scene.clips.get(clip_id).ok_or("render.invalid-reference")?;
            if let Some(shape) = &clip.shape {
                clips.push(raster_shape(shape));
            }
        }
        raster_ops.push(RasterOp {
            shape: raster_shape(shape),
            clips,
            source,
            opacity: op.opacity,
            composite: op.composite,
        });
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
                    for op in &raster_ops {
                        let Some(local_point) = raster_shape_local_point(&op.shape, point) else {
                            continue;
                        };
                        if !local_shape_contains(&op.shape.shape, local_point)
                            || !op
                                .clips
                                .iter()
                                .all(|clip| raster_shape_contains(clip, point))
                        {
                            continue;
                        }
                        if let Some(source) =
                            raster_source_at(chart, &op.source, local_point, op.opacity)?
                        {
                            composite_premultiplied(&mut sample, source, op.composite)?;
                        }
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
                }),
                None,
            )
        }
        GeometryData::Polyline { points } | GeometryData::Polygon { points } => {
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
            (bounds, Some(LocalShape::Polygon { points: values }), None)
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
        _ => (
            [0.0, 0.0, chart.viewport_width, chart.viewport_height],
            None,
            None,
        ),
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
