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
    pub opacity: f64,
    pub world_matrix: [f64; 9],
    pub composite: u16,
    pub clip_chain: Vec<u64>,
    pub bounds: [f64; 4],
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

#[derive(Clone, Copy)]
struct EvaluatedShape {
    shape: LocalShape,
    world_matrix: [f64; 9],
}

#[derive(Clone, Copy)]
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
}

#[derive(Clone, Copy)]
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
    let (fill_rgba, bounds) = if node.kind.is_drawable() {
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
        let fill_rgba = match node.fill_paint {
            Some(index) => paint_rgba(
                chart,
                chart
                    .paints
                    .get(index as usize)
                    .ok_or("render.invalid-reference")?,
                chart_time,
                attachment.environment,
            )?,
            None => None,
        };
        (fill_rgba, geometry.world_bounds)
    } else {
        (None, [0.0; 4])
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
            environment: attachment_environment(chart_time, 0.0, 0.0),
        }),
        3 => {
            let (matrix, q) = line_world_matrix(chart, node.attachment.id, chart_time)?;
            Ok(AttachmentState {
                matrix,
                environment: attachment_environment(chart_time, q, 0.0),
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
            let base_environment = attachment_environment(chart_time, q, 0.0);
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
            let environment = attachment_environment(chart_time, q, distance);
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

fn attachment_environment(chart_time: f64, q: f64, d: f64) -> EvaluationEnvironment {
    EvaluationEnvironment {
        s: chart_time,
        b: chart_time,
        q,
        d,
        p: 0.0,
    }
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
        let environment = attachment_environment(chart_time, q, 0.0);
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
        EvaluationEnvironment::at_time(chart_time),
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

#[derive(Clone, Copy)]
struct RasterShape {
    shape: LocalShape,
    inverse_world: Option<[f64; 9]>,
}

struct RasterOp {
    shape: RasterShape,
    clips: Vec<RasterShape>,
    source: [f64; 4],
    composite: u16,
}

fn raster_shape(shape: EvaluatedShape) -> RasterShape {
    RasterShape {
        shape: shape.shape,
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

fn raster_shape_contains(shape: RasterShape, point: [f64; 2]) -> bool {
    let Some(inverse_world) = shape.inverse_world else {
        return false;
    };
    let Ok([x, y]) = transform_point(inverse_world, point) else {
        return false;
    };
    local_shape_contains(shape.shape, [x, y])
}

fn local_shape_contains(shape: LocalShape, point: [f64; 2]) -> bool {
    match shape {
        LocalShape::Rect { bounds } => {
            bounds[2] > bounds[0]
                && bounds[3] > bounds[1]
                && point[0] >= bounds[0]
                && point[0] <= bounds[2]
                && point[1] >= bounds[1]
                && point[1] <= bounds[3]
        }
        LocalShape::RoundedRect { bounds, radii } => rounded_rect_contains(bounds, radii, point),
        LocalShape::Circle { center, radius } => {
            radius > 0.0
                && (point[0] - center[0]).mul_add(
                    point[0] - center[0],
                    (point[1] - center[1]) * (point[1] - center[1]),
                ) <= radius * radius
        }
        LocalShape::Ellipse {
            center,
            radius_x,
            radius_y,
            rotation,
        } => ellipse_contains(center, radius_x, radius_y, rotation, point),
    }
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

/// Rasterize supported solid-fill geometry to tightly packed RGBA8 bytes.
///
/// The Render 1.0 reference sample grid is used for Rect, RoundedRect, Circle,
/// and Ellipse; other geometry kinds remain semantic-only until their raster
/// coverage is implemented.
pub fn rasterize_solid_rgba8(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_at(chart, 0.0, width, height)
}

/// Rasterize the bounded solid-fill surface at one chart-time query point.
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
            NodeKind::Rect | NodeKind::RoundedRect | NodeKind::Circle | NodeKind::Ellipse
        ) {
            continue;
        }
        let Some(fill) = op.fill_rgba else {
            continue;
        };
        if fill.iter().any(|value| !value.is_finite()) || !op.opacity.is_finite() {
            return Err("render.invalid-descriptor");
        }
        if !matches!(op.composite, 1..=5) {
            return Err("render.invalid-composite");
        }
        let Some(shape) = scene.shapes.get(&op.node_id).copied() else {
            continue;
        };
        let mut clips = Vec::new();
        for clip_id in &op.clip_chain {
            let clip = scene.clips.get(clip_id).ok_or("render.invalid-reference")?;
            if let Some(shape) = clip.shape {
                clips.push(raster_shape(shape));
            }
        }
        let alpha = (fill[3].clamp(0.0, 1.0) * op.opacity).clamp(0.0, 1.0);
        raster_ops.push(RasterOp {
            shape: raster_shape(shape),
            clips,
            source: [
                fill[0].clamp(0.0, 1.0) * alpha,
                fill[1].clamp(0.0, 1.0) * alpha,
                fill[2].clamp(0.0, 1.0) * alpha,
                alpha,
            ],
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
                        if raster_shape_contains(op.shape, point)
                            && op
                                .clips
                                .iter()
                                .all(|clip| raster_shape_contains(*clip, point))
                        {
                            composite_premultiplied(&mut sample, op.source, op.composite)?;
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
) -> Result<Option<[f64; 4]>, &'static str> {
    match paint.data {
        // `colorDescriptor` is an FCBC descriptor index, not a constant-pool slot
        // (fcs-render.md sections 14.5 and 15.3); the loader already validated it as a
        // Color descriptor, so an unresolvable or wrongly-typed result is an invariant
        // violation. Transform, attachment, clip, and non-solid paint evaluation remain
        // in the broader #295 Render product path.
        PaintData::Solid { color } => {
            let evaluation = query_descriptor(&chart.core, color, chart_time, environment)
                .map_err(|_| "render.invalid-descriptor")?;
            match evaluation.value {
                RuntimeValue::Color(rgba) => Ok(Some(rgba)),
                _ => Err("render.invalid-descriptor"),
            }
        }
        _ => Ok(None),
    }
}

struct GeometryEvaluation {
    world_bounds: [f64; 4],
    shape: Option<LocalShape>,
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
        });
    };
    let Some(geometry) = chart.geometries.get(index as usize) else {
        return Err("render.invalid-reference");
    };
    let (local_bounds, shape) = match &geometry.data {
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
            (bounds, Some(LocalShape::Circle { center, radius }))
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
            )
        }
        _ => (
            [0.0, 0.0, chart.viewport_width, chart.viewport_height],
            None,
        ),
    };
    Ok(GeometryEvaluation {
        world_bounds: transformed_bounds(world_matrix, local_bounds)?,
        shape,
    })
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
