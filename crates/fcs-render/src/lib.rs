//! Product FCS Render Profile surface (I9).
//!
//! Owns RenderSection product load/write, semantic draw-list evaluation, solid
//! reference raster for core fill geometries, and restricted fixture asset codecs. Realtime GPU backends
//! remain post-RC.

mod assets;
mod loader;
mod semantic;
mod writer;

pub use assets::{
    AssetError, DecodedImage, ShapedGlyph, TestFont, build_test_font, decode_font,
    decode_font_with_limits, decode_image, decode_image_with_limits, encode_test_png,
    encode_test_webp, shape_simple_ltr, shape_simple_ltr_with_limits,
};
pub use loader::{
    DecodedRenderChart, GeometryData, NodeKind, load_render, load_render_with_limits,
};
pub use semantic::{
    DrawOp, evaluate_semantic_draw_list, evaluate_semantic_draw_list_at, rasterize_solid_rgba8,
    rasterize_solid_rgba8_at, rasterize_solid_rgba8_with_limits,
    rasterize_solid_rgba8_with_limits_at,
};
pub use writer::{
    ANALYTIC_NOTE_TEXT_ID, FONT_RESOURCE_TEXT_ID, MALFORMED_RESOURCE_TEXT_ID, PNG_RESOURCE_TEXT_ID,
    RenderAssets, TEXT_NOTE_TEXT_ID, UNSUPPORTED_RESOURCE_TEXT_ID, WEBP_RESOURCE_TEXT_ID, note_id,
    resource_id, stable_id, write_nonempty_render,
};

/// Public implementation limits used by the Render loader, asset decoders, and reference raster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderLimits {
    /// Maximum Resources records. Default: 4096.
    pub max_resources: usize,
    /// Maximum Layer records. Default: 4096.
    pub max_layers: usize,
    /// Maximum Node records. Default: 4096.
    pub max_nodes: usize,
    /// Maximum Geometry records. Default: 4096.
    pub max_geometries: usize,
    /// Maximum Path records. Default: 4096.
    pub max_paths: usize,
    /// Maximum Paint records. Default: 4096.
    pub max_paints: usize,
    /// Maximum Stroke records. Default: 4096.
    pub max_strokes: usize,
    /// Maximum Clip records. Default: 4096.
    pub max_clips: usize,
    /// Maximum GlyphRun records. Default: 4096.
    pub max_glyph_runs: usize,
    /// Maximum entries in one Render-owned Value array or object. Default: 4096.
    pub max_descriptor_values: usize,
    /// Maximum nesting depth of one Render-owned Value. Default: 64.
    pub max_descriptor_value_depth: usize,
    /// Maximum commands in one Path. Default: 4096.
    pub max_path_commands: usize,
    /// Maximum points in one Polyline or Polygon. Default: 4096.
    pub max_points: usize,
    /// Maximum stops in one gradient. Default: 4096.
    pub max_gradient_stops: usize,
    /// Maximum elements in one stroke dash array. Default: 4096.
    pub max_stroke_dashes: usize,
    /// Maximum placements in one GlyphRun. Default: 4096.
    pub max_glyphs_per_run: usize,
    /// Maximum bytes in one Render resource. Default: 64 MiB.
    pub max_single_resource_bytes: u64,
    /// Maximum bytes across all Render resources. Default: 256 MiB.
    pub max_total_resource_bytes: u64,
    /// Maximum decoded image width. Default: 8192.
    pub max_image_width: u32,
    /// Maximum decoded image height. Default: 8192.
    pub max_image_height: u32,
    /// Maximum image decoder allocation. Default: 64 MiB.
    pub max_image_decoded_bytes: u64,
    /// Maximum PNG or WebP chunks inspected for one image. Default: 4096.
    pub max_image_metadata_chunks: usize,
    /// Maximum tables inspected in one font. Default: 7.
    pub max_font_tables: usize,
    /// Maximum glyphs inspected in one font. Default: 2.
    pub max_font_glyphs: usize,
    /// Maximum contours in one simple TrueType glyph. Default: 4096.
    pub max_font_contours_per_glyph: usize,
    /// Maximum points in one simple TrueType glyph. Default: 65536.
    pub max_font_points_per_glyph: usize,
    /// Maximum points decoded across one font. Default: 1048576.
    pub max_font_glyph_work: usize,
    /// Maximum segments inspected in one TrueType cmap. Default: 4096.
    pub max_font_cmap_segments: usize,
    /// Maximum scalar-to-glyph mappings produced from one TrueType cmap. Default: 65536.
    pub max_font_cmap_mappings: usize,
    /// Maximum glyphs produced by one simple-ltr shaping call. Default: 4096.
    pub max_shaped_glyphs: usize,
    /// Maximum Group nodes in one ancestry chain. Default: 256.
    pub max_group_depth: usize,
    /// Maximum ClipGroup nodes in one ancestry chain. Default: 256.
    pub max_clip_depth: usize,
    /// Maximum width or height of the reference raster. Default: 4096.
    pub max_raster_dimension: u32,
}

