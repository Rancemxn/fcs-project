//! Product FCS Render Profile surface (I9).
//!
//! Owns RenderSection product load/write, semantic draw-list evaluation, solid
//! reference raster, and restricted fixture asset codecs. Realtime GPU backends
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
pub use loader::{DecodedRenderChart, NodeKind, load_render, load_render_with_limits};
pub use semantic::{
    DrawOp, evaluate_semantic_draw_list, rasterize_solid_rgba8, rasterize_solid_rgba8_with_limits,
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
    use crate::loader::{GeometryData, PaintData};
    use fcs_fcbc::{
        DescriptorKind, PropertyDescriptor, RuntimeValue, ValueType, write_nonempty_execution,
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
            if let GeometryData::Rect { origin, size: rect_size } = &mut geometry.data {
                if *origin == 2 {
                    *rect_size = size;
                }
            }
        }
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
        set_full_viewport_rect(&mut render);
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
