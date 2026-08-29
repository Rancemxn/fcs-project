use fcs_model::{
    CanonicalImageSampling, CanonicalProfile, CanonicalProfileFeature, CanonicalRenderGeometryData,
    CanonicalRenderNodeKind, CanonicalValue, EntityKind,
};
use fcs_source::ResourceLimits;
use fcs_source::elaborator::CompileTimeLimits;
use fcs_source::parser::parse_document;
use tempfile::tempdir;

fn canonical(source: &str) -> fcs_model::CanonicalChart {
    parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_chart(CompileTimeLimits::default())
        .unwrap_or_else(|diagnostics| panic!("canonical chart lowering failed: {diagnostics:?}"))
}

#[test]
fn canonical_chart_aggregates_current_i3_products_and_identity() {
    let chart = canonical(
        r#"#fcs 5.0.0
format { profile: chart; features: [playable,]; }
meta { title: "Aggregate"; }
resources {
    audio song {
        source: "assets/song.ogg";
        hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000";
        mediaType: "audio/ogg";
    }
}
sync { primaryAudio: @song; audioOffset: 0s; }
extensions { extension("org.test.chart", 1.0.0) required { "mode": "test", "enabled": true, } }
tempoMap { 0beat -> 120bpm; }
lines {
    line main {
        tracks {
            track fade -> alpha: float {
                segments { [0s, 1s): 1.0 -> 0.5 using "linear"; }
            }
        }
    }
}
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );

    assert_eq!(chart.source_version().as_str(), "5.0.0");
    assert_eq!(chart.profile(), CanonicalProfile::Chart);
    assert!(
        chart
            .features()
            .contains(&CanonicalProfileFeature::Playable)
    );
    assert_eq!(chart.time_map().segments().count(), 1);
    assert_eq!(
        chart.metadata().meta().unwrap().get("title"),
        Some(&CanonicalValue::String("Aggregate".into()))
    );
    assert_eq!(chart.lines().lines().count(), 1);
    assert_eq!(chart.notes().notes().len(), 1);
    assert_eq!(chart.tracks().tracks().len(), 1);
    assert_eq!(chart.scroll().lines().len(), 1);
    assert_eq!(chart.required_extensions().len(), 1);
    let extension = chart.required_extensions().first().unwrap();
    assert_eq!(extension.namespace(), "org.test.chart");
    assert_eq!(extension.version(), "1.0.0");
    assert_eq!(extension.payload().entries()[0].key(), "mode");
    assert_eq!(
        extension.payload().entries()[0].value(),
        &CanonicalValue::String("test".into())
    );
    assert_eq!(extension.payload().entries()[1].key(), "enabled");
    assert_eq!(
        extension.payload().entries()[1].value(),
        &CanonicalValue::Bool(true)
    );
}

#[test]
fn canonical_chart_is_stable_when_top_level_declarations_are_reordered() {
    let first = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
lines { line main {} }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
"#,
    );
    let reordered = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
collections { notes { tap { id: "tap"; line: @main; gameplay.time: 1s; }; } }
lines { line main {} }
tempoMap { 0beat -> 120bpm; }
"#,
    );

    assert_eq!(first, reordered);
}

