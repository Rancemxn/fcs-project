use fcs_model::{CanonicalValue, CanonicalValueType};
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::parse_document;

fn canonical(source: &str) -> fcs_model::CanonicalMetadata {
    parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_metadata()
        .unwrap_or_else(|diagnostics| panic!("canonical metadata failed: {diagnostics:?}"))
}

#[test]
fn lowers_the_metadata_fixture_without_retaining_workspace_source_paths() {
    let source = include_str!(
        "../../../docs/conformance/fcs5/source/valid/metadata-credits-resources-sync.fcs"
    );
    let metadata = canonical(source);

    let meta = metadata.meta().expect("meta block");
    assert_eq!(
        meta.get("title"),
        Some(&CanonicalValue::String("Conformance".into()))
    );
    assert_eq!(meta.get("revision"), Some(&CanonicalValue::Int(1)));
    assert!(matches!(
        meta.get("custom"),
        Some(CanonicalValue::Object(_))
    ));
    assert_eq!(metadata.contributors().len(), 1);
    assert_eq!(metadata.credits()[0].role(), "charter");
    assert_eq!(metadata.credits()[0].contributors(), &["alice"]);

    let resource = metadata.resources().get("empty").expect("resource");
    assert_eq!(resource.kind(), fcs_model::CanonicalResourceKind::Binary);
    assert_eq!(resource.media_type(), "application/octet-stream");
    assert!(resource.metadata().get("source").is_none());
    assert_eq!(metadata.sync().unwrap().audio_offset().seconds(), 0.1);
}

#[test]
fn typed_custom_data_preserves_object_and_array_order() {
    let metadata = canonical(
        r#"#fcs 5.0.0
format { profile: fragment; }
meta {
    custom: { "first": [1, 2], "second": { "nested": true } };
}
"#,
    );
    let Some(CanonicalValue::Object(custom)) = metadata.meta().unwrap().get("custom") else {
        panic!("expected custom object");
    };
    assert_eq!(custom.entries()[0].key(), "first");
    assert_eq!(custom.entries()[1].key(), "second");
    let CanonicalValue::Array {
        element_type,
        values,
    } = custom.entries()[0].value()
    else {
        panic!("expected typed array");
    };
    assert_eq!(element_type, &CanonicalValueType::Int);
    assert_eq!(values, &[CanonicalValue::Int(1), CanonicalValue::Int(2)]);
}

#[test]
fn rejects_duplicate_custom_keys_at_the_canonical_boundary() {
    let document = parse_document(
        r#"#fcs 5.0.0
format { profile: fragment; }
meta { custom: { "same": 1, "same": 2 }; }
"#,
    )
    .into_result()
    .unwrap();
    let diagnostics = document.canonical_metadata().unwrap_err();
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::SCHEMA_DUPLICATE_FIELD
    );
}

#[test]
fn validates_resource_paths_without_reading_or_hashing_the_filesystem() {
    let source =
        include_str!("../../../docs/conformance/fcs5/source/invalid/resource-path-escape.fcs");
    let document = parse_document(source).into_result().unwrap();
    let diagnostics = document.canonical_metadata().unwrap_err();
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE
    );

    let mismatch =
        include_str!("../../../docs/conformance/fcs5/source/invalid/resource-hash-mismatch.fcs");
    let document = parse_document(mismatch).into_result().unwrap();
    assert!(
        document.canonical_metadata().is_ok(),
        "hash comparison belongs to I5"
    );
}

#[test]
fn resolves_typed_references_and_enforces_resource_kinds_and_preview() {
    let metadata = canonical(
        r#"#fcs 5.0.0
format { profile: fragment; }
contributors { person alice { name: "Alice"; } }
credits { credit { role: "composer"; contributors: [@alice]; } }
resources {
    audio song { source: "audio/song.ogg"; mediaType: "audio/ogg"; }
    image cover { source: "cover.png"; mediaType: "image/png"; }
}
artwork { primary: @cover; }
sync { primaryAudio: @song; audioOffset: -100ms; preview: [30s, 45s); }
meta { custom: { "person": @alice, "sound": @song }; }
"#,
    );
    assert_eq!(metadata.artwork().unwrap().primary(), Some("cover"));
    assert_eq!(metadata.sync().unwrap().primary_audio(), Some("song"));
    assert_eq!(metadata.sync().unwrap().audio_offset().seconds(), -0.1);
    assert_eq!(
        metadata.sync().unwrap().preview().unwrap().start_seconds(),
        30.0
    );
    let CanonicalValue::ContributorReference(person) = metadata
        .meta()
        .unwrap()
        .get("custom")
        .and_then(|value| match value {
            CanonicalValue::Object(object) => object.get("person"),
            _ => None,
        })
        .unwrap()
    else {
        panic!("expected contributor reference");
    };
    assert_eq!(person, "alice");
}

