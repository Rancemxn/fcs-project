use fcs_source::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;

const SYNC: &str = "sync { primaryAudio: @song; audioOffset: 0s; }";
const LINE: &str = "lines { line main {} }";
const RENDER: &str = "render profile 1.0.0 {}";
const HASH: &str =
    "hash: \"sha256:0000000000000000000000000000000000000000000000000000000000000000\";";

fn complete_source(profile: &str, features: Option<&[&str]>) -> String {
    let features = match features {
        None => String::new(),
        Some(values) => format!(" features: [{}];", values.join(", ")),
    };
    format!(
        r#"#fcs 5.0.0
format {{ profile: {profile};{features} }}
meta {{
    title: "Profile matrix";
    documentId: "org.example.profile-matrix";
    chartVersion: "1";
    license: "CC0-1.0";
}}
credits {{ credit {{ role: "charter"; }} }}
resources {{
    audio song {{
        source: "song.ogg";
        {HASH}
        mediaType: "audio/ogg";
    }}
}}
{SYNC}
tempoMap {{ 0beat -> 120bpm; }}
{LINE}
{RENDER}
"#
    )
}

fn parse(source: &str) -> fcs_source::ast::Document {
    parse_document(source)
        .into_result()
        .unwrap_or_else(|diagnostics| panic!("source should parse: {diagnostics:?}\n{source}"))
}

fn validate(source: &str) -> Result<(), Vec<Diagnostic>> {
    parse(source).validate_profile_requirements(CompileTimeLimits::default())
}

fn expect_requirement(source: &str, message_fragment: &str) -> Vec<Diagnostic> {
    let diagnostics = validate(source).expect_err("profile requirements should fail");
    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.code() == DiagnosticCode::PROFILE_REQUIREMENT_MISSING
                && diagnostic.stage() == DiagnosticStage::Canonical
                && diagnostic.primary_span().start <= diagnostic.primary_span().end
                && diagnostic.primary_span().end <= source.len()
        }),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains(message_fragment)),
        "missing {message_fragment:?} in {diagnostics:?}"
    );
    diagnostics
}

#[test]
fn every_legal_primary_profile_and_orthogonal_feature_combination_is_accepted() {
    let cases: &[(&str, Option<&[&str]>)] = &[
        ("fragment", None),
        ("fragment", Some(&[])),
        ("chart", None),
        ("chart", Some(&["playable"])),
        ("chart", Some(&["renderable"])),
        ("chart", Some(&["playable", "renderable"])),
        ("playable", None),
        ("playable", Some(&["playable"])),
        ("playable", Some(&["renderable"])),
        ("playable", Some(&["playable", "renderable"])),
        ("renderable", None),
        ("renderable", Some(&["renderable"])),
        ("renderable", Some(&["playable"])),
        ("renderable", Some(&["playable", "renderable"])),
        ("publishable", Some(&["playable"])),
        ("publishable", Some(&["renderable"])),
        ("publishable", Some(&["playable", "renderable"])),
    ];

    for (profile, features) in cases {
        let source = complete_source(profile, *features);
        validate(&source).unwrap_or_else(|diagnostics| {
            panic!("{profile} {features:?} should be legal: {diagnostics:?}")
        });
    }
}

#[test]
fn minimal_profiles_do_not_inherit_orthogonal_or_publishable_requirements() {
    let cases = [
        (
            "chart",
            r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
"#,
        ),
        (
            "playable",
            r#"#fcs 5.0.0
format { profile: playable; }
resources { audio song { source: "song.ogg"; mediaType: "audio/ogg"; } }
sync { primaryAudio: @song; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
"#,
        ),
        (
            "renderable",
            r#"#fcs 5.0.0
format { profile: renderable; }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {}
"#,
        ),
        (
            "publishable-playable",
            r#"#fcs 5.0.0
format { profile: publishable; features: [playable]; }
meta { title: "P"; documentId: "p"; chartVersion: "1"; license: "CC0-1.0"; }
credits { credit { role: "charter"; } }
resources { audio song {
    source: "song.ogg";
    hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000";
    mediaType: "audio/ogg";
} }
sync { primaryAudio: @song; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
"#,
        ),
        (
            "publishable-renderable",
            r#"#fcs 5.0.0
format { profile: publishable; features: [renderable]; }
meta { title: "P"; documentId: "p"; chartVersion: "1"; license: "CC0-1.0"; }
credits { credit { role: "charter"; } }
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {}
"#,
        ),
    ];

    for (name, source) in cases {
        validate(source)
            .unwrap_or_else(|diagnostics| panic!("minimal {name} failed: {diagnostics:?}"));
    }
}

#[test]
fn fragment_allows_missing_chart_inputs_but_rejects_each_declared_capability() {
    let minimal = "#fcs 5.0.0\nformat { profile: fragment; features: []; }\n";
    validate(minimal).expect("an empty fragment feature list declares no capability");

    for feature in ["playable", "renderable"] {
        let source =
            format!("#fcs 5.0.0\nformat {{ profile: fragment; features: [{feature}]; }}\n");
        let diagnostics = expect_requirement(&source, "fragment profile cannot declare");
        let expected_start = source.find(feature).unwrap();
        assert_eq!(diagnostics[0].primary_span().start, expected_start);
        assert_eq!(
            diagnostics[0].primary_span().end,
            expected_start + feature.len()
        );
    }
}

#[test]
fn chart_capability_requires_a_present_and_valid_tempo_model() {
    let missing = complete_source("chart", None).replace("tempoMap { 0beat -> 120bpm; }\n", "");
    expect_requirement(&missing, "requires a tempoMap");

    let invalid =
        complete_source("chart", None).replace("tempoMap { 0beat -> 120bpm; }", "tempoMap {}");
    let diagnostics = validate(&invalid).expect_err("an empty tempo map should be invalid");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::TEMPO_INVALID);
    assert_eq!(diagnostics[0].stage(), DiagnosticStage::Canonical);
}

#[test]
fn playable_capability_requires_sync_primary_audio_and_an_elaborated_line() {
    let complete = complete_source("playable", None);
    expect_requirement(&complete.replace(&format!("{SYNC}\n"), ""), "sync block");
    expect_requirement(
        &complete.replace(SYNC, "sync { audioOffset: 0s; }"),
        "sync.primaryAudio",
    );
    expect_requirement(&complete.replace(&format!("{LINE}\n"), ""), "gameplay Line");

    let both = complete_source("chart", Some(&["playable", "renderable"]))
        .replace(&format!("{SYNC}\n"), "")
        .replace(&format!("{LINE}\n"), "")
        .replace(&format!("{RENDER}\n"), "");
    let diagnostics = expect_requirement(&both, "sync block");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("gameplay Line"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("Render scene envelope"))
    );
}

