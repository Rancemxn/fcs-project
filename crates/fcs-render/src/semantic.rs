//! Product Render semantic evaluation and reference raster surfaces (I9).

use fcs_fcbc::{EvaluationEnvironment, RuntimeValue, ValueType, query_descriptor};

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
    pub bounds: [f64; 4],
}

/// Evaluate a deterministic draw-list for the loaded Render scene.
///
/// Drawable nodes follow the loader-validated layer and hierarchical storage order.
/// Group/ClipGroup containers are omitted from the draw list.
pub fn evaluate_semantic_draw_list(
    chart: &DecodedRenderChart,
) -> Result<Vec<DrawOp>, &'static str> {
    let mut ops = Vec::new();
    let mut children = vec![Vec::new(); chart.nodes.len()];
    for (index, node) in chart.nodes.iter().enumerate() {
        if let Some(parent) = node.parent {
            children
                .get_mut(parent as usize)
                .ok_or("render.invalid-graph")?
                .push(index);
        }
    }

    for layer in &chart.layers {
        let (first, roots) = if layer.root_count == 0 {
            (0, &chart.nodes[0..0])
        } else {
            let first = layer.first_root as usize;
            let end = first
                .checked_add(layer.root_count as usize)
                .ok_or("render.invalid-graph")?;
            (
                first,
                chart.nodes.get(first..end).ok_or("render.invalid-graph")?,
            )
        };
        for offset in 0..roots.len() {
            emit_draw_subtree(chart, &children, first + offset, &mut ops)?;
        }
    }
    Ok(ops)
}

fn emit_draw_subtree(
    chart: &DecodedRenderChart,
    children: &[Vec<usize>],
    node_index: usize,
    ops: &mut Vec<DrawOp>,
) -> Result<(), &'static str> {
    let node = chart.nodes.get(node_index).ok_or("render.invalid-graph")?;
    if node.kind.is_drawable() {
        let fill_rgba = match node.fill_paint {
            Some(index) => paint_rgba(
                chart,
                chart
                    .paints
                    .get(index as usize)
                    .ok_or("render.invalid-reference")?,
            )?,
            None => None,
        };
        let bounds = geometry_bounds(chart, node.geometry_ref)?;
        ops.push(DrawOp {
            node_id: node.id,
            kind: node.kind,
            layer_index: node.layer_index,
            z_order: node.z_order,
            document_order: node.document_order,
            fill_rgba,
            bounds,
        });
    }
    for child_index in children.get(node_index).ok_or("render.invalid-graph")? {
        emit_draw_subtree(chart, children, *child_index, ops)?;
    }
    Ok(())
}

/// Rasterize a solid-fill rectangle scene to tightly packed RGBA8 bytes.
///
/// Rasterize the first solid Rect with the Render 1.0 reference sample grid.
pub fn rasterize_solid_rgba8(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    rasterize_solid_rgba8_with_limits(chart, width, height, &RenderLimits::default())
}

pub fn rasterize_solid_rgba8_with_limits(
    chart: &DecodedRenderChart,
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
    let ops = evaluate_semantic_draw_list(chart)?;
    let (bounds, fill) = ops
        .iter()
        .find(|op| op.kind == NodeKind::Rect)
        .map(|op| (op.bounds, op.fill_rgba.unwrap_or([0.0; 4])))
        .unwrap_or(([0.0; 4], [0.0; 4]));
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for py in 0..height {
        for px in 0..width {
            let mut sum = [0.0; 4];
            for sy in 0..8 {
                for sx in 0..8 {
                    let device_x = f64::from(px) + (f64::from(sx) + 0.5) / 8.0;
                    let device_y = f64::from(py) + (f64::from(sy) + 0.5) / 8.0;
                    let logical_x = (device_x / f64::from(width) - 0.5) * chart.viewport_width;
                    let logical_y =
                        (0.5 - device_y / f64::from(height)) * chart.viewport_height;
                    if bounds[2] > bounds[0]
                        && bounds[3] > bounds[1]
                        && logical_x >= bounds[0]
                        && logical_x <= bounds[2]
                        && logical_y >= bounds[1]
                        && logical_y <= bounds[3]
                    {
                        let alpha = fill[3].clamp(0.0, 1.0);
                        sum[0] += fill[0].clamp(0.0, 1.0) * alpha;
                        sum[1] += fill[1].clamp(0.0, 1.0) * alpha;
                        sum[2] += fill[2].clamp(0.0, 1.0) * alpha;
                        sum[3] += alpha;
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
            let encode = |value| {
                if chart.viewport_color_space == 2 {
                    encode_srgb(value)
                } else {
                    value
                }
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
) -> Result<Option<[f64; 4]>, &'static str> {
    match paint.data {
        // `colorDescriptor` is an FCBC descriptor index, not a constant-pool slot
        // (fcs-render.md sections 14.5 and 15.3); the loader already validated it as a
        // Color descriptor, so an unresolvable or wrongly-typed result is an invariant
        // violation. This surface has no chartTime parameter yet (#295), so descriptors
        // are evaluated at time 0.0.
        PaintData::Solid { color } => {
            let evaluation =
                query_descriptor(&chart.core, color, 0.0, EvaluationEnvironment::at_time(0.0))
                    .map_err(|_| "render.invalid-descriptor")?;
            match evaluation.value {
                RuntimeValue::Color(rgba) => Ok(Some(rgba)),
                _ => Err("render.invalid-descriptor"),
            }
        }
        _ => Ok(None),
    }
}

fn geometry_bounds(
    chart: &DecodedRenderChart,
    geometry_ref: Option<u32>,
) -> Result<[f64; 4], &'static str> {
    let Some(index) = geometry_ref else {
        return Err("render.invalid-reference");
    };
    let Some(geometry) = chart.geometries.get(index as usize) else {
        return Err("render.invalid-reference");
    };
    match geometry.data {
        GeometryData::Rect { origin, size } => {
            let [x, y] = vec2_length(chart, origin)?;
            let [width, height] = vec2_length(chart, size)?;
            let right = x + width;
            let bottom = y + height;
            if width < 0.0 || height < 0.0 || !right.is_finite() || !bottom.is_finite() {
                return Err("render.invalid-geometry");
            }
            Ok([x, y, right, bottom])
        }
        _ => Ok([0.0, 0.0, chart.viewport_width, chart.viewport_height]),
    }
}

fn vec2_length(chart: &DecodedRenderChart, descriptor: u32) -> Result<[f64; 2], &'static str> {
    let value = query_descriptor(
        &chart.core,
        descriptor,
        0.0,
        EvaluationEnvironment::at_time(0.0),
    )
    .map_err(|_| "render.invalid-descriptor")?
    .value;
    match value {
        RuntimeValue::Vec2 {
            ty: ValueType::Vec2Length,
            value,
        } if value.iter().all(|component| component.is_finite()) => Ok(value),
        _ => Err("render.invalid-descriptor"),
    }
}