#[test]
fn declaration_reordering_does_not_change_map_owned_metadata_but_credit_order_remains() {
    let first = canonical(
        r#"#fcs 5.0.0
format { profile: fragment; }
contributors { person alice { name: "Alice"; } person bob { name: "Bob"; } }
credits {
    credit { role: "composer"; contributors: [@alice]; }
    credit { role: "charter"; contributors: [@bob]; }
}
resources {
    binary one { source: "one.bin"; mediaType: "application/octet-stream"; }
    binary two { source: "two.bin"; mediaType: "application/octet-stream"; }
}
"#,
    );
    let reordered = canonical(
        r#"#fcs 5.0.0
format { profile: fragment; }
resources {
    binary two { mediaType: "application/octet-stream"; source: "two.bin"; }
    binary one { mediaType: "application/octet-stream"; source: "one.bin"; }
}
credits {
    credit { role: "composer"; contributors: [@alice]; }
    credit { role: "charter"; contributors: [@bob]; }
}
contributors { person bob { name: "Bob"; } person alice { name: "Alice"; } }
"#,
    );
    assert_eq!(first, reordered);
    assert_eq!(first.credits()[0].role(), "composer");
    assert_eq!(first.credits()[1].role(), "charter");
}

#[test]
fn rejects_invalid_preview_and_wrong_artwork_reference_type() {
    let document = parse_document(
        r#"#fcs 5.0.0
format { profile: fragment; }
resources { binary blob { source: "blob.bin"; mediaType: "application/octet-stream"; } }
artwork { primary: @blob; }
sync { preview: [-1s, 1s); }
"#,
    )
    .into_result()
    .unwrap();
    let diagnostics = document.canonical_metadata().unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::RESOURCE_TYPE_MISMATCH })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::TYPE_INVALID_OPERATION })
    );
}

#[test]
fn evaluates_static_metadata_choose_and_typed_empty_standard_arrays() {
    let metadata = canonical(
        r#"#fcs 5.0.0
format { profile: fragment; }
meta {
    alternativeTitles: [];
    tags: [];
    custom: { "selected": choose { when false => 1; else => 2; } };
}
"#,
    );
    assert!(matches!(
        metadata.meta().unwrap().get("alternativeTitles"),
        Some(CanonicalValue::Array {
            element_type: CanonicalValueType::String,
            values
        }) if values.is_empty()
    ));
    assert_eq!(
        metadata
            .meta()
            .unwrap()
            .get("custom")
            .and_then(|value| match value {
                CanonicalValue::Object(object) => object.get("selected"),
                _ => None,
            }),
        Some(&CanonicalValue::Int(2))
    );
}

#[test]
fn validates_revision_level_roles_required_resource_fields_and_references() {
    let document = parse_document(
        r#"#fcs 5.0.0
format { profile: fragment; }
meta { revision: -1; level: 1.0 / 0.0; }
contributors { person alice { aliases: [1]; } person alice { name: "Again"; } }
credits {
    credit { role: "custom(role)"; contributors: [@missing]; }
}
resources {
    image cover { source: "cover.png"; hash: "sha256:bad"; mediaType: "image/png"; }
    binary missingMedia { source: "missing.bin"; }
}
sync { primaryAudio: @missing; }
"#,
    )
    .into_result()
    .unwrap();
    let diagnostics = document.canonical_metadata().unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::TYPE_INVALID_OPERATION })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::NAME_DUPLICATE })
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == DiagnosticCode::SCHEMA_MISSING_REQUIRED_FIELD
        })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE })
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code() == DiagnosticCode::NAME_UNKNOWN })
    );
}
