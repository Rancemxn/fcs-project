#![no_main]

use fcs_conversion::{ArtifactRole, PecLimits, SourceArtifact, parse_pec_document};
use libfuzzer_sys::fuzz_target;

// PEC is a line-oriented text format parsed directly from the artifact, so
// this target reaches the command lexing/interpretation with no JSON layer
// in front of it.
fuzz_target!(|data: &[u8]| {
    let Ok(artifact) = SourceArtifact::new("chart.pec", ArtifactRole::Chart, data) else {
        return;
    };
    let _ = parse_pec_document(&artifact, PecLimits::default());
});