#[test]
fn playable_capability_counts_a_line_created_only_by_elaboration() {
    let source = r#"#fcs 5.0.0
format { profile: playable; }
resources { audio song { source: "song.ogg"; mediaType: "audio/ogg"; } }
sync { primaryAudio: @song; }
tempoMap { 0beat -> 120bpm; }
definitions {
    template Line makeLine() {
        return Line { id: "generated"; };
    }
}
collections { judgelines { makeLine(); } }
"#;

    validate(source).expect("a template-produced Line satisfies the playable requirement");
}

#[test]
fn renderable_capability_requires_only_the_i5_representable_render_envelope() {
    let source = complete_source("renderable", None).replace(&format!("{RENDER}\n"), "");
    expect_requirement(&source, "Render scene envelope");

    let empty_envelope = complete_source("renderable", None);
    validate(&empty_envelope).expect(
        "I5 validates the versioned Render envelope; payload and resource closure belong to I9",
    );
}

#[test]
fn publishable_profile_requires_capability_metadata_credit_and_declared_hashes() {
    expect_requirement(
        &complete_source("publishable", None),
        "at least one playable or renderable feature",
    );

    let complete = complete_source("publishable", Some(&["playable"]));
    for (field, spelling) in [
        ("meta.title", "    title: \"Profile matrix\";\n"),
        (
            "meta.documentId",
            "    documentId: \"org.example.profile-matrix\";\n",
        ),
        ("meta.chartVersion", "    chartVersion: \"1\";\n"),
        ("meta.license", "    license: \"CC0-1.0\";\n"),
    ] {
        expect_requirement(&complete.replace(spelling, ""), field);
    }
    expect_requirement(
        &complete.replace("credits { credit { role: \"charter\"; } }\n", ""),
        "at least one credit",
    );
    let missing_hash = complete.replace(&format!("        {HASH}\n"), "");
    let diagnostics = expect_requirement(&missing_hash, "declared SHA-256 hash");
    let resource_start = missing_hash.find("song").unwrap();
    assert_eq!(diagnostics[0].primary_span().start, resource_start);
}

#[test]
fn existing_metadata_errors_remain_more_specific_than_profile_requirements() {
    let source = complete_source("publishable", Some(&["playable"]))
        .replace("title: \"Profile matrix\"", "title: 1");
    let diagnostics = validate(&source).expect_err("invalid metadata should fail");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::TYPE_MISMATCH);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code() != DiagnosticCode::PROFILE_REQUIREMENT_MISSING)
    );
}

#[test]
fn profile_diagnostics_are_repeatable_and_canonical_chart_uses_the_same_gate() {
    let source = "#fcs 5.0.0\nformat { profile: publishable; }\ntempoMap { 0beat -> 120bpm; }\n";
    let document = parse(source);
    let first = document
        .validate_profile_requirements(CompileTimeLimits::default())
        .unwrap_err();
    let second = document
        .validate_profile_requirements(CompileTimeLimits::default())
        .unwrap_err();
    assert_eq!(first, second);
    assert!(first.windows(2).all(|pair| {
        let left = &pair[0];
        let right = &pair[1];
        (
            left.primary_span().start,
            left.primary_span().end,
            left.message(),
        ) <= (
            right.primary_span().start,
            right.primary_span().end,
            right.message(),
        )
    }));

    let chart_errors = document
        .canonical_chart(CompileTimeLimits::default())
        .expect_err("canonical chart construction must enforce profile requirements");
    assert_eq!(chart_errors, first);
}

#[test]
fn tempo_less_fragment_profile_validation_does_not_invent_a_chart_time_model() {
    let source = "#fcs 5.0.0\nformat { profile: fragment; }\n";
    let document = parse(source);
    document
        .validate_profile_requirements(CompileTimeLimits::default())
        .expect("fragment does not require a tempo map");
    let diagnostics = document
        .canonical_chart(CompileTimeLimits::default())
        .expect_err("CanonicalChart still requires the section 17 tempo map");
    assert_eq!(diagnostics[0].code(), DiagnosticCode::TEMPO_INVALID);
}
