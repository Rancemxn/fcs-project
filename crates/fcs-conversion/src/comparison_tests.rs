use super::*;
use fcs_model::{
    AudioOffset, Beat, CanonicalBundledResource, CanonicalChartScrollTempoPoint, CanonicalColor,
    CanonicalCompilation, CanonicalJudgeShape, CanonicalLineBase, CanonicalLineGraph,
    CanonicalLineInherit, CanonicalMetadata, CanonicalNote, CanonicalNoteGameplay,
    CanonicalNoteKind, CanonicalNotePresentation, CanonicalNoteScorePolicy, CanonicalNoteSet,
    CanonicalNoteSide, CanonicalNoteSoundPolicy, CanonicalObject, CanonicalPreview,
    CanonicalProfile, CanonicalResource, CanonicalResourceBundle, CanonicalResourceKind,
    CanonicalScrollCoordinate, CanonicalScrollLine, CanonicalScrollSet, CanonicalScrollTempo,
    CanonicalSourceVersion, CanonicalSync, CanonicalTextualId, CanonicalTime, CanonicalTrackSet,
    CanonicalValue, CanonicalVec2, ChartTimeMap, DistributionMetadata, EntityKind, StableId,
    StableIdRegistry, TempoPoint,
};

fn record(sink: &mut Mismatches<'_>, domain: &str, field: &str) {
    sink.push(ComparisonMismatch::new(
        domain, "discrete", field, "expected", "actual", None,
    ));
}

#[test]
fn a_drop_authorization_only_suppresses_its_own_selector() {
    let dropped = vec!["motion.line.parent".to_owned()];
    let mut sink = Mismatches::new(&dropped);
    record(&mut sink, "motion", "lines[0].parent");
    record(&mut sink, "motion", "lines[0].inherit");
    record(&mut sink, "motion", "lines[0].documentOrder");
    let kept: Vec<String> = sink
        .into_inner()
        .iter()
        .map(|mismatch| mismatch.selector().to_owned())
        .collect();
    assert_eq!(kept, ["motion.line.inherit", "motion.line.documentOrder"]);
}

#[test]
fn an_entity_wide_selector_does_not_cover_other_entities() {
    let dropped = vec!["motion.line".to_owned()];
    let mut sink = Mismatches::new(&dropped);
    record(&mut sink, "motion", "lines[0].parent");
    record(&mut sink, "motion", "tracks[0].header");
    let kept = sink.into_inner();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].selector(), "motion.track.header");
}

/// A chart holding `notes` on one Line, with everything else minimal.
fn chart_with_notes(notes: Vec<CanonicalNote>) -> CanonicalChart {
    chart_with_time_map(notes, time_map())
}

