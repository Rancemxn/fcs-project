//! Product Render semantic evaluation and reference raster surfaces (I9).

use fcs_fcbc::{EvaluationEnvironment, RuntimeValue, query_descriptor};

use crate::loader::{DecodedRenderChart, GeometryData, NodeKind, PaintData, PaintRecord};

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
/// Drawable nodes are sorted by (layer index, node z, node document order, node id).
/// Group/ClipGroup containers are omitted from the draw list.
pub fn evaluate_semantic_draw_list(
    chart: &DecodedRenderChart,
) -> Result<Vec<DrawOp>, &'static str> {
    let mut ops = Vec::new();
    for (layer_index, _layer) in chart.layers.iter().enumerate() {
        for node in &chart.nodes {
            if node.layer_index as usize != layer_index || !node.kind.is_drawable() {
                continue;
            }
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
            let bounds = geometry_bounds(chart, node.geometry_ref);
            ops.push(DrawOp {
                node_id: node.id,
                kind: node.kind,
                layer_index: layer_index as u32,
                z_order: node.z_order,
                document_order: node.document_order,
                fill_rgba,
                bounds,
            });
        }
    }
    ops.sort_by(|left, right| {
        (
            left.layer_index,
            left.z_order,
            left.document_order,
            left.node_id,
        )
            .cmp(&(
                right.layer_index,
                right.z_order,
                right.document_order,
                right.node_id,
            ))
    });
    Ok(ops)
}

/// Rasterize a solid-fill rectangle scene to tightly packed RGBA8 bytes.
///
/// Fills the full viewport with the first drawable Rect fill color. This is the
/// product reference solid raster entry used by CLI/RC solid-rect conformance.
pub fn rasterize_solid_rgba8(
    chart: &DecodedRenderChart,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, &'static str> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err("render.limit-exceeded");
    }
    let ops = evaluate_semantic_draw_list(chart)?;
    let fill = ops
        .iter()
        .find(|op| op.kind == NodeKind::Rect)
        .and_then(|op| op.fill_rgba)
        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
    let pixel = [
        (fill[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (fill[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (fill[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (fill[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ];
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for _ in 0..(width * height) {
        out.extend_from_slice(&pixel);
    }
    Ok(out)
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
                query_descriptor(&chart.core, color, 0.0, EvaluationEnvironment::at_time(0.0))?;
            match evaluation.value {
                RuntimeValue::Color(rgba) => Ok(Some(rgba)),
                _ => Err("render.invalid-descriptor"),
            }
        }
        _ => Ok(None),
    }
}

fn geometry_bounds(chart: &DecodedRenderChart, geometry_ref: Option<u32>) -> [f64; 4] {
    let Some(index) = geometry_ref else {
        return [0.0, 0.0, 0.0, 0.0];
    };
    let Some(geometry) = chart.geometries.get(index as usize) else {
        return [0.0, 0.0, 0.0, 0.0];
    };
    match geometry.data {
        GeometryData::Rect { .. } => {
            // Descriptor-backed origin/size are resolved as viewport-centered unit bounds
            // for product semantic summaries when only indices are available.
            let half_w = chart.viewport_width / 2.0;
            let half_h = chart.viewport_height / 2.0;
            [-half_w, -half_h, half_w, half_h]
        }
        _ => [0.0, 0.0, chart.viewport_width, chart.viewport_height],
    }
}
