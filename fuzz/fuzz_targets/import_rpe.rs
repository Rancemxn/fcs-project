#![no_main]

use fcs_conversion::{
    ArtifactRole, RpeLimits, SourceArtifact, SourceFormat, parse_json_document, parse_rpe_document,
};
use libfuzzer_sys::fuzz_target;

// RPE charts are untrusted community input; same layering as import_pgr, with
// the RPE eventLayers/meta interpretation behind the lossless-JSON walk.
fuzz_target!(|data: &[u8]| {
    let Ok(artifact) = SourceArtifact::new("chart.json", ArtifactRole::Chart, data) else {
        return;
    };
    let Ok(document) = parse_json_document(SourceFormat::Rpe, &artifact) else {
        return;
    };
    let _ = parse_rpe_document(&document, RpeLimits::default());
});