fn chart_with_time_map(notes: Vec<CanonicalNote>, time_map: ChartTimeMap) -> CanonicalChart {
    CanonicalChart::new(
        CanonicalSourceVersion::new("5.0.0").unwrap(),
        CanonicalProfile::Chart,
        [],
        time_map,
        CanonicalMetadata::new(
            None,
            Default::default(),
            Vec::new(),
            Default::default(),
            None,
            None,
        ),
        CanonicalLineGraph::new([CanonicalLine::new(
            ids().0,
            None,
            0,
            CanonicalLineBase::default(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap()])
        .unwrap(),
        CanonicalNoteSet::new(notes).unwrap(),
        CanonicalTrackSet::new(Vec::new()).unwrap(),
        CanonicalScrollSet::new(Vec::new()).unwrap(),
        [],
    )
}

fn chart_with_sync(sync: CanonicalSync) -> CanonicalChart {
    let chart = chart_with_notes(vec![tap_note()]);
    CanonicalChart::new(
        chart.source_version().clone(),
        chart.profile(),
        chart.features().iter().copied(),
        chart.time_map().clone(),
        CanonicalMetadata::new(
            chart.metadata().meta().cloned(),
            chart.metadata().contributors().clone(),
            chart.metadata().credits().to_vec(),
            chart.metadata().resources().clone(),
            chart.metadata().artwork().cloned(),
            Some(sync),
        ),
        chart.lines().clone(),
        chart.notes().clone(),
        chart.tracks().clone(),
        chart.scroll().clone(),
        chart.required_extensions().iter().cloned(),
    )
}

fn chart_with_meta(value: &str) -> CanonicalChart {
    let chart = chart_with_notes(Vec::new());
    CanonicalChart::new(
        chart.source_version().clone(),
        chart.profile(),
        chart.features().iter().copied(),
        chart.time_map().clone(),
        CanonicalMetadata::new(
            Some(BTreeMap::from([(
                "private".to_owned(),
                CanonicalValue::String(value.to_owned()),
            )])),
            chart.metadata().contributors().clone(),
            chart.metadata().credits().to_vec(),
            chart.metadata().resources().clone(),
            chart.metadata().artwork().cloned(),
            chart.metadata().sync().cloned(),
        ),
        chart.lines().clone(),
        chart.notes().clone(),
        chart.tracks().clone(),
        chart.scroll().clone(),
        chart.required_extensions().iter().cloned(),
    )
}

fn chart_with_resource(resource: CanonicalResource) -> CanonicalChart {
    let mut resources = BTreeMap::new();
    resources.insert(resource.id().to_owned(), resource);
    let (line_id, _) = ids();
    CanonicalChart::new(
        CanonicalSourceVersion::new("5.0.0").unwrap(),
        CanonicalProfile::Chart,
        [],
        time_map(),
        CanonicalMetadata::new(None, Default::default(), Vec::new(), resources, None, None),
        CanonicalLineGraph::new([CanonicalLine::new(
            line_id,
            None,
            0,
            CanonicalLineBase::default(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap()])
        .unwrap(),
        CanonicalNoteSet::new(Vec::new()).unwrap(),
        CanonicalTrackSet::new(Vec::new()).unwrap(),
        CanonicalScrollSet::new(Vec::new()).unwrap(),
        [],
    )
}

fn compilation_with_resource(bytes: &[u8]) -> CanonicalCompilation {
    let resource = CanonicalResource::new(
        "asset",
        CanonicalResourceKind::Binary,
        "application/octet-stream",
        None,
        CanonicalObject::new(Vec::new()).unwrap(),
    );
    let bundled = CanonicalBundledResource::new(resource.clone(), bytes.to_vec()).unwrap();
    let resources = CanonicalResourceBundle::new(vec![bundled]).unwrap();
    CanonicalCompilation::new(
        chart_with_resource(resource),
        resources,
        DistributionMetadata::empty(),
    )
}

fn chart_with_scroll(coordinate: CanonicalScrollCoordinate) -> CanonicalChart {
    let (line_id, _) = ids();
    CanonicalChart::new(
        CanonicalSourceVersion::new("5.0.0").unwrap(),
        CanonicalProfile::Chart,
        [],
        time_map(),
        CanonicalMetadata::new(
            None,
            Default::default(),
            Vec::new(),
            Default::default(),
            None,
            None,
        ),
        CanonicalLineGraph::new([CanonicalLine::new(
            line_id.clone(),
            None,
            0,
            CanonicalLineBase::default(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap()])
        .unwrap(),
        CanonicalNoteSet::new(Vec::new()).unwrap(),
        CanonicalTrackSet::new(Vec::new()).unwrap(),
        CanonicalScrollSet::new(vec![
            CanonicalScrollLine::new(line_id, coordinate, 1.0, false, 120.0, 0.0, 0.0).unwrap(),
        ])
        .unwrap(),
        [],
    )
}

fn chart_with_parent(parent_name: &str) -> CanonicalChart {
    let mut registry = StableIdRegistry::new();
    let parent_a = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("parent-a").unwrap(),
        )
        .unwrap();
    let parent_b = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("parent-b").unwrap(),
        )
        .unwrap();
    let child = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("child").unwrap(),
        )
        .unwrap();
    let parent = match parent_name {
        "parent-a" => parent_a.clone(),
        "parent-b" => parent_b.clone(),
        _ => panic!("unknown test parent {parent_name}"),
    };
    let lines = CanonicalLineGraph::new([
        CanonicalLine::new(
            parent_a,
            None,
            0,
            line_base((10.0, 0.0)),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap(),
        CanonicalLine::new(
            parent_b,
            None,
            0,
            line_base((20.0, 0.0)),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap(),
        CanonicalLine::new(
            child,
            Some(parent),
            1,
            line_base((1.0, 0.0)),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap(),
    ])
    .unwrap();
    CanonicalChart::new(
        CanonicalSourceVersion::new("5.0.0").unwrap(),
        CanonicalProfile::Chart,
        [],
        time_map(),
        CanonicalMetadata::new(
            None,
            Default::default(),
            Vec::new(),
            Default::default(),
            None,
            None,
        ),
        lines,
        CanonicalNoteSet::new(Vec::new()).unwrap(),
        CanonicalTrackSet::new(Vec::new()).unwrap(),
        CanonicalScrollSet::new(Vec::new()).unwrap(),
        [],
    )
}

fn chart_with_lines(lines: Vec<CanonicalLine>) -> CanonicalChart {
    CanonicalChart::new(
        CanonicalSourceVersion::new("5.0.0").unwrap(),
        CanonicalProfile::Chart,
        [],
        time_map(),
        CanonicalMetadata::new(
            None,
            Default::default(),
            Vec::new(),
            Default::default(),
            None,
            None,
        ),
        CanonicalLineGraph::new(lines).unwrap(),
        CanonicalNoteSet::new(Vec::new()).unwrap(),
        CanonicalTrackSet::new(Vec::new()).unwrap(),
        CanonicalScrollSet::new(Vec::new()).unwrap(),
        [],
    )
}

fn line_base(position: (f64, f64)) -> CanonicalLineBase {
    CanonicalLineBase::new(
        CanonicalVec2::new(position.0, position.1).unwrap(),
        0.0,
        CanonicalVec2::new(1.0, 1.0).unwrap(),
        1.0,
        CanonicalVec2::new(0.0, 0.0).unwrap(),
        CanonicalVec2::new(0.5, 0.5).unwrap(),
        120.0,
        0.0,
        0.0,
        false,
        0,
    )
    .unwrap()
}

fn time_map() -> ChartTimeMap {
    ChartTimeMap::new([TempoPoint {
        beat: Beat::zero(),
        bpm: 120.0,
    }])
    .unwrap()
}

fn scroll_coordinate(points: impl IntoIterator<Item = (f64, f64)>) -> CanonicalScrollCoordinate {
    CanonicalScrollCoordinate::new(
        points
            .into_iter()
            .map(|(chart_time, bpm)| CanonicalChartScrollTempoPoint::new(chart_time, bpm).unwrap()),
    )
    .unwrap()
}

fn ids() -> (StableId, StableId) {
    let mut registry = StableIdRegistry::new();
    let line = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("main").unwrap(),
        )
        .unwrap();
    let note = registry
        .insert(
            EntityKind::Note,
            CanonicalTextualId::explicit("note").unwrap(),
        )
        .unwrap();
    (line, note)
}

fn note_with(
    kind: CanonicalNoteKind,
    time: CanonicalTime,
    document_order: u64,
    sound_policy: CanonicalNoteSoundPolicy,
) -> CanonicalNote {
    let (line, note) = ids();
    let gameplay = CanonicalNoteGameplay::new(
        kind,
        line,
        time,
        None,
        CanonicalNoteSide::Above,
        true,
        CanonicalJudgeShape::LineDefault,
        sound_policy,
        CanonicalNoteScorePolicy::Default,
    )
    .unwrap();
    let presentation = CanonicalNotePresentation::new(
        0.0,
        1.0,
        0.0,
        0.0,
        1.0,
        1.0,
        1.0,
        0.0,
        CanonicalColor::rgba(255, 255, 255, 255),
        None,
        true,
        None,
        None,
    )
    .unwrap();
    CanonicalNote::new(note, kind, document_order, gameplay, presentation).unwrap()
}

fn tap_note_with_time(time: CanonicalTime) -> CanonicalNote {
    note_with(
        CanonicalNoteKind::Tap,
        time,
        0,
        CanonicalNoteSoundPolicy::Default,
    )
}

fn tap_note() -> CanonicalNote {
    tap_note_with_time(time_map().chart_time(Beat::zero()).unwrap())
}

#[test]
fn a_gameplay_drop_cannot_authorize_losing_every_note() {
    // Section 6.3 scopes a drop to domain/entity/field, so authorizing the
    // gameplay domain never authorizes losing the notes themselves: the
    // reparse that kept no note is not canonically equivalent.
    let comparison = compare_canonical_charts_with_budgets(
        &chart_with_notes(vec![tap_note()]),
        &chart_with_notes(Vec::new()),
        &BTreeMap::new(),
        &["gameplay.note".to_owned()],
    );
    assert!(!comparison.is_equivalent());
    let counts: Vec<&str> = comparison
        .mismatches()
        .iter()
        .map(ComparisonMismatch::field)
        .collect();
    assert!(counts.contains(&"note.count"));
    assert!(counts.iter().any(|field| field.starts_with("notes[")));
}

#[test]
fn a_field_selector_keeps_unrelated_note_failures() {
    let expected = chart_with_notes(vec![note_with(
        CanonicalNoteKind::Tap,
        CanonicalTime::from_chart_time_seconds(1.0).unwrap(),
        0,
        CanonicalNoteSoundPolicy::Default,
    )]);
    let actual = chart_with_notes(vec![note_with(
        CanonicalNoteKind::Drag,
        CanonicalTime::from_chart_time_seconds(1.0).unwrap(),
        0,
        CanonicalNoteSoundPolicy::None,
    )]);
    let comparison = compare_canonical_charts_with_budgets(
        &expected,
        &actual,
        &BTreeMap::new(),
        &["gameplay.note.soundPolicy".to_owned()],
    );

    assert!(!comparison.is_equivalent());
    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.selector() == "gameplay.note.kind")
    );
    assert!(
        !comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.selector() == "gameplay.note.soundPolicy")
    );
    assert_eq!(
        comparison.unverified_selectors(),
        ["gameplay.note.soundPolicy"]
    );
}

#[test]
fn a_document_order_selector_cannot_suppress_structure() {
    let time = CanonicalTime::from_chart_time_seconds(1.0).unwrap();
    let expected = chart_with_notes(vec![note_with(
        CanonicalNoteKind::Tap,
        time,
        0,
        CanonicalNoteSoundPolicy::Default,
    )]);
    let actual = chart_with_notes(vec![note_with(
        CanonicalNoteKind::Tap,
        time,
        1,
        CanonicalNoteSoundPolicy::Default,
    )]);
    let comparison = compare_canonical_charts_with_budgets(
        &expected,
        &actual,
        &BTreeMap::new(),
        &["gameplay.note.documentOrder".to_owned()],
    );

    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.selector() == "gameplay.note.documentOrder")
    );
}

