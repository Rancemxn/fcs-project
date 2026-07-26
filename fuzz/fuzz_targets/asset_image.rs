#![no_main]

use libfuzzer_sys::fuzz_target;

// The first byte selects the declared decode parameters so every legal
// media-type/color-space/alpha combination stays reachable under mutation;
// the remainder is the image payload handed to the restricted decoder.
fuzz_target!(|data: &[u8]| {
    let Some((&selector, bytes)) = data.split_first() else {
        return;
    };
    let media_type = if selector & 1 == 0 {
        "image/png"
    } else {
        "image/webp"
    };
    let color_space = if selector & 2 == 0 {
        "srgb"
    } else {
        "linear-srgb"
    };
    let alpha = if selector & 4 == 0 {
        "straight"
    } else {
        "premultiplied"
    };
    let _ = fcs_render::decode_image(media_type, color_space, alpha, bytes);
});
