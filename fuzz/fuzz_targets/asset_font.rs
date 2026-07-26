#![no_main]

use std::sync::OnceLock;

use fcs_render::TestFont;
use libfuzzer_sys::fuzz_target;

// decode_font gates on a whole-file checksum, so arbitrary bytes only reach
// the header path; the reachable hostile surface is arbitrary *text* shaped
// against a valid font, which is why the same input is also fed to the shaper.
fuzz_target!(|data: &[u8]| {
    let _ = fcs_render::decode_font(data);
    if let Ok(text) = std::str::from_utf8(data) {
        static FONT: OnceLock<TestFont> = OnceLock::new();
        let font = FONT.get_or_init(|| {
            fcs_render::decode_font(&fcs_render::build_test_font())
                .expect("the project-built test font must decode")
        });
        let _ = fcs_render::shape_simple_ltr(font, text);
    }
});