#[test]
fn canonical_render_image_binds_resource_and_rect_descriptors() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    image sprite {
        source: "assets/sprite.png";
        mediaType: "image/png";
        colorSpace: "srgb";
        alpha: "straight";
        sampling: "linear";
    }
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 4px; height: 4px; colorSpace: "linear-srgb"; }
    layer main {
        pass: "overlay";
        children {
            image spriteNode {
                resource: @sprite;
                destination.origin: vec2(0px, 0px);
                destination.size: vec2(4px, 4px);
                sourceRect.origin: vec2(0.0, 0.0);
                sourceRect.size: vec2(2.0, 2.0);
                sampling: "nearest";
            }
        }
    }
}
"#;
    let chart = parse_document(source)
        .into_result()
        .expect("source should parse")
        .canonical_chart_with_source(source, CompileTimeLimits::default())
        .unwrap_or_else(|diagnostics| panic!("Render canonical lowering failed: {diagnostics:?}"));
    let scene = chart.render().expect("canonical Render scene");

    assert_eq!(scene.nodes()[0].kind(), CanonicalRenderNodeKind::Image);
    let CanonicalRenderGeometryData::Image {
        resource,
        destination,
        source,
        sampling,
    } = scene.geometries()[0].data()
    else {
        panic!("expected canonical Image geometry");
    };
    assert_eq!(resource.namespace(), EntityKind::Resource);
    assert_eq!(destination.len(), 4);
    assert_eq!(source.as_ref().map(|values| values.len()), Some(4));
    assert_eq!(*sampling, CanonicalImageSampling::Nearest);
    let roots = chart.descriptors().expect("Render descriptors").roots();
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.geometry.destination.width"
            && root.owner() == scene.geometries()[0].id().value()
    }));
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.geometry.source.height"
            && root.owner() == scene.geometries()[0].id().value()
    }));
}