#[test]
fn a_timing_sync_selector_keeps_resource_and_metadata_facts() {
    let expected = chart_with_sync(
        CanonicalSync::new(
            Some("song-a".into()),
            AudioOffset::new(0.0).unwrap(),
            CanonicalPreview::new(1.0, 2.0),
        )
        .unwrap(),
    );
    let actual = chart_with_sync(
        CanonicalSync::new(
            Some("song-b".into()),
            AudioOffset::new(0.5).unwrap(),
            CanonicalPreview::new(2.0, 3.0),
        )
        .unwrap(),
    );
    let comparison = compare_canonical_charts_with_budgets(
        &expected,
        &actual,
        &BTreeMap::new(),
        &["timing.sync".to_owned()],
    );

    let selectors = comparison
        .mismatches()
        .iter()
        .map(ComparisonMismatch::selector)
        .collect::<Vec<_>>();
    assert_eq!(
        selectors,
        ["resource.sync.primaryAudio", "metadata.sync.preview"]
    );
}

#[test]
fn same_scroll_speed_with_different_integrated_distance_is_not_equivalent() {
    let expected = chart_with_scroll(scroll_coordinate([(0.0, 120.0), (1.0, 240.0)]));
    let actual = chart_with_scroll(scroll_coordinate([(0.0, 120.0), (1.0, 120.0)]));
    assert_eq!(
        expected.scroll().lines()[0].speed(),
        actual.scroll().lines()[0].speed()
    );

    let comparison = compare_canonical_charts(&expected, &actual);

    assert!(!comparison.is_equivalent());
    assert!(
        comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.metric() == "scroll.distance")
    );
    assert!(compare_canonical_charts(&expected, &expected).is_equivalent());
}

