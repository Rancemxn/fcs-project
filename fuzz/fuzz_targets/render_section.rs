#![no_main]

use libfuzzer_sys::fuzz_target;

// Render loading is layered on the FCBC Core load, so this target reaches the
// RenderSection/resource parsing that fcbc_container cannot: reference
// resolution, geometry/paint/stroke/clip record decoding, and the render
// graph checks. Rejections are expected constantly; panics are the finding.
fuzz_target!(|bytes: &[u8]| {
    let _ = fcs_render::load_render(bytes);
});