#[test]
fn canonical_source_text_shapes_against_the_resolved_font_bundle() {
    let source = r#"#fcs 5.0.0
format { profile: renderable; }
resources {
    font primary {
        source: "assets/primary.ttf";
        mediaType: "font/ttf";
    }
    font fallback {
        source: "assets/fallback.ttf";
        mediaType: "font/ttf";
    }
}
tempoMap { 0beat -> 120bpm; }
render profile 1.0.0 {
    viewport { width: 32px; height: 16px; }
    layer main {
        pass: "overlay";
        children {
            text label {
                content: "AB";
                font: @primary;
                fallbackFonts: [@fallback,];
                size: 12px;
                fill: solid(#FFFFFFFF);
            }
        }
    }
}
"#;
    let workspace = tempdir().expect("temporary workspace");
    std::fs::create_dir(workspace.path().join("assets")).expect("asset directory");
    let font = include_bytes!("../../../docs/conformance/render/assets/fcs-test-font.ttf");
    std::fs::write(workspace.path().join("assets/primary.ttf"), font).expect("primary font");
    std::fs::write(
        workspace.path().join("assets/fallback.ttf"),
        font_with_b_mapping(font, -65),
    )
    .expect("fallback font");
    let document = parse_document(source)
        .into_result()
        .expect("source should parse");
    let compilation = document
        .canonical_compilation_with_source(
            source,
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("Text compilation failed: {diagnostics:?}"));
    let scene = compilation
        .chart()
        .render()
        .expect("canonical Render scene");
    let CanonicalRenderGeometryData::Text {
        glyph_runs,
        origin: _,
    } = scene.geometries()[0].data()
    else {
        panic!("expected canonical Text geometry");
    };
    assert_eq!(glyph_runs, &vec![0, 1]);
    assert_eq!(scene.glyph_runs().len(), 2);
    let primary = &scene.glyph_runs()[0];
    assert_eq!(primary.font().textual().as_str(), "primary");
    assert_eq!(primary.face_index(), 0);
    assert_eq!(primary.run_offset(), [0.0, 0.0]);
    assert_eq!(primary.glyphs().len(), 1);
    assert_eq!(primary.glyphs()[0].glyph_id, 1);
    assert_eq!(primary.glyphs()[0].x_advance, 1.0);
    assert_eq!(primary.glyphs()[0].y_advance, 0.0);
    assert_eq!(primary.glyphs()[0].x_offset, 0.0);
    assert_eq!(primary.glyphs()[0].y_offset, 0.0);
    let fallback = &scene.glyph_runs()[1];
    assert_eq!(fallback.font().textual().as_str(), "fallback");
    assert_eq!(fallback.run_offset(), [1.0, 0.0]);
    assert_eq!(fallback.glyphs().len(), 1);
    assert_eq!(fallback.glyphs()[0].glyph_id, 1);
    assert_eq!(fallback.glyphs()[0].x_advance, 1.0);
    assert_eq!(fallback.glyphs()[0].y_advance, 0.0);
    assert_eq!(fallback.glyphs()[0].x_offset, 0.0);
    assert_eq!(fallback.glyphs()[0].y_offset, 0.0);

    std::fs::write(
        workspace.path().join("assets/primary.ttf"),
        font_with_b_mapping(font, -66),
    )
    .expect("primary font with missing sentinel");
    let missing_source = source.replace("content: \"AB\"", "content: \"B\"");
    let missing_document = parse_document(&missing_source)
        .into_result()
        .expect("missing-sentinel source should parse");
    let missing = missing_document
        .canonical_compilation_with_source(
            &missing_source,
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .expect("glyph zero should continue to the fallback font");
    let missing_scene = missing.chart().render().expect("missing-sentinel scene");
    assert_eq!(missing_scene.glyph_runs().len(), 1);
    assert_eq!(
        missing_scene.glyph_runs()[0].font().textual().as_str(),
        "fallback"
    );
    std::fs::write(workspace.path().join("assets/primary.ttf"), font)
        .expect("restore primary font");

    assert!(compilation.resources().get("primary").is_some());
    assert!(compilation.resources().get("fallback").is_some());
    let roots = compilation
        .chart()
        .descriptors()
        .expect("Render descriptors")
        .roots();
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.geometry.origin"
            && root.owner() == scene.geometries()[0].id().value()
    }));
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.glyphRun.size"
            && root.owner() == scene.glyph_runs()[0].id().value()
    }));
    assert!(roots.iter().any(|root| {
        root.target_path() == "render.glyphRun.size"
            && root.owner() == scene.glyph_runs()[1].id().value()
    }));

    let empty_source = source.replace("content: \"AB\"", "content: \"\"");
    let empty_document = parse_document(&empty_source)
        .into_result()
        .expect("empty Text source should parse");
    let empty = empty_document
        .canonical_compilation_with_source(
            &empty_source,
            CompileTimeLimits::default(),
            workspace.path(),
            ResourceLimits::default(),
        )
        .expect("empty Text content should compile");
    let empty_scene = empty.chart().render().expect("empty Render scene");
    assert_eq!(empty_scene.glyph_runs().len(), 1);
    assert!(empty_scene.glyph_runs()[0].glyphs().is_empty());

    let chart_only = document
        .canonical_chart_with_source(source, CompileTimeLimits::default())
        .expect_err("Text chart lowering must not read workspace bytes");
    assert!(chart_only[0].message().contains("resolved resource bundle"));

    for (invalid_source, expected_message) in [
        (
            source.replace("content: \"AB\"", "content: \"C\""),
            "no glyph",
        ),
        (
            source.replace("content: \"AB\"", "content: \"\\u{0001}\""),
            "forbidden",
        ),
        (
            source.replace(
                "fallbackFonts: [@fallback,];",
                "fallbackFonts: [@fallback,];\n                language: \"ar\";",
            ),
            "language",
        ),
    ] {
        let invalid_document = parse_document(&invalid_source)
            .into_result()
            .expect("invalid Text boundary source should parse");
        let diagnostics = invalid_document
            .canonical_compilation_with_source(
                &invalid_source,
                CompileTimeLimits::default(),
                workspace.path(),
                ResourceLimits::default(),
            )
            .expect_err("invalid Text boundary should fail");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected_message)),
            "expected {expected_message:?}, got {diagnostics:?}"
        );
    }
}

