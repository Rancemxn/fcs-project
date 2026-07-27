#![no_main]

use fcs_conversion::{
    ArtifactRole, PgrLimits, SourceArtifact, SourceFormat, parse_json_document, parse_pgr_document,
};
use libfuzzer_sys::fuzz_target;

// PGR charts are untrusted community input. The artifact/JSON layers reject
// most mutations; the interesting depth is the lossless-JSON walk and the
// PGR field interpretation behind them, under the default limits the product
// importer uses.
fuzz_target!(|data: &[u8]| {
    let Ok(artifact) = SourceArtifact::new("chart.json", ArtifactRole::Chart, data) else {
        return;
    };
    let Ok(document) = parse_json_document(SourceFormat::Pgr, &artifact) else {
        return;
    };
    let _ = parse_pgr_document(&document, PgrLimits::default());
});
