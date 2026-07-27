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
    use super::*;
    use crate::loader::PaintData;
    use fcs_fcbc::{RuntimeValue, write_nonempty_execution};

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
}