fn font_with_b_mapping(font: &[u8], id_delta: i16) -> Vec<u8> {
    let mut mapped = font.to_vec();
    let count = usize::from(u16::from_be_bytes([mapped[4], mapped[5]]));
    let mut cmap_record = None;
    let mut head_offset = None;
    for index in 0..count {
        let record = 12 + index * 16;
        let tag = &mapped[record..record + 4];
        let offset = usize::try_from(u32::from_be_bytes(
            mapped[record + 8..record + 12]
                .try_into()
                .expect("font offset"),
        ))
        .expect("font offset fits");
        if tag == b"cmap" {
            cmap_record = Some((record, offset));
        }
        if tag == b"head" {
            head_offset = Some(offset);
        }
    }
    let (cmap_record, cmap_offset) = cmap_record.expect("cmap record");
    let subtable_offset = cmap_offset
        + usize::try_from(u32::from_be_bytes(
            mapped[cmap_offset + 8..cmap_offset + 12]
                .try_into()
                .expect("cmap offset"),
        ))
        .expect("cmap offset fits");
    mapped[subtable_offset + 14..subtable_offset + 16].copy_from_slice(&0x0042u16.to_be_bytes());
    mapped[subtable_offset + 20..subtable_offset + 22].copy_from_slice(&0x0042u16.to_be_bytes());
    mapped[subtable_offset + 24..subtable_offset + 26].copy_from_slice(&id_delta.to_be_bytes());

    let table_length = usize::try_from(u32::from_be_bytes(
        mapped[cmap_record + 12..cmap_record + 16]
            .try_into()
            .expect("cmap length"),
    ))
    .expect("cmap length fits");
    let checksum = |bytes: &[u8]| {
        bytes
            .chunks(4)
            .map(|chunk| {
                let mut word = [0; 4];
                word[..chunk.len()].copy_from_slice(chunk);
                u32::from_be_bytes(word)
            })
            .fold(0, u32::wrapping_add)
    };
    let cmap_checksum = checksum(&mapped[cmap_offset..cmap_offset + table_length]);
    mapped[cmap_record + 4..cmap_record + 8].copy_from_slice(&cmap_checksum.to_be_bytes());
    let head_offset = head_offset.expect("head record");
    mapped[head_offset + 8..head_offset + 12].fill(0);
    let adjustment = 0xb1b0_afba_u32.wrapping_sub(checksum(&mapped));
    mapped[head_offset + 8..head_offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    mapped
}

#[test]
fn canonical_chart_includes_direct_template_and_generator_judgelines() {
    let chart = canonical(
        r#"#fcs 5.0.0
format { profile: chart; }
tempoMap { 0beat -> 120bpm; }
definitions {
    template Line makeJudge() {
        return Line { id: "made"; };
    }
}
collections {
    judgelines {
        Line { id: "direct"; zOrder: 7; };
        makeJudge();
        generate i: int in 0..=0 step 1 {
            emit Line { id: "generator"; };
        }
    }
    notes {
        tap { id: "direct-note"; line: @direct; gameplay.time: 1s; };
        tap { id: "template-note"; line: @made; gameplay.time: 2s; };
        tap { id: "generator-note"; line: @generator; gameplay.time: 3s; };
    }
}
"#,
    );

    assert_eq!(chart.lines().lines().count(), 3);
    assert_eq!(chart.notes().notes().len(), 3);
    assert_eq!(chart.scroll().lines().len(), 3);
    let direct = chart
        .lines()
        .line_by_textual_id("direct")
        .expect("direct emitted Line should enter the canonical graph");
    assert_eq!(direct.base().z_order(), 7);
    assert_eq!(direct.base().position().x(), 0.0);
    assert_eq!(direct.base().position().y(), 0.0);
    assert!(direct.inherit().position());
    assert!(!direct.inherit().scroll());
    assert_eq!(
        chart
            .lines()
            .line_by_textual_id("made")
            .expect("template-produced Line should enter the canonical graph")
            .id()
            .textual()
            .as_str(),
        "made"
    );
    assert!(chart.lines().line_by_textual_id("generator").is_some());
}