impl Default for RenderLimits {
    fn default() -> Self {
        Self {
            max_resources: 4096,
            max_layers: 4096,
            max_nodes: 4096,
            max_geometries: 4096,
            max_paths: 4096,
            max_paints: 4096,
            max_strokes: 4096,
            max_clips: 4096,
            max_glyph_runs: 4096,
            max_descriptor_values: 4096,
            max_descriptor_value_depth: 64,
            max_path_commands: 4096,
            max_points: 4096,
            max_gradient_stops: 4096,
            max_stroke_dashes: 4096,
            max_glyphs_per_run: 4096,
            max_single_resource_bytes: 64 * 1024 * 1024,
            max_total_resource_bytes: 256 * 1024 * 1024,
            max_image_width: 8192,
            max_image_height: 8192,
            max_image_decoded_bytes: 64 * 1024 * 1024,
            max_image_metadata_chunks: 4096,
            max_font_tables: 7,
            max_font_glyphs: 2,
            max_font_contours_per_glyph: 4096,
            max_font_points_per_glyph: 65_536,
            max_font_glyph_work: 1_048_576,
            max_font_cmap_segments: 4096,
            max_font_cmap_mappings: 65_536,
            max_shaped_glyphs: 4096,
            max_group_depth: 256,
            max_clip_depth: 256,
            max_raster_dimension: 4096,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::assets::PNG_PIXELS;
    use crate::loader::{Attachment, GeometryData, PaintData, PaintRecord};
    use fcs_fcbc::{
        DescriptorKind, PropertyDescriptor, RuntimeValue, Segment, ValueType,
        write_nonempty_execution,
    };
    use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};

    fn render_fixture() -> Vec<u8> {
        let core = write_nonempty_execution();
        let png = encode_test_png();
        let webp = encode_test_webp();
        let font = build_test_font();
        write_nonempty_render(
            &core,
            RenderAssets {
                png: &png,
                webp: &webp,
                font: &font,
                malformed: b"not-an-image",
            },
        )
    }

    fn set_descriptor_constant(
        render: &mut DecodedRenderChart,
        descriptor: u32,
        value: RuntimeValue,
    ) {
        let constant = render.core.constants.len() as u32;
        render.core.constants.push(value);
        render.core.descriptors[descriptor as usize].kind = DescriptorKind::Constant(constant);
    }

    fn add_descriptor_constant(render: &mut DecodedRenderChart, value: RuntimeValue) -> u32 {
        let constant = render.core.constants.len() as u32;
        let descriptor = render.core.descriptors.len() as u32;
        let domain = render.core.descriptors[2].domain;
        render.core.constants.push(value.clone());
        render.core.descriptors.push(PropertyDescriptor {
            property_type: value.value_type(),
            domain,
            kind: DescriptorKind::Constant(constant),
        });
        descriptor
    }

    fn add_descriptor_segment_points(
        render: &mut DecodedRenderChart,
        start: RuntimeValue,
        end: RuntimeValue,
    ) -> u32 {
        let start_constant = render.core.constants.len() as u32;
        let property_type = start.value_type();
        render.core.constants.push(start);
        let end_constant = render.core.constants.len() as u32;
        render.core.constants.push(end);
        let descriptor = render.core.descriptors.len() as u32;
        render.core.descriptors.push(PropertyDescriptor {
            property_type,
            domain: render.core.descriptors[2].domain,
            kind: DescriptorKind::SegmentTrack(vec![
                Segment {
                    start: 0.0,
                    end: 0.0,
                    interpolation: 1,
                    easing: 0,
                    flags: 1,
                    start_constant,
                    end_constant: start_constant,
                    bezier: [0.0; 4],
                },
                Segment {
                    start: 1.0,
                    end: 1.0,
                    interpolation: 1,
                    easing: 0,
                    flags: 1,
                    start_constant: end_constant,
                    end_constant,
                    bezier: [0.0; 4],
                },
            ]),
        });
        descriptor
    }

    fn set_full_viewport_rect(render: &mut DecodedRenderChart) {
        set_descriptor_constant(
            render,
            2,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [-6.0, -6.0],
            },
        );
        let size = add_descriptor_constant(
            render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [12.0, 12.0],
            },
        );
        for geometry in &mut render.geometries {
            if let GeometryData::Rect {
                origin,
                size: rect_size,
            } = &mut geometry.data
                && *origin == 2
            {
                *rect_size = size;
            }
        }
    }

    fn make_world_attached(render: &mut DecodedRenderChart) {
        for node in &mut render.nodes {
            node.attachment = Attachment { kind: 1, id: 0 };
        }
    }

    fn isolate_solid_shape(render: &mut DecodedRenderChart, kind: NodeKind) {
        make_world_attached(render);
        render.viewport_width = 4.0;
        render.viewport_height = 4.0;
        render.viewport_color_space = 1;

        let hidden = add_descriptor_constant(render, RuntimeValue::Bool(false));
        let visible = add_descriptor_constant(render, RuntimeValue::Bool(true));
        let zero_position = add_descriptor_constant(
            render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [0.0, 0.0],
            },
        );
        let zero_angle = add_descriptor_constant(
            render,
            RuntimeValue::Scalar {
                ty: ValueType::Angle,
                value: 0.0,
            },
        );
        let unit_scale = add_descriptor_constant(
            render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Float,
                value: [1.0, 1.0],
            },
        );
        let opaque = add_descriptor_constant(
            render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 1.0,
            },
        );

        for node in &mut render.nodes {
            node.visibility_descriptor = hidden;
        }
        let target = render
            .nodes
            .iter()
            .position(|node| node.kind == kind)
            .expect("fixture shape node");
        let mut current = Some(target);
        while let Some(index) = current {
            let node = &mut render.nodes[index];
            node.visibility_descriptor = visible;
            node.position_descriptor = zero_position;
            node.origin_descriptor = zero_position;
            node.rotation_descriptor = zero_angle;
            node.scale_descriptor = unit_scale;
            node.opacity_descriptor = opaque;
            current = node.parent.map(|parent| parent as usize);
        }

        let geometry_index = render.nodes[target]
            .geometry_ref
            .expect("fixture shape geometry") as usize;
        let center = add_descriptor_constant(
            render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [0.0, 0.0],
            },
        );
        let radius = add_descriptor_constant(
            render,
            RuntimeValue::Scalar {
                ty: ValueType::Length,
                value: 2.0,
            },
        );
        let radius_y = add_descriptor_constant(
            render,
            RuntimeValue::Scalar {
                ty: ValueType::Length,
                value: 1.0,
            },
        );
        let angle = add_descriptor_constant(
            render,
            RuntimeValue::Scalar {
                ty: ValueType::Angle,
                value: 0.0,
            },
        );
        render.geometries[geometry_index].data = match kind {
            NodeKind::RoundedRect => {
                let origin = add_descriptor_constant(
                    render,
                    RuntimeValue::Vec2 {
                        ty: ValueType::Vec2Length,
                        value: [-2.0, -2.0],
                    },
                );
                let size = add_descriptor_constant(
                    render,
                    RuntimeValue::Vec2 {
                        ty: ValueType::Vec2Length,
                        value: [4.0, 4.0],
                    },
                );
                GeometryData::RoundedRect {
                    origin,
                    size,
                    radii: [radius; 4],
                }
            }
            NodeKind::Circle => GeometryData::Circle { center, radius },
            NodeKind::Ellipse => GeometryData::Ellipse {
                center,
                radius_x: radius,
                radius_y,
                rotation: angle,
            },
            _ => panic!("test helper only configures core fill shapes"),
        };
        let color = add_descriptor_constant(render, RuntimeValue::Color([1.0, 1.0, 1.0, 1.0]));
        let paint = render.paints.len() as u32;
        render.paints.push(PaintRecord {
            id: u64::MAX - u64::from(kind as u16),
            data: PaintData::Solid { color },
        });
        render.nodes[target].fill_paint = Some(paint);
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 bytes"))
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("u16 bytes"))
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 bytes"))
    }

    fn crc32_iso_hdlc(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    fn mutate_render_section(mut bytes: Vec<u8>, mutate: impl FnOnce(&mut [u8])) -> Vec<u8> {
        let section_count = u32_at(&bytes, 36) as usize;
        let table_offset = u64_at(&bytes, 40) as usize;
        let entry = (0..section_count)
            .map(|index| table_offset + index * 40)
            .find(|entry| u32_at(&bytes, *entry) == 14)
            .expect("Render section entry");
        let offset = u64_at(&bytes, entry + 16) as usize;
        let length = u64_at(&bytes, entry + 24) as usize;
        mutate(&mut bytes[offset..offset + length]);
        let checksum = crc32_iso_hdlc(&bytes[offset..offset + length]);
        bytes[entry + 32..entry + 36].copy_from_slice(&checksum.to_le_bytes());
        bytes
    }

    fn node_record_offsets(section: &[u8]) -> Vec<usize> {
        let mut offset = 68usize;
        for _ in 0..u32_at(section, 36) {
            offset += u32_at(section, offset) as usize;
        }
        let mut nodes = Vec::new();
        for _ in 0..u32_at(section, 40) {
            nodes.push(offset);
            offset += u32_at(section, offset) as usize;
        }
        nodes
    }

    #[test]
    fn product_render_write_load_eval_and_raster() {
        let core = write_nonempty_execution();
        let png = encode_test_png();
        let webp = encode_test_webp();
        let font = build_test_font();
        let malformed = b"not-an-image".as_slice();
        let assets = RenderAssets {
            png: &png,
            webp: &webp,
            font: &font,
            malformed,
        };
        let bytes = write_nonempty_render(&core, assets);
        let render = load_render(&bytes).expect("product render load");
        assert!(!render.layers.is_empty());
        assert!(!render.nodes.is_empty());
        assert_eq!(render.core.lines.len(), 2);
        let draw = evaluate_semantic_draw_list(&render).expect("semantic draw list");
        assert!(!draw.is_empty());
        let pixels = rasterize_solid_rgba8(&render, 4, 4).expect("solid raster");
        assert_eq!(pixels.len(), 4 * 4 * 4);
    }

    #[test]
    fn semantic_query_honors_active_half_open_interval() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        let root_index = render
            .nodes
            .iter_mut()
            .position(|node| node.parent.is_none() && node.kind == NodeKind::Group)
            .expect("fixture root");
        let root = &mut render.nodes[root_index];
        root.flags &= !0b11;
        root.active_start = 0.0;
        root.active_end = 1.0;
        let rect_id = render
            .nodes
            .iter()
            .find(|node| node.parent == Some(root_index as u32) && node.kind == NodeKind::Rect)
            .expect("fixture root descendant")
            .id;

        assert!(
            evaluate_semantic_draw_list_at(&render, 0.999)
                .expect("active query")
                .iter()
                .any(|op| op.node_id == rect_id)
        );
        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.id == rect_id)
            .expect("fixture Rect node");
        let paint = render.nodes[rect_index]
            .fill_paint
            .expect("fixture Rect paint");
        let color_descriptor = match render.paints[paint as usize].data {
            PaintData::Solid { color } => color,
            _ => panic!("fixture Rect uses a solid paint"),
        };
        render.core.descriptors[color_descriptor as usize].kind =
            DescriptorKind::Constant(u32::MAX);
        assert!(
            !evaluate_semantic_draw_list_at(&render, 1.0)
                .expect("half-open end query")
                .iter()
                .any(|op| op.node_id == rect_id)
        );
    }

    #[test]
    fn semantic_visibility_skips_subtree_before_later_descriptor_errors() {
        let mut render = load_render(&render_fixture()).expect("render load");
        let parent = render
            .nodes
            .iter()
            .position(|node| node.parent.is_none() && node.kind == NodeKind::Group)
            .expect("fixture parent");
        let visibility_descriptor = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Bool(false),
            RuntimeValue::Bool(true),
        );
        render.nodes[parent].visibility_descriptor = visibility_descriptor;
        let child = render
            .nodes
            .iter()
            .position(|node| node.parent == Some(parent as u32))
            .expect("fixture child");
        let child_id = render.nodes[child].id;
        let parent_id = render.nodes[parent].id;
        for node in &mut render.nodes {
            if node.parent.is_none() && node.id != parent_id {
                node.flags = 0;
                node.active_start = 0.0;
                node.active_end = 0.0;
            }
        }
        assert!(
            !evaluate_semantic_draw_list_at(&render, 0.0)
                .expect("hidden query")
                .iter()
                .any(|op| op.node_id == child_id)
        );
        assert!(
            evaluate_semantic_draw_list_at(&render, 1.0)
                .expect("visible query")
                .iter()
                .any(|op| op.node_id == child_id)
        );
        let paint = render.nodes[child].fill_paint.expect("fixture child paint");
        let color_descriptor = match render.paints[paint as usize].data {
            PaintData::Solid { color } => color,
            _ => panic!("fixture child uses a solid paint"),
        };
        render.core.descriptors[color_descriptor as usize].kind =
            DescriptorKind::Constant(u32::MAX);

        let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("invisible subtree query");
        assert!(!draw.is_empty(), "later root remains queryable");
        assert!(!draw.iter().any(|op| op.node_id == child_id));
    }

    #[test]
    fn semantic_note_attachment_gate_short_circuits_subtree() {
        let mut render = load_render(&render_fixture()).expect("render load");
        let parent = render
            .nodes
            .iter()
            .position(|node| {
                node.parent.is_none() && node.kind == NodeKind::Group && node.attachment.kind == 4
            })
            .expect("fixture Note-attached Group");
        let note_id = render.nodes[parent].attachment.id;
        let note_index = render
            .core
            .notes
            .iter()
            .position(|note| note.id == note_id)
            .expect("fixture attachment Note");
        let child = render
            .nodes
            .iter()
            .position(|node| node.parent == Some(parent as u32))
            .expect("fixture Note-attached child");
        let child_id = render.nodes[child].id;
        let parent_id = render.nodes[parent].id;
        for node in &mut render.nodes {
            if node.parent.is_none() && node.id != parent_id {
                node.flags = 0;
                node.active_start = 0.0;
                node.active_end = 0.0;
            }
        }

        let dynamic_visibility = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Bool(false),
            RuntimeValue::Bool(true),
        );
        render.core.notes[note_index].property_descriptors[9] = dynamic_visibility;
        assert!(
            !evaluate_semantic_draw_list_at(&render, 0.0)
                .expect("hidden attachment query")
                .iter()
                .any(|op| op.node_id == child_id)
        );
        assert!(
            evaluate_semantic_draw_list_at(&render, 1.0)
                .expect("visible attachment query")
                .iter()
                .any(|op| op.node_id == child_id)
        );

        let paint = render.nodes[child].fill_paint.expect("fixture child paint");
        let color_descriptor = match render.paints[paint as usize].data {
            PaintData::Solid { color } => color,
            _ => panic!("fixture child uses a solid paint"),
        };
        render.core.descriptors[color_descriptor as usize].kind =
            DescriptorKind::Constant(u32::MAX);
        let draw = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("hidden attachment skips child descriptor failure");
        assert!(!draw.iter().any(|op| op.node_id == child_id));

        render.core.notes[note_index].flags &= !(1 << 1);
        assert!(
            !evaluate_semantic_draw_list_at(&render, 1.0)
                .expect("static render gate query")
                .iter()
                .any(|op| op.node_id == child_id)
        );
    }

    #[test]
    fn semantic_query_evaluates_geometry_and_solid_paint_at_query_time() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect node");
        render.viewport_width = 1.0;
        render.viewport_height = 1.0;
        let mut active_nodes = vec![rect_index];
        let mut current = Some(rect_index);
        while let Some(index) = current {
            active_nodes.push(index);
            current = render.nodes[index].parent.map(|parent| parent as usize);
        }
        for (index, node) in render.nodes.iter_mut().enumerate() {
            if !active_nodes.contains(&index) {
                node.flags = 0;
                node.active_start = 0.0;
                node.active_end = 0.0;
            }
        }

        let dynamic_origin = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [-0.5, -0.5],
            },
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [1.0, 2.0],
            },
        );
        let dynamic_size = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [1.0, 1.0],
            },
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [3.0, 4.0],
            },
        );
        let geometry = render.nodes[rect_index]
            .geometry_ref
            .expect("fixture Rect geometry");
        match &mut render.geometries[geometry as usize].data {
            GeometryData::Rect { origin, size } => {
                *origin = dynamic_origin;
                *size = dynamic_size;
            }
            _ => panic!("fixture Rect geometry kind changed"),
        }

        let dynamic_color = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Color([1.0, 0.0, 0.0, 1.0]),
            RuntimeValue::Color([0.0, 1.0, 0.0, 0.5]),
        );
        let paint = render.nodes[rect_index]
            .fill_paint
            .expect("fixture Rect paint");
        match &mut render.paints[paint as usize].data {
            PaintData::Solid { color } => *color = dynamic_color,
            _ => panic!("fixture Rect uses a solid paint"),
        }

        let at_start = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("query at start")
            .into_iter()
            .find(|op| op.node_id == render.nodes[rect_index].id)
            .expect("Rect at start");
        assert_eq!(at_start.bounds, [-0.5, -0.5, 0.5, 0.5]);
        assert_eq!(at_start.fill_rgba, Some([1.0, 0.0, 0.0, 1.0]));

        let at_end = evaluate_semantic_draw_list_at(&render, 1.0)
            .expect("query at end")
            .into_iter()
            .find(|op| op.node_id == render.nodes[rect_index].id)
            .expect("Rect at end");
        assert_eq!(at_end.bounds, [1.0, 2.0, 4.0, 6.0]);
        assert_eq!(at_end.fill_rgba, Some([0.0, 1.0, 0.0, 0.5]));

        let start_pixels =
            rasterize_solid_rgba8_at(&render, 0.0, 1, 1).expect("raster query at start");
        let end_pixels = rasterize_solid_rgba8_at(&render, 1.0, 1, 1).expect("raster query at end");
        assert_eq!(start_pixels[3], 255);
        assert_eq!(end_pixels[3], 0);
    }

    #[test]
    fn semantic_query_propagates_node_transform_into_world_bounds() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect node");
        let rect_id = render.nodes[rect_index].id;
        let dynamic_position = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [0.0, 0.0],
            },
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [2.0, 3.0],
            },
        );
        render.nodes[rect_index].position_descriptor = dynamic_position;

        let at_start = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("query at start")
            .into_iter()
            .find(|op| op.node_id == rect_id)
            .expect("Rect at start");
        let at_end = evaluate_semantic_draw_list_at(&render, 1.0)
            .expect("query at end")
            .into_iter()
            .find(|op| op.node_id == rect_id)
            .expect("Rect at end");

        assert_eq!(
            at_start.world_matrix,
            [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            at_end.world_matrix,
            [1.0, 0.0, 2.0, 0.0, 1.0, 3.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            at_end.bounds,
            [
                at_start.bounds[0] + 2.0,
                at_start.bounds[1] + 3.0,
                at_start.bounds[2] + 2.0,
                at_start.bounds[3] + 3.0,
            ]
        );
    }

    #[test]
    fn semantic_draw_ops_preserve_composite_and_inherited_clip_chain() {
        let render = load_render(&render_fixture()).expect("render load");
        let clip_group = render
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::ClipGroup)
            .expect("fixture ClipGroup");
        let clip = clip_group.clip_ref.expect("fixture ClipGroup clip");
        let clip_id = render.clips[clip as usize].id;
        let image_node = render
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Image)
            .expect("fixture Image");
        let image_id = image_node.id;
        let image = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("semantic query")
            .into_iter()
            .find(|op| op.node_id == image_id)
            .expect("Image draw op");

        assert_eq!(image.composite, image_node.composite);
        assert_eq!(image.clip_chain, vec![clip_id]);
    }

    #[test]
    fn semantic_query_evaluates_core_line_and_note_attachment_matrix() {
        let mut render = load_render(&render_fixture()).expect("render load");
        let root = render
            .nodes
            .iter()
            .find(|node| node.parent.is_none() && node.attachment.kind == 4)
            .expect("Note-attached root");
        let note_index = render
            .core
            .notes
            .iter()
            .position(|note| note.id == root.attachment.id)
            .expect("attachment Note");
        let line_id = render.core.notes[note_index].line_id;
        let line_index = render
            .core
            .lines
            .iter()
            .position(|line| line.id == line_id)
            .expect("attachment Line");
        let position = add_descriptor_constant(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [2.0, 3.0],
            },
        );
        let scale = add_descriptor_constant(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Float,
                value: [2.0, 3.0],
            },
        );
        let speed = add_descriptor_constant(
            &mut render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 1.0,
            },
        );
        let tempo = add_descriptor_constant(
            &mut render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 60.0,
            },
        );
        let line = &mut render.core.lines[line_index];
        line.position_descriptor = position;
        line.scale_descriptor = scale;
        line.scroll_speed_descriptor = speed;
        line.scroll_tempo_descriptor = tempo;
        line.floor_scale = 1.0;
        line.integration_origin = 0.0;
        line.initial_floor_position = 0.0;
        let distance = line.distance_descriptor as usize;
        render.core.distances[distance].scroll_speed_descriptor = speed;
        render.core.distances[distance].integration_origin = 0.0;
        render.core.distances[distance].initial_floor_position = 0.0;
        render.core.distances[distance].classification =
            fcs_fcbc::DistanceClassification::PortableAnalytic;
        render.core.notes[note_index].time = 1.0;

        let rect_id = render
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect")
            .id;
        let rect = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("attachment query")
            .into_iter()
            .find(|op| op.node_id == rect_id)
            .expect("Rect draw op");
        assert_eq!(
            rect.world_matrix,
            [2.0, 0.0, 4.0, 0.0, 3.0, 6.0, 0.0, 0.0, 1.0]
        );

        for node in &mut render.nodes {
            node.attachment = Attachment {
                kind: 3,
                id: line_id,
            };
        }
        let line_rect = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("Line attachment query")
            .into_iter()
            .find(|op| op.node_id == rect_id)
            .expect("Line-attached Rect draw op");
        assert_eq!(
            line_rect.world_matrix,
            [2.0, 0.0, 2.0, 0.0, 3.0, 3.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn semantic_query_reports_opacity_bounds_and_exposes_effective_value() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("semantic query");
        let rect = draw
            .iter()
            .find(|op| op.kind == NodeKind::Rect)
            .expect("fixture Rect");
        assert_eq!(rect.opacity, 1.0);

        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect node");
        let opacity_descriptor = add_descriptor_constant(
            &mut render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.5,
            },
        );
        render.nodes[rect_index].opacity_descriptor = opacity_descriptor;
        let draw = evaluate_semantic_draw_list_at(&render, 0.0).expect("opacity query");
        assert_eq!(
            draw.iter()
                .find(|op| op.kind == NodeKind::Rect)
                .expect("opacity Rect")
                .opacity,
            0.5
        );

        set_descriptor_constant(
            &mut render,
            opacity_descriptor,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.0,
            },
        );
        assert_eq!(
            evaluate_semantic_draw_list_at(&render, 0.0)
                .expect("zero opacity query")
                .iter()
                .find(|op| op.kind == NodeKind::Rect)
                .expect("zero opacity Rect")
                .opacity,
            0.0
        );

        set_descriptor_constant(
            &mut render,
            opacity_descriptor,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 1.5,
            },
        );
        assert_eq!(
            evaluate_semantic_draw_list_at(&render, 0.0).expect_err("invalid opacity"),
            "render.invalid-composite"
        );

        let dynamic_opacity = add_descriptor_segment_points(
            &mut render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.25,
            },
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.75,
            },
        );
        render.nodes[rect_index].opacity_descriptor = dynamic_opacity;
        assert_eq!(
            evaluate_semantic_draw_list_at(&render, 0.0)
                .expect("dynamic opacity at start")
                .iter()
                .find(|op| op.kind == NodeKind::Rect)
                .expect("dynamic opacity Rect at start")
                .opacity,
            0.25
        );
        assert_eq!(
            evaluate_semantic_draw_list_at(&render, 1.0)
                .expect("dynamic opacity at later time")
                .iter()
                .find(|op| op.kind == NodeKind::Rect)
                .expect("dynamic opacity Rect at later time")
                .opacity,
            0.75
        );

        render.nodes[rect_index].opacity_descriptor = opacity_descriptor;
        render.core.descriptors[opacity_descriptor as usize].kind =
            DescriptorKind::Constant(u32::MAX);
        assert_eq!(
            evaluate_semantic_draw_list_at(&render, 0.0)
                .expect_err("opacity descriptor execution failure"),
            "render.invalid-descriptor"
        );
    }

    #[test]
    fn isolated_group_own_opacity_does_not_multiply_into_descendant_draw_ops() {
        let mut render = load_render(&render_fixture()).expect("render load");
        // The fixture's only isolated Group root wraps a Text child
        // (writer.rs: text-isolate).
        let isolated_index = render
            .nodes
            .iter()
            .position(|node| {
                node.parent.is_none() && node.kind == NodeKind::Group && node.isolated()
            })
            .expect("fixture isolated Group root");

        let baseline_text = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("baseline query")
            .into_iter()
            .find(|op| op.kind == NodeKind::Text)
            .expect("fixture Text under isolated Group")
            .opacity;

        // Replace the isolated Group's own opacity descriptor with 0.5 without
        // touching the descendant's descriptor (Issue #448 bounded scope:
        // DrawOp has no group boundary, so full isolated compositing is out
        // of scope for this unit).
        let isolated_opacity = add_descriptor_constant(
            &mut render,
            RuntimeValue::Scalar {
                ty: ValueType::Float,
                value: 0.5,
            },
        );
        render.nodes[isolated_index].opacity_descriptor = isolated_opacity;

        let bounded_text = evaluate_semantic_draw_list_at(&render, 0.0)
            .expect("isolated query")
            .into_iter()
            .find(|op| op.kind == NodeKind::Text)
            .expect("fixture Text under isolated Group")
            .opacity;

        // The isolated Group's own opacity must not multiply into descendants
        // in this bounded DrawOp. If the boundary had been applied correctly
        // via an offscreen pass the value would differ; we explicitly do not
        // claim that behavior here.
        assert_eq!(bounded_text, baseline_text);
    }

    #[test]
    fn solid_paint_color_comes_from_the_descriptor_table_not_the_constant_pool() {
        let core = write_nonempty_execution();
        let png = encode_test_png();
        let webp = encode_test_webp();
        let font = build_test_font();
        let assets = RenderAssets {
            png: &png,
            webp: &webp,
            font: &font,
            malformed: b"not-an-image",
        };
        let bytes = write_nonempty_render(&core, assets);
        let mut render = load_render(&bytes).expect("product render load");
        set_full_viewport_rect(&mut render);
        let color_descriptor = render
            .paints
            .iter()
            .find_map(|paint| match paint.data {
                PaintData::Solid { color } => Some(color),
                _ => None,
            })
            .expect("fixture has a Solid paint");
        // Guard: the constant-pool slot sharing the Solid index is not a Color, so a
        // constant-pool lookup provably renders the wrong color instead of coinciding
        // with the descriptor resolution (fcs-render.md sections 14.5 and 15.3).
        assert!(!matches!(
            render.core.constants.get(color_descriptor as usize),
            Some(RuntimeValue::Color(_))
        ));
        let draw = evaluate_semantic_draw_list(&render).expect("semantic draw list");
        let rect = draw
            .iter()
            .find(|op| op.kind == NodeKind::Rect)
            .expect("fixture has a Rect draw op");
        // Descriptor 9 is the fixture's white Color constant descriptor.
        assert_eq!(rect.fill_rgba, Some([1.0, 1.0, 1.0, 1.0]));
        let pixels = rasterize_solid_rgba8(&render, 2, 2).expect("solid raster");
        assert_eq!(pixels, vec![255u8; 16]);
    }

    #[test]
    fn solid_rect_uses_bounds_output_color_space_and_even_quantization() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        set_full_viewport_rect(&mut render);
        render.viewport_color_space = 1;
        let color_descriptor = render
            .paints
            .iter()
            .find_map(|paint| match paint.data {
                PaintData::Solid { color } => Some(color),
                _ => None,
            })
            .expect("fixture has a Solid paint");
        set_descriptor_constant(
            &mut render,
            color_descriptor,
            RuntimeValue::Color([0.5, 0.5, 0.5, 1.0]),
        );

        let rect = evaluate_semantic_draw_list(&render)
            .expect("semantic draw list")
            .into_iter()
            .find(|op| op.kind == NodeKind::Rect)
            .expect("fixture has a Rect draw op");
        assert_eq!(rect.bounds, [-6.0, -6.0, 6.0, 6.0]);
        assert_eq!(
            rasterize_solid_rgba8(&render, 1, 1).expect("linear raster"),
            vec![128, 128, 128, 255]
        );

        render.viewport_color_space = 2;
        assert_eq!(
            rasterize_solid_rgba8(&render, 1, 1).expect("sRGB raster"),
            vec![188, 188, 188, 255]
        );

        render.viewport_color_space = 1;
        set_descriptor_constant(
            &mut render,
            color_descriptor,
            RuntimeValue::Color([0.5 / 255.0, 0.5 / 255.0, 0.5 / 255.0, 1.0]),
        );
        assert_eq!(
            rasterize_solid_rgba8(&render, 1, 1).expect("tie raster"),
            vec![0, 0, 0, 255]
        );
    }

    #[test]
    fn solid_rect_raster_composites_multiple_rect_ops_in_draw_order() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        set_full_viewport_rect(&mut render);
        render.viewport_color_space = 1;

        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect");
        let second_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::RoundedRect)
            .expect("fixture RoundedRect");
        let rect_geometry = render.nodes[rect_index].geometry_ref;
        let solid_paint = render.nodes[rect_index]
            .fill_paint
            .expect("fixture solid paint");
        let color_descriptor = match render.paints[solid_paint as usize].data {
            PaintData::Solid { color } => color,
            _ => panic!("fixture Rect paint is not solid"),
        };
        set_descriptor_constant(
            &mut render,
            color_descriptor,
            RuntimeValue::Color([1.0, 0.0, 0.0, 1.0]),
        );
        let blue = add_descriptor_constant(&mut render, RuntimeValue::Color([0.0, 0.0, 1.0, 0.5]));
        let blue_paint = render.paints.len() as u32;
        render.paints.push(PaintRecord {
            id: u64::MAX,
            data: PaintData::Solid { color: blue },
        });
        render.nodes[second_index].kind = NodeKind::Rect;
        render.nodes[second_index].geometry_ref = rect_geometry;
        render.nodes[second_index].fill_paint = Some(blue_paint);
        render.nodes[second_index].composite = 1;

        assert_eq!(
            rasterize_solid_rgba8(&render, 1, 1).expect("composited raster"),
            vec![128, 0, 128, 255]
        );
    }

    #[test]
    fn solid_rect_raster_applies_rect_clip_coverage() {
        let mut render = load_render(&render_fixture()).expect("render load");
        make_world_attached(&mut render);
        set_full_viewport_rect(&mut render);
        render.viewport_color_space = 1;

        let rect_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Rect)
            .expect("fixture Rect");
        let hidden_visibility = add_descriptor_constant(&mut render, RuntimeValue::Bool(false));
        render.nodes[rect_index].visibility_descriptor = hidden_visibility;
        let rect_geometry = render.nodes[rect_index].geometry_ref;
        let rect_fill = render.nodes[rect_index].fill_paint;
        let image_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Image)
            .expect("fixture Image");
        render.nodes[image_index].kind = NodeKind::Rect;
        render.nodes[image_index].geometry_ref = rect_geometry;
        render.nodes[image_index].fill_paint = rect_fill;

        let clip_group_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::ClipGroup)
            .expect("fixture ClipGroup");
        let clip_index = render.nodes[clip_group_index]
            .clip_ref
            .expect("fixture ClipGroup clip") as usize;
        let clip_geometry_index = render.clips[clip_index].geometry_ref as usize;
        let origin = add_descriptor_constant(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [-6.0, -6.0],
            },
        );
        let size = add_descriptor_constant(
            &mut render,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Length,
                value: [6.0, 12.0],
            },
        );
        render.geometries[clip_geometry_index].kind = NodeKind::Rect;
        render.geometries[clip_geometry_index].data = GeometryData::Rect { origin, size };

        assert_eq!(
            rasterize_solid_rgba8(&render, 2, 1).expect("clipped raster"),
            vec![255, 255, 255, 255, 0, 0, 0, 0]
        );
    }

    #[test]
    fn solid_core_shapes_rasterize_with_boundary_coverage() {
        for kind in [NodeKind::RoundedRect, NodeKind::Circle, NodeKind::Ellipse] {
            let mut render = load_render(&render_fixture()).expect("render load");
            isolate_solid_shape(&mut render, kind);

            let draw = evaluate_semantic_draw_list(&render).expect("shape semantic draw list");
            assert_eq!(draw.len(), 1);
            assert_eq!(draw[0].kind, kind);

            let pixels = rasterize_solid_rgba8(&render, 4, 4).expect("shape raster");
            let alpha: Vec<_> = pixels.chunks_exact(4).map(|pixel| pixel[3]).collect();
            assert!(alpha.iter().any(|value| *value > 0));
            assert!(alpha.iter().any(|value| *value < 255));
        }
    }

    #[test]
    fn solid_descriptor_execution_failure_uses_render_category() {
        let mut render = load_render(&render_fixture()).expect("render load");
        let color_descriptor = render
            .paints
            .iter()
            .find_map(|paint| match paint.data {
                PaintData::Solid { color } => Some(color),
                _ => None,
            })
            .expect("fixture has a Solid paint");
        // The bytes have already passed loader validation. Replace only the payload
        // to force the evaluator error path at the Render ownership boundary.
        render.core.descriptors[color_descriptor as usize].kind =
            DescriptorKind::Constant(u32::MAX);

        assert_eq!(
            evaluate_semantic_draw_list(&render).expect_err("descriptor must fail"),
            "render.invalid-descriptor"
        );
    }

    #[test]
    fn group_children_stay_before_later_root_siblings() {
        let core = write_nonempty_execution();
        let png = encode_test_png();
        let webp = encode_test_webp();
        let font = build_test_font();
        let assets = RenderAssets {
            png: &png,
            webp: &webp,
            font: &font,
            malformed: b"not-an-image",
        };
        let mut render = load_render(&write_nonempty_render(&core, assets)).expect("render load");

        let path_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Path)
            .expect("fixture has a path child");
        let image_index = render
            .nodes
            .iter()
            .position(|node| node.kind == NodeKind::Image)
            .expect("fixture has an image sibling subtree");
        let root_index = render
            .nodes
            .iter()
            .position(|node| node.parent.is_none() && node.kind == NodeKind::ClipGroup)
            .expect("fixture has a later root sibling");
        // The bytes have already passed loader validation. Making this root
        // drawable isolates traversal order without constructing another full
        // RenderSection fixture.
        render.nodes[root_index].kind = NodeKind::Rect;
        render.nodes[root_index].clip_ref = None;
        render.nodes[root_index].geometry_ref = None;
        render.nodes[root_index].fill_paint = None;
        render.nodes[root_index].stroke_ref = None;
        render.nodes[path_index].z_order = 10;

        let draw = evaluate_semantic_draw_list(&render).expect("semantic draw list");
        let path_position = draw
            .iter()
            .position(|op| op.node_id == render.nodes[path_index].id)
            .expect("path draw op");
        let image_position = draw
            .iter()
            .position(|op| op.node_id == render.nodes[image_index].id)
            .expect("image draw op");
        let root_position = draw
            .iter()
            .position(|op| op.node_id == render.nodes[root_index].id)
            .expect("root draw op");
        assert!(path_position < image_position);
        assert!(path_position < root_position);
    }

    #[test]
    fn png_16_bit_channels_are_normalized_before_8_bit_compatibility_conversion() {
        let mut bytes = Vec::new();
        let pixels = [0x0100_u16, 0x8000, 0xffff, 0xffff, 0, 0, 0, 0x8000];
        let raw = pixels
            .into_iter()
            .flat_map(u16::to_ne_bytes)
            .collect::<Vec<_>>();
        PngEncoder::new(Cursor::new(&mut bytes))
            .write_image(&raw, 2, 1, ColorType::Rgba16.into())
            .expect("fixed 16-bit PNG encoding");

        let decoded = decode_image("image/png", "linear-srgb", "straight", &bytes)
            .expect("16-bit PNG decode");
        assert_eq!(decoded.rgba8, vec![1, 128, 255, 255, 0, 0, 0, 128]);
        assert_eq!(
            decoded.linear_premultiplied[0][0],
            f64::from(0x0100_u16) / 65_535.0
        );
        assert_ne!(
            decoded.linear_premultiplied[0][0],
            f64::from(decoded.rgba8[0]) / 255.0
        );
        assert_eq!(
            decoded.linear_premultiplied[1][3],
            f64::from(0x8000_u16) / 65_535.0
        );

        let eight_bit = decode_image("image/png", "linear-srgb", "straight", &encode_test_png())
            .expect("8-bit PNG decode");
        assert_eq!(eight_bit.rgba8, PNG_PIXELS);
        assert_eq!(eight_bit.linear_premultiplied[0], [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn every_public_render_limit_has_focused_boundary_evidence() {
        let bytes = render_fixture();
        load_render(&bytes).expect("default limits accept the fixed fixture");

        let tighteners: [fn(&mut RenderLimits); 25] = [
            |limits| limits.max_resources = 0,
            |limits| limits.max_layers = 0,
            |limits| limits.max_nodes = 0,
            |limits| limits.max_geometries = 0,
            |limits| limits.max_paths = 0,
            |limits| limits.max_paints = 0,
            |limits| limits.max_strokes = 0,
            |limits| limits.max_clips = 0,
            |limits| limits.max_glyph_runs = 0,
            |limits| limits.max_descriptor_values = 0,
            |limits| limits.max_descriptor_value_depth = 1,
            |limits| limits.max_path_commands = 0,
            |limits| limits.max_points = 1,
            |limits| limits.max_gradient_stops = 1,
            |limits| limits.max_stroke_dashes = 1,
            |limits| limits.max_glyphs_per_run = 0,
            |limits| limits.max_single_resource_bytes = 0,
            |limits| limits.max_total_resource_bytes = 0,
            |limits| limits.max_image_width = 1,
            |limits| limits.max_image_height = 1,
            |limits| limits.max_image_decoded_bytes = 1,
            |limits| limits.max_image_metadata_chunks = 0,
            |limits| limits.max_font_tables = 6,
            |limits| limits.max_font_glyphs = 1,
            |limits| limits.max_font_contours_per_glyph = 0,
        ];
        for tighten in tighteners {
            let mut limits = RenderLimits::default();
            tighten(&mut limits);
            assert_eq!(
                load_render_with_limits(&bytes, &limits),
                Err("render.limit-exceeded")
            );
        }

        for tighten in [
            |limits: &mut RenderLimits| limits.max_font_points_per_glyph = 3,
            |limits: &mut RenderLimits| limits.max_font_glyph_work = 3,
            |limits: &mut RenderLimits| limits.max_font_cmap_segments = 1,
            |limits: &mut RenderLimits| limits.max_font_cmap_mappings = 1,
        ] as [fn(&mut RenderLimits); 4]
        {
            let mut limits = RenderLimits::default();
            tighten(&mut limits);
            assert_eq!(
                load_render_with_limits(&bytes, &limits),
                Err("render.limit-exceeded")
            );
        }

        let font = decode_font(&build_test_font()).expect("fixed font");
        let limits = RenderLimits {
            max_shaped_glyphs: 0,
            ..RenderLimits::default()
        };
        assert_eq!(
            shape_simple_ltr_with_limits(&font, "A", &limits),
            Err(AssetError::LimitExceeded)
        );

        let chart = load_render(&bytes).expect("fixed Render fixture");
        let limits = RenderLimits {
            max_raster_dimension: 1,
            ..RenderLimits::default()
        };
        assert_eq!(
            rasterize_solid_rgba8_with_limits(&chart, 2, 1, &limits),
            Err("render.limit-exceeded")
        );
    }

    #[test]
    fn group_and_clip_depth_overflow_stays_distinct_from_graph_cycles() {
        let bytes = render_fixture();
        for (kind, tighten) in [
            (1, |limits: &mut RenderLimits| limits.max_group_depth = 1),
            (2, |limits: &mut RenderLimits| limits.max_clip_depth = 1),
        ] as [(u16, fn(&mut RenderLimits)); 2]
        {
            let deep = mutate_render_section(bytes.clone(), |section| {
                let nodes = node_record_offsets(section);
                let root_count = u32_at(section, 100) as usize;
                let root = nodes[..root_count]
                    .iter()
                    .position(|offset| u16_at(section, *offset + 16) == kind)
                    .expect("container root kind");
                let child = *nodes[root_count..]
                    .iter()
                    .find(|offset| u32_at(section, **offset + 20) as usize == root)
                    .expect("container child");
                section[child + 16..child + 18].copy_from_slice(&kind.to_le_bytes());
            });
            let mut limits = RenderLimits::default();
            tighten(&mut limits);
            assert_eq!(
                load_render_with_limits(&deep, &limits),
                Err("render.limit-exceeded")
            );
        }

        let cycle = mutate_render_section(bytes, |section| {
            let nodes = node_record_offsets(section);
            let root_count = u32_at(section, 100) as usize;
            let child = nodes[root_count];
            section[child + 20..child + 24].copy_from_slice(&(root_count as u32).to_le_bytes());
        });
        assert_eq!(load_render(&cycle), Err("render.invalid-graph"));
    }
}
