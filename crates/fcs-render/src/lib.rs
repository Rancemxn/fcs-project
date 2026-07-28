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
    AssetError, DecodedImage, ShapedGlyph, TestFont, build_test_font, decode_font, decode_image,
    encode_test_png, encode_test_webp, shape_simple_ltr,
};
pub use loader::{DecodedRenderChart, NodeKind, load_render};
pub use semantic::{DrawOp, evaluate_semantic_draw_list, rasterize_solid_rgba8};
pub use writer::{
    ANALYTIC_NOTE_TEXT_ID, FONT_RESOURCE_TEXT_ID, MALFORMED_RESOURCE_TEXT_ID, PNG_RESOURCE_TEXT_ID,
    RenderAssets, TEXT_NOTE_TEXT_ID, UNSUPPORTED_RESOURCE_TEXT_ID, WEBP_RESOURCE_TEXT_ID, note_id,
    resource_id, stable_id, write_nonempty_render,
};

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use crate::assets::PNG_PIXELS;
    use crate::loader::PaintData;
    use fcs_fcbc::{RuntimeValue, write_nonempty_execution};
    use image::{ColorType, ImageEncoder, codecs::png::PngEncoder};

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
        let render = load_render(&bytes).expect("product render load");
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
        let pixels = [0x0101_u16, 0x8000, 0xffff, 0xffff, 0, 0, 0, 0x8000];
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
            f64::from(0x0101_u16) / 65_535.0
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
}
