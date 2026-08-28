//! Native-output FCBC mutation lane (Issue #314).
//!
//! The checked-in manifests under `docs/conformance/fcbc/` patch fixed byte
//! offsets of the checked-in goldens, so they cannot be replayed verbatim
//! against natively written bytes, whose section offsets differ. Each mutation
//! here is the structural equivalent of a checked-in pattern: the target field
//! is located through the validated section table of the pristine container,
//! corrupted in memory, and the product loader must reject with the same
//! stable `fcbc.*` category the manifests bind.

use std::fs;

use fcs_fcbc::{
    SECTION_ENTRY_SIZE, ValidatedContainer, load_chart, load_container, section_crc32_iso_hdlc,
    write_from_compilation,
};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use tempfile::tempdir;

const STRING_TABLE: u32 = 1;
const RESOURCES: u32 = 6;
const TEMPO: u32 = 8;
const NOTES: u32 = 10;
const TRACKS: u32 = 11;
const EXPRESSIONS: u32 = 12;
const DISTANCES: u32 = 13;
const RESOURCE_DATA: u32 = 20;

/// Sections 6/10/11/13 payloads open with a `count` u32 followed by an 8-byte
/// record header, so the first record's fields start at payload offset 12.
const FIRST_RECORD: usize = 12;

/// Compiles a nonempty native chart covering every section the mutations
/// target: a sub-beat tempo map, a Tap and a Hold, an expression descriptor,
/// a required extension, and one embedded audio resource.
fn native_bytes() -> Vec<u8> {
    let workspace = tempdir().unwrap();
    fs::write(workspace.path().join("hit.bin"), b"exact hit sound").unwrap();
    let source = r#"#fcs 5.0.0
format { profile: chart; }
resources {
    audio hit { source: "hit.bin"; mediaType: "audio/ogg"; }
}
tempoMap { 0beat -> 120bpm; 0.5beat -> 180bpm; 1.5beat -> 240bpm; }
lines { line main {} }
collections {
    notes {
        tap {
            id: "tap";
            line: @main;
            gameplay.time: 1beat;
            gameplay.soundPolicy: "resource";
            gameplay.soundResource: "hit";
            presentation.alpha: choose {
                when d < 100px => 1.0;
                else => 0.5;
            };
        };
        hold {
            id: "hold";
            line: @main;
            gameplay.time: 2beat;
            gameplay.endTime: 4beat;
            gameplay.scorePolicy: "custom";
            gameplay.scoreExtension: "score.ext";
        };
    }
}
extensions {
    extension("score.ext", 1.2.3) required { "mode": "test", }
}
"#;
    let document = parse_document(source).into_result().unwrap();
    let compilation = document
        .canonical_compilation(
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap();
    write_from_compilation(&compilation).unwrap()
}

/// Absolute offset of the section-table entry for `section_type`.
fn entry_offset(container: &ValidatedContainer, section_type: u32) -> usize {
    let index = container
        .sections
        .iter()
        .position(|section| section.section_type == section_type)
        .expect("section present");
    container.header.section_table_offset as usize + index * SECTION_ENTRY_SIZE
}

/// Absolute payload range of the section for `section_type`.
fn payload_range(container: &ValidatedContainer, section_type: u32) -> std::ops::Range<usize> {
    let section = container
        .sections
        .iter()
        .find(|section| section.section_type == section_type)
        .expect("section present");
    let start = section.offset as usize;
    start..start + section.length as usize
}

/// Overwrites payload bytes of one section and repairs its table checksum, so
/// the mutation reaches Core decode instead of `fcbc.section-checksum` — the
/// same shape as the second patch of every checked-in content mutation.
fn corrupt_payload(
    bytes: &mut [u8],
    container: &ValidatedContainer,
    section_type: u32,
    offset_in_payload: usize,
    replacement: &[u8],
) {
    let range = payload_range(container, section_type);
    let start = range.start + offset_in_payload;
    bytes[start..start + replacement.len()].copy_from_slice(replacement);
    let checksum = section_crc32_iso_hdlc(&bytes[range]);
    let checksum_at = entry_offset(container, section_type) + 32;
    bytes[checksum_at..checksum_at + 4].copy_from_slice(&checksum.to_le_bytes());
}

/// Header corruptions from `mutations.toml`: the header layout is fixed, so
/// these are the one family whose golden offsets replay verbatim on native
/// bytes. Both product surfaces must reject with the manifest's category.
#[test]
fn header_mutations_reject_on_both_product_surfaces() {
    let base = native_bytes();
    let bad_length = (base.len() as u64 + 1).to_le_bytes().to_vec();
    let cases: [(&str, usize, Vec<u8>, &str); 6] = [
        ("bad-magic", 0, vec![0x00], "fcbc.bad-magic"),
        (
            "unsupported-source-major",
            8,
            vec![0x06, 0x00],
            "fcbc.unsupported-source-version",
        ),
        (
            "unsupported-container-major",
            14,
            vec![0x03, 0x00],
            "fcbc.unsupported-container-version",
        ),
        (
            "unsupported-abi-major",
            20,
            vec![0x02, 0x00],
            "fcbc.unsupported-abi-version",
        ),
        (
            "reserved-container-profile",
            26,
            vec![0x02],
            "fcbc.unsupported-profile",
        ),
        (
            "file-length-mismatch",
            48,
            bad_length,
            "fcbc.file-length-mismatch",
        ),
    ];
    for (id, offset, replacement, category) in cases {
        let mut bytes = base.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let framing =
            load_container(&bytes).expect_err(&format!("native mutation {id} unexpectedly framed"));
        assert_eq!(framing.category(), category, "{id} via load_container");
        let core =
            load_chart(&bytes).expect_err(&format!("native mutation {id} unexpectedly loaded"));
        assert_eq!(core, category, "{id} via load_chart");
    }
}

/// Section-table corruptions mirroring the layout mutations of
/// `mutations.toml` (misaligned offset, corrupted checksum, overlap, unknown
/// or missing required section), located structurally instead of by golden
/// offset.
#[test]
fn section_table_mutations_reject_with_the_layout_categories() {
    let base = native_bytes();
    let container = load_container(&base).expect("pristine native bytes must frame");
    let first = entry_offset(&container, STRING_TABLE);
    let first_section = &container.sections[0];
    assert_eq!(first_section.section_type, STRING_TABLE);

    // first-section-misaligned: offset field at entry +16 loses 8-alignment.
    let mut misaligned = base.clone();
    let shifted = (first_section.offset + 1).to_le_bytes();
    misaligned[first + 16..first + 24].copy_from_slice(&shifted);
    assert_eq!(
        load_chart(&misaligned).unwrap_err(),
        "fcbc.section-alignment"
    );

    // first-section-checksum: checksum field at entry +32 no longer matches.
    let mut checksum = base.clone();
    for byte in &mut checksum[first + 32..first + 36] {
        *byte ^= 0xFF;
    }
    assert_eq!(load_chart(&checksum).unwrap_err(), "fcbc.section-checksum");

    // section-overlap via a corrupted length: extending the first section's
    // length (with its checksum repaired over the extended slice) makes the
    // second section start before the recomputed layout cursor.
    let mut overlap = base.clone();
    let extended = first_section.length + 8;
    overlap[first + 24..first + 32].copy_from_slice(&extended.to_le_bytes());
    let start = first_section.offset as usize;
    let crc = section_crc32_iso_hdlc(&overlap[start..start + extended as usize]);
    overlap[first + 32..first + 36].copy_from_slice(&crc.to_le_bytes());
    assert_eq!(load_chart(&overlap).unwrap_err(), "fcbc.section-overlap");

    // unknown-required-section: a required section whose version major is not
    // 1 is an unknown required section to the loader.
    let mut unknown = base.clone();
    unknown[first + 4..first + 6].copy_from_slice(&2u16.to_le_bytes());
    assert_eq!(
        load_chart(&unknown).unwrap_err(),
        "fcbc.unknown-required-section"
    );

    // missing-resource-data: retype the last entry (ResourceData, 20) to the
    // unassigned type 21 and clear its required flag, exactly like the golden
    // mutation's two patches; section 20 then vanishes from the required set.
    let mut missing = base.clone();
    let last = entry_offset(&container, RESOURCE_DATA);
    assert_eq!(
        container.sections.last().unwrap().section_type,
        RESOURCE_DATA
    );
    missing[last..last + 4].copy_from_slice(&21u32.to_le_bytes());
    missing[last + 10..last + 12].copy_from_slice(&0u16.to_le_bytes());
    assert_eq!(
        load_chart(&missing).unwrap_err(),
        "fcbc.missing-required-section"
    );
}

/// Core content corruptions mirroring `nonempty-execution-mutations.toml`
/// (forbidden descriptor kind, unknown expression opcode, forbidden distance
/// classification) and `embedded-resource-mutations.toml` (hash mismatch,
/// invalid resource data), plus a count, a kind byte, an id, and the
/// section 10 tempo consistency on natively written bytes.
#[test]
fn core_mutations_reject_with_the_content_categories() {
    let base = native_bytes();
    let container = load_container(&base).expect("pristine native bytes must frame");
    let flipped_resource_byte = [base[payload_range(&container, RESOURCE_DATA).start] ^ 0xFF];
    let cases: [(&str, u32, usize, Vec<u8>, &str); 9] = [
        // The tempo section is raw 40-byte points after the count.
        (
            "tempo-count-zero",
            TEMPO,
            0,
            0u32.to_le_bytes().to_vec(),
            "fcbc.invalid-tempo",
        ),
        // Floor the 1/2-beat point's denominator to 1: the stored chartTime
        // 0.25 no longer matches the Core mapping, so the loader's section 10
        // revalidation must reject the natively written bytes.
        (
            "tempo-floored-denominator",
            TEMPO,
            4 + 40 + 8,
            1i64.to_le_bytes().to_vec(),
            "fcbc.invalid-tempo",
        ),
        // NoteRecord: id u64 at +0, kind u8 at +20 of the record payload.
        (
            "note-id-zero",
            NOTES,
            FIRST_RECORD,
            0u64.to_le_bytes().to_vec(),
            "fcbc.duplicate-id",
        ),
        (
            "note-kind-out-of-range",
            NOTES,
            FIRST_RECORD + 20,
            vec![5],
            "fcbc.invalid-note",
        ),
        // PropertyDescriptor: kind u8 at +1; 5 is the forbidden Reference kind.
        (
            "forbidden-descriptor-kind",
            TRACKS,
            FIRST_RECORD + 1,
            vec![5],
            "fcbc.forbidden-descriptor",
        ),
        // Expression nodes are raw 20-byte records; opcode u16 first.
        (
            "unknown-expression-opcode",
            EXPRESSIONS,
            4,
            999u16.to_le_bytes().to_vec(),
            "fcbc.invalid-expression",
        ),
        // DistanceDescriptor: classification u8 at +68; 3 is forbidden.
        (
            "forbidden-distance-classification",
            DISTANCES,
            FIRST_RECORD + 68,
            vec![3],
            "fcbc.invalid-distance",
        ),
        // ResourceRecord: dataLength u64 at +28 balloons past section 20.
        (
            "resource-data-out-of-bounds",
            RESOURCES,
            FIRST_RECORD + 28,
            0x1_0000u64.to_le_bytes().to_vec(),
            "fcbc.invalid-resource-data",
        ),
        // One flipped payload byte breaks the recorded content SHA-256.
        (
            "resource-hash-mismatch",
            RESOURCE_DATA,
            0,
            flipped_resource_byte.to_vec(),
            "fcbc.resource-hash-mismatch",
        ),
    ];
    for (id, section_type, offset, replacement, category) in cases {
        let mut bytes = base.clone();
        corrupt_payload(&mut bytes, &container, section_type, offset, &replacement);
        let error =
            load_chart(&bytes).expect_err(&format!("native mutation {id} unexpectedly loaded"));
        assert_eq!(error, category, "{id} diagnostic mismatch");
    }
}

#[test]
fn unknown_optional_section_is_skipped_by_both_product_loaders() {
    const UNKNOWN_TYPE: u32 = 21;
    const UNKNOWN_PAYLOAD: [u8; 4] = [0xde, 0xad, 0xbe, 0xef];

    let base = native_bytes();
    let original = load_container(&base).expect("pristine native bytes must frame");
    let table_start = original.header.section_table_offset as usize;
    let table_end = table_start + original.sections.len() * SECTION_ENTRY_SIZE;

    // Insert one table entry before the body, then repair every shifted section offset.
    let mut bytes = Vec::with_capacity(base.len() + SECTION_ENTRY_SIZE + UNKNOWN_PAYLOAD.len() + 8);
    bytes.extend_from_slice(&base[..table_end]);
    bytes.resize(bytes.len() + SECTION_ENTRY_SIZE, 0);
    bytes.extend_from_slice(&base[table_end..]);
    for index in 0..original.sections.len() {
        let entry = table_start + index * SECTION_ENTRY_SIZE;
        let offset = u64::from_le_bytes(bytes[entry + 16..entry + 24].try_into().unwrap());
        bytes[entry + 16..entry + 24]
            .copy_from_slice(&(offset + SECTION_ENTRY_SIZE as u64).to_le_bytes());
    }

    let unknown_offset = bytes.len().checked_add(7).unwrap() & !7;
    bytes.resize(unknown_offset, 0);
    bytes.extend_from_slice(&UNKNOWN_PAYLOAD);
    let unknown_entry = table_end;
    bytes[unknown_entry..unknown_entry + 4].copy_from_slice(&UNKNOWN_TYPE.to_le_bytes());
    bytes[unknown_entry + 4..unknown_entry + 6].copy_from_slice(&1u16.to_le_bytes());
    bytes[unknown_entry + 12] = 3;
    bytes[unknown_entry + 16..unknown_entry + 24]
        .copy_from_slice(&(unknown_offset as u64).to_le_bytes());
    bytes[unknown_entry + 24..unknown_entry + 32]
        .copy_from_slice(&(UNKNOWN_PAYLOAD.len() as u64).to_le_bytes());
    bytes[unknown_entry + 32..unknown_entry + 36]
        .copy_from_slice(&section_crc32_iso_hdlc(&UNKNOWN_PAYLOAD).to_le_bytes());
    bytes[36..40].copy_from_slice(&((original.sections.len() + 1) as u32).to_le_bytes());
    let file_length = bytes.len() as u64;
    bytes[48..56].copy_from_slice(&file_length.to_le_bytes());

    let framed = load_container(&bytes).expect("unknown optional section must be skippable");
    assert_eq!(framed.sections.len(), original.sections.len() + 1);
    assert_eq!(framed.section_types().last(), Some(&UNKNOWN_TYPE));
    assert_eq!(
        framed.section_payload(&bytes, UNKNOWN_TYPE),
        Some(UNKNOWN_PAYLOAD.as_slice())
    );

    let original_chart = load_chart(&base).expect("pristine native bytes must load");
    let chart = load_chart(&bytes).expect("Core loader must skip unknown optional section");
    assert_eq!(chart.lines, original_chart.lines);
    assert_eq!(chart.notes, original_chart.notes);
    assert_eq!(chart.descriptors, original_chart.descriptors);
    assert_eq!(chart.expressions, original_chart.expressions);
    assert_eq!(chart.distances, original_chart.distances);
    assert_eq!(chart.sections.len(), original_chart.sections.len() + 1);
    assert_eq!(chart.sections.last().unwrap().section_type, UNKNOWN_TYPE);
}

#[test]
fn native_resource_data_trailing_byte_rejects() {
    let base = native_bytes();
    let container = load_container(&base).expect("pristine native bytes must frame");
    let resource = container
        .sections
        .iter()
        .find(|section| section.section_type == RESOURCE_DATA)
        .expect("ResourceData section");
    assert_eq!(
        container.sections.last().unwrap().section_type,
        RESOURCE_DATA
    );

    let mut trailing = base;
    trailing.push(0xA5);
    let resource_entry = entry_offset(&container, RESOURCE_DATA);
    let extended_length = resource.length + 1;
    trailing[resource_entry + 24..resource_entry + 32]
        .copy_from_slice(&extended_length.to_le_bytes());
    let payload_start = resource.offset as usize;
    let checksum =
        section_crc32_iso_hdlc(&trailing[payload_start..payload_start + extended_length as usize]);
    trailing[resource_entry + 32..resource_entry + 36].copy_from_slice(&checksum.to_le_bytes());
    let file_length = trailing.len() as u64;
    trailing[48..56].copy_from_slice(&file_length.to_le_bytes());

    assert_eq!(
        load_chart(&trailing).unwrap_err(),
        "fcbc.invalid-resource-data"
    );
}