#[test]
fn aggregate_mismatch_values_are_fixed_sha256_fingerprints() {
    const SECRET: &str = "secret-marker-that-must-not-enter-a-conversion-report-0123456789-abcdefghijklmnopqrstuvwxyz";
    let expected = chart_with_meta(&format!("{SECRET}-expected"));
    let actual = chart_with_meta(&format!("{SECRET}-actual"));

    let comparison = compare_canonical_charts(&expected, &actual);
    let mismatch = comparison
        .mismatches()
        .iter()
        .find(|mismatch| mismatch.field() == "meta")
        .unwrap();
    assert_eq!(
        mismatch.expected(),
        aggregate_fingerprint(expected.metadata().meta())
    );

    for (fingerprint, raw) in [
        (
            mismatch.expected(),
            format!("{:?}", expected.metadata().meta()),
        ),
        (mismatch.actual(), format!("{:?}", actual.metadata().meta())),
    ] {
        assert!(!fingerprint.contains(SECRET));
        assert!(!fingerprint.contains("private"));
        assert_ne!(fingerprint, raw);
        let hex = fingerprint.strip_prefix("sha256:").unwrap();
        assert_eq!(hex.len(), 64);
        assert!(
            hex.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
    }
    assert_ne!(mismatch.expected(), mismatch.actual());
}

#[test]
fn raw_resource_bytes_are_compared_even_without_declared_hash() {
    let expected = compilation_with_resource(b"expected");
    let actual = compilation_with_resource(b"actual");

    let comparison = compare_canonical_compilations(&expected, &actual);

    assert!(!comparison.is_equivalent());
    assert!(comparison.mismatches().iter().any(|mismatch| {
        mismatch.domain() == "resource"
            && mismatch.metric() == "resource.raw_byte_hash"
            && mismatch.field() == "resources[asset].rawByteHash"
    }));
    assert!(compare_canonical_compilations(&expected, &expected).is_equivalent());
}

#[test]
fn source_beat_provenance_is_compared_even_when_chart_time_is_equal() {
    let expected = chart_with_notes(vec![tap_note()]);
    let actual = chart_with_notes(vec![tap_note_with_time(
        CanonicalTime::from_chart_time_seconds(0.0).unwrap(),
    )]);
    assert_eq!(
        expected.notes().notes()[0].gameplay().time(),
        actual.notes().notes()[0].gameplay().time()
    );

    let comparison = compare_canonical_charts(&expected, &actual);

    assert!(!comparison.is_equivalent());
    assert!(comparison.mismatches().iter().any(|mismatch| {
        mismatch.domain() == "timing"
            && mismatch.metric() == "timing.source_beat"
            && mismatch.field() == "notes[0].time.sourceBeat"
            && mismatch.error().is_none()
    }));
    assert!(compare_canonical_charts(&expected, &expected).is_equivalent());
}

#[test]
fn note_time_budget_cannot_relax_a_forced_note_boundary() {
    let expected = chart_with_notes(vec![tap_note_with_time(
        CanonicalTime::from_chart_time_seconds(1.0).unwrap(),
    )]);
    let actual = chart_with_notes(vec![tap_note_with_time(
        CanonicalTime::from_chart_time_seconds(1.25).unwrap(),
    )]);
    let metric = "gameplay.note_time";
    let comparison = compare_canonical_charts_with_budgets(
        &expected,
        &actual,
        &BTreeMap::from([(metric.to_owned(), 0.5)]),
        &[],
    );

    assert!(!comparison.is_equivalent());
    assert!(comparison.mismatches().iter().any(|mismatch| {
        mismatch.domain() == "gameplay"
            && mismatch.metric() == metric
            && mismatch.field() == "notes[0].time"
    }));
    assert_eq!(comparison.verified_maximum_error(metric), None);
    assert_eq!(comparison.verified_sample_count(metric), None);
}

#[test]
fn non_finite_budgeted_error_is_a_mismatch_without_verified_metric_evidence() {
    let mut verified = VerifiedMetricObservations::default();
    let mut mismatches = Mismatches::new(&[]);

    compare_float(
        "timing",
        "timing.chart_time",
        "tempo[0].chartTime".into(),
        f64::NAN,
        0.0,
        &BTreeMap::from([("timing.chart_time".to_owned(), 1.0)]),
        &mut verified,
        &mut mismatches,
    );

    assert_eq!(mismatches.into_inner().len(), 1);
    assert_eq!(verified.maximum_errors.get("timing.chart_time"), None);
    assert_eq!(verified.sample_counts.get("timing.chart_time"), None);
}

#[test]
fn world_transform_comparison_catches_parent_identity_hidden_by_document_order() {
    let expected = chart_with_parent("parent-a");
    let actual = chart_with_parent("parent-b");

    let comparison = compare_canonical_charts(&expected, &actual);

    assert!(!comparison.is_equivalent());
    assert!(comparison.mismatches().iter().any(|mismatch| {
        mismatch.metric() == "motion.world_transform" && mismatch.field().contains("worldTransform")
    }));
    assert!(compare_canonical_charts(&expected, &expected).is_equivalent());
}

#[test]
fn missing_line_is_reported_by_stable_identity_without_misaligning_later_lines() {
    let mut registry = StableIdRegistry::new();
    let mut id = |name| {
        registry
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit(name).unwrap(),
            )
            .unwrap()
    };
    let a = id("a");
    let b = id("b");
    let c = id("c");
    let make = |id: StableId, order, x| {
        CanonicalLine::new(
            id,
            None,
            order,
            line_base((x, 0.0)),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap()
    };
    let expected = chart_with_lines(vec![
        make(a.clone(), 0, 0.0),
        make(b.clone(), 1, 1.0),
        make(c.clone(), 2, 2.0),
    ]);
    let actual = chart_with_lines(vec![make(a, 0, 0.0), make(c, 2, 2.0)]);

    let comparison = compare_canonical_charts(&expected, &actual);

    assert!(comparison.mismatches().iter().any(|mismatch| {
        mismatch.field() == format!("lines[{}]", b.value())
            && mismatch.expected() == "present"
            && mismatch.actual() == "missing"
    }));
    assert!(
        !comparison
            .mismatches()
            .iter()
            .any(|mismatch| mismatch.field() == "lines[1].documentOrder")
    );
}

#[test]
fn a_timing_drop_cannot_authorize_losing_tempo_segments() {
    let actual = chart_with_notes(vec![tap_note()]);
    let expected = chart_with_time_map(
        vec![tap_note()],
        ChartTimeMap::new([
            TempoPoint {
                beat: Beat::zero(),
                bpm: 120.0,
            },
            TempoPoint {
                beat: Beat::new(1, 1).unwrap(),
                bpm: 90.0,
            },
        ])
        .unwrap(),
    );
    let comparison = compare_canonical_charts_with_budgets(
        &expected,
        &actual,
        &BTreeMap::new(),
        &["timing.tempo".to_owned()],
    );

    assert!(!comparison.is_equivalent());
    assert_eq!(comparison.mismatches()[0].field(), "tempo.segment_count");
}

#[test]
fn a_filtered_selector_is_unverified_instead_of_equivalent() {
    let comparison = compare_canonical_charts_with_budgets(
        &chart_with_notes(vec![tap_note()]),
        &chart_with_notes(vec![tap_note()]),
        &BTreeMap::new(),
        &["metadata.chart".to_owned()],
    );

    assert!(!comparison.is_equivalent());
    assert_eq!(comparison.unverified_selectors(), ["metadata.chart"]);
}

#[test]
fn an_empty_authorization_keeps_every_domain() {
    let dropped: Vec<String> = Vec::new();
    let mut sink = Mismatches::new(&dropped);
    record(&mut sink, "motion", "lines[0].parent");
    record(&mut sink, "motion", "lines[0].documentOrder");
    assert_eq!(sink.into_inner().len(), 2);
}

#[test]
fn mismatch_sink_bounds_owned_items_at_report_limit() {
    let dropped = Vec::new();
    let mut sink = Mismatches::new(&dropped);
    for index in 0..=MAX_REPORT_ENTRIES {
        sink.push(ComparisonMismatch::new(
            "metadata",
            "discrete",
            format!("field[{index}]"),
            "expected",
            "actual",
            None,
        ));
    }

    let (items, observed) = sink.into_parts();
    assert_eq!(items.len(), MAX_REPORT_ENTRIES);
    assert_eq!(observed, MAX_REPORT_ENTRIES + 1);
    assert_eq!(items[0].field(), "field[0]");
    assert_eq!(items[MAX_REPORT_ENTRIES - 1].field(), "field[1023]");
}
