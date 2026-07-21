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
    use fcs_fcbc::write_nonempty_execution;

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
        let draw = evaluate_semantic_draw_list(&render);
        assert!(!draw.is_empty());
        let pixels = rasterize_solid_rgba8(&render, 4, 4).expect("solid raster");
        assert_eq!(pixels.len(), 4 * 4 * 4);
    }
}
