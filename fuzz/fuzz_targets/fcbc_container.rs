#![no_main]

use libfuzzer_sys::fuzz_target;

// The FCBC loader's whole threat model is hostile container bytes: issues
// #313 and #329 were input-controlled stack exhaustion in exactly this
// surface. Framing validation and the full Core load are separate public
// seams, so both are exercised: a framing-valid prefix must reach the deep
// section loaders without panicking, aborting, or hanging.
fuzz_target!(|bytes: &[u8]| {
    let _ = fcs_fcbc::load_container(bytes);
    let _ = fcs_fcbc::load_chart(bytes);
});
