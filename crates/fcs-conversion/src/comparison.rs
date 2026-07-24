//! Canonical semantic comparison for target reparses.
//!
//! External importers generate format-specific stable IDs, so comparison aligns
//! Lines by canonical document order and Notes by canonical sort order rather
//! than comparing raw IDs or source array positions.

use std::collections::BTreeMap;

use fcs_model::{
    CanonicalChart, CanonicalLine, CanonicalScrollLine, CanonicalTrack, CanonicalTrackPiece,
    CanonicalTrackValue,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonMismatch {
    domain: String,
    metric: String,
    field: String,
    expected: String,
    actual: String,
    error: Option<f64>,
}

impl ComparisonMismatch {
    fn new(
        domain: impl Into<String>,
        metric: impl Into<String>,
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
        error: Option<f64>,
    ) -> Self {
        Self {
            domain: domain.into(),
            metric: metric.into(),
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
            error,
        }
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn metric(&self) -> &str {
        &self.metric
    }

    pub fn field(&self) -> &str {
        &self.field
    }

    pub fn expected(&self) -> &str {
        &self.expected
    }

    pub fn actual(&self) -> &str {
        &self.actual
    }

    pub const fn error(&self) -> Option<f64> {
        self.error
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalComparison {
    mismatches: Vec<ComparisonMismatch>,
}

impl CanonicalComparison {
    pub fn is_equivalent(&self) -> bool {
        self.mismatches.is_empty()
    }

    pub fn mismatches(&self) -> &[ComparisonMismatch] {
        &self.mismatches
    }
}

/// Compare all currently materialized canonical chart fields exactly.
pub fn compare_canonical_charts(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
) -> CanonicalComparison {
    compare_canonical_charts_with_budgets(expected, actual, &BTreeMap::new(), &[])
}

/// Compare canonical fields with explicit metric budgets and explicitly dropped
/// domains. A missing budget remains exact; no implicit epsilon is used.
pub fn compare_canonical_charts_with_budgets(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    dropped_domains: &[String],
) -> CanonicalComparison {
    let mut mismatches = Vec::new();
    let ignored = |domain: &str| {
        dropped_domains.iter().any(|allowed| {
            domain == allowed
                || domain
                    .strip_prefix(allowed)
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    };

    if !ignored("entity") {
        if expected.source_version() != actual.source_version() {
            mismatch(
                &mut mismatches,
                "entity",
                "chart.sourceVersion",
                expected.source_version().to_string(),
                actual.source_version().to_string(),
            );
        }
        if expected.profile() != actual.profile() || expected.features() != actual.features() {
            mismatch(
                &mut mismatches,
                "entity",
                "chart.profile",
                format!("{:?}/{:?}", expected.profile(), expected.features()),
                format!("{:?}/{:?}", actual.profile(), actual.features()),
            );
        }
    }

    if !ignored("timing") {
        compare_time_map(expected, actual, budgets, &mut mismatches);
        compare_sync(expected, actual, budgets, &mut mismatches);
    }
    if !ignored("metadata") {
        compare_metadata(expected, actual, &mut mismatches);
    }
    if !ignored("resource") && expected.metadata().resources() != actual.metadata().resources() {
        mismatch(
            &mut mismatches,
            "resource",
            "metadata.resources",
            format!("{:?}", expected.metadata().resources()),
            format!("{:?}", actual.metadata().resources()),
        );
    }
    if !ignored("motion") {
        compare_lines(expected, actual, budgets, &mut mismatches);
    }
    if !ignored("gameplay") || !ignored("presentation") {
        compare_notes(expected, actual, budgets, &mut mismatches, &ignored);
    }
    if !ignored("motion") {
        compare_tracks(expected, actual, budgets, &mut mismatches);
    }
    if !ignored("scroll") {
        compare_scroll(expected, actual, budgets, &mut mismatches);
    }
    if !ignored("expression") && expected.descriptors() != actual.descriptors() {
        mismatch(
            &mut mismatches,
            "expression",
            "descriptor.structure",
            "equal",
            format!(
                "expected={:?} actual={:?}",
                expected.descriptors(),
                actual.descriptors()
            ),
        );
    }
    if !ignored("entity") && expected.required_extensions() != actual.required_extensions() {
        mismatch(
            &mut mismatches,
            "entity",
            "required_extensions",
            format!("{:?}", expected.required_extensions()),
            format!("{:?}", actual.required_extensions()),
        );
    }

    CanonicalComparison { mismatches }
}

fn compare_time_map(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let left: Vec<_> = expected.time_map().segments().collect();
    let right: Vec<_> = actual.time_map().segments().collect();
    compare_len(
        "timing",
        "tempo.segment_count",
        left.len(),
        right.len(),
        mismatches,
    );
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        if left.0 != right.0 {
            mismatch(
                mismatches,
                "timing",
                format!("tempo[{index}].beat"),
                format!("{:?}", left.0),
                format!("{:?}", right.0),
            );
        }
        compare_float(
            "timing",
            "timing.chart_time",
            format!("tempo[{index}].chartTime"),
            left.1,
            right.1,
            budgets,
            mismatches,
        );
        compare_float(
            "timing",
            "timing.bpm",
            format!("tempo[{index}].bpm"),
            left.2,
            right.2,
            budgets,
            mismatches,
        );
    }
}

fn compare_metadata(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let left = expected.metadata();
    let right = actual.metadata();
    if left.meta() != right.meta() {
        mismatch(
            mismatches,
            "metadata",
            "meta",
            format!("{:?}", left.meta()),
            format!("{:?}", right.meta()),
        );
    }
    if left.contributors() != right.contributors() {
        mismatch(
            mismatches,
            "metadata",
            "contributors",
            format!("{:?}", left.contributors()),
            format!("{:?}", right.contributors()),
        );
    }
    if left.credits() != right.credits() {
        mismatch(
            mismatches,
            "metadata",
            "credits",
            format!("{:?}", left.credits()),
            format!("{:?}", right.credits()),
        );
    }
    if left.artwork() != right.artwork() {
        mismatch(
            mismatches,
            "metadata",
            "artwork",
            format!("{:?}", left.artwork()),
            format!("{:?}", right.artwork()),
        );
    }
}

fn compare_sync(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    match (expected.metadata().sync(), actual.metadata().sync()) {
        (Some(left), Some(right)) => {
            compare_float(
                "timing",
                "timing.audio_offset",
                "sync.audioOffset".into(),
                left.audio_offset().seconds(),
                right.audio_offset().seconds(),
                budgets,
                mismatches,
            );
            if left.primary_audio() != right.primary_audio() || left.preview() != right.preview() {
                mismatch(
                    mismatches,
                    "timing",
                    "sync.discrete",
                    format!("{:?}/{:?}", left.primary_audio(), left.preview()),
                    format!("{:?}/{:?}", right.primary_audio(), right.preview()),
                );
            }
        }
        (None, None) => {}
        (left, right) => mismatch(
            mismatches,
            "timing",
            "sync",
            format!("{left:?}"),
            format!("{right:?}"),
        ),
    }
}

fn compare_lines(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let left = ordered_lines(expected);
    let right = ordered_lines(actual);
    compare_len("motion", "line.count", left.len(), right.len(), mismatches);
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        let field = |name: &str| format!("lines[{index}].{name}");
        if left.document_order() != right.document_order() {
            mismatch(
                mismatches,
                "entity",
                field("documentOrder"),
                left.document_order().to_string(),
                right.document_order().to_string(),
            );
        }
        let left_parent = left
            .parent()
            .and_then(|id| expected.lines().line(id.value()))
            .map(CanonicalLine::document_order);
        let right_parent = right
            .parent()
            .and_then(|id| actual.lines().line(id.value()))
            .map(CanonicalLine::document_order);
        if left_parent != right_parent {
            mismatch(
                mismatches,
                "motion",
                field("parent"),
                format!("{left_parent:?}"),
                format!("{right_parent:?}"),
            );
        }
        if left.inherit() != right.inherit() {
            mismatch(
                mismatches,
                "motion",
                field("inherit"),
                format!("{:?}", left.inherit()),
                format!("{:?}", right.inherit()),
            );
        }
        if left.scroll_tempo() != right.scroll_tempo() {
            mismatch(
                mismatches,
                "scroll",
                field("scrollTempo"),
                format!("{:?}", left.scroll_tempo()),
                format!("{:?}", right.scroll_tempo()),
            );
        }
        let lb = left.base();
        let rb = right.base();
        for (name, lv, rv) in [
            ("position.x", lb.position().x(), rb.position().x()),
            ("position.y", lb.position().y(), rb.position().y()),
            ("rotation", lb.rotation(), rb.rotation()),
            ("scale.x", lb.scale().x(), rb.scale().x()),
            ("scale.y", lb.scale().y(), rb.scale().y()),
            ("alpha", lb.alpha(), rb.alpha()),
            (
                "transformOrigin.x",
                lb.transform_origin().x(),
                rb.transform_origin().x(),
            ),
            (
                "transformOrigin.y",
                lb.transform_origin().y(),
                rb.transform_origin().y(),
            ),
            (
                "textureAnchor.x",
                lb.texture_anchor().x(),
                rb.texture_anchor().x(),
            ),
            (
                "textureAnchor.y",
                lb.texture_anchor().y(),
                rb.texture_anchor().y(),
            ),
            ("floorScale", lb.floor_scale(), rb.floor_scale()),
            (
                "integrationOrigin",
                lb.integration_origin(),
                rb.integration_origin(),
            ),
            (
                "initialFloorPosition",
                lb.initial_floor_position(),
                rb.initial_floor_position(),
            ),
        ] {
            compare_float(
                "motion",
                "motion.value",
                field(name),
                lv,
                rv,
                budgets,
                mismatches,
            );
        }
        if lb.allow_reverse_scroll() != rb.allow_reverse_scroll() || lb.z_order() != rb.z_order() {
            mismatch(
                mismatches,
                "motion",
                field("base.discrete"),
                format!("reverse={} z={}", lb.allow_reverse_scroll(), lb.z_order()),
                format!("reverse={} z={}", rb.allow_reverse_scroll(), rb.z_order()),
            );
        }
    }
}

fn compare_notes(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
    ignored: &impl Fn(&str) -> bool,
) {
    let left = expected.notes().notes();
    let right = actual.notes().notes();
    compare_len(
        "gameplay",
        "note.count",
        left.len(),
        right.len(),
        mismatches,
    );
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        let field = |name: &str| format!("notes[{index}].{name}");
        let lg = left.gameplay();
        let rg = right.gameplay();
        if !ignored("gameplay") {
            if left.document_order() != right.document_order() {
                mismatch(
                    mismatches,
                    "entity",
                    field("documentOrder"),
                    left.document_order().to_string(),
                    right.document_order().to_string(),
                );
            }
            if lg.kind() != rg.kind()
                || lg.side() != rg.side()
                || lg.judgment_enabled() != rg.judgment_enabled()
                || lg.judge_shape() != rg.judge_shape()
                || lg.sound_policy() != rg.sound_policy()
                || lg.score_policy() != rg.score_policy()
            {
                mismatch(
                    mismatches,
                    "gameplay",
                    field("discrete"),
                    format!("{:?}", lg),
                    format!("{:?}", rg),
                );
            }
            let left_line = expected
                .lines()
                .line(lg.line().value())
                .map(CanonicalLine::document_order);
            let right_line = actual
                .lines()
                .line(rg.line().value())
                .map(CanonicalLine::document_order);
            if left_line != right_line {
                mismatch(
                    mismatches,
                    "gameplay",
                    field("line"),
                    format!("{left_line:?}"),
                    format!("{right_line:?}"),
                );
            }
            compare_time(
                "gameplay",
                "timing.note_time",
                field("time"),
                lg.time().chart_time_seconds(),
                rg.time().chart_time_seconds(),
                budgets,
                mismatches,
            );
            compare_optional_time(
                "gameplay",
                "timing.hold_time",
                field("endTime"),
                lg.end_time().map(|time| time.chart_time_seconds()),
                rg.end_time().map(|time| time.chart_time_seconds()),
                budgets,
                mismatches,
            );
        }
        if !ignored("presentation") {
            let lp = left.presentation();
            let rp = right.presentation();
            for (name, lv, rv) in [
                ("positionX", lp.position_x(), rp.position_x()),
                ("scrollFactor", lp.scroll_factor(), rp.scroll_factor()),
                ("xOffset", lp.x_offset(), rp.x_offset()),
                ("yOffset", lp.y_offset(), rp.y_offset()),
                ("alpha", lp.alpha(), rp.alpha()),
                ("scaleX", lp.scale_x(), rp.scale_x()),
                ("scaleY", lp.scale_y(), rp.scale_y()),
                ("rotation", lp.rotation(), rp.rotation()),
            ] {
                compare_float(
                    "presentation",
                    "presentation.value",
                    field(name),
                    lv,
                    rv,
                    budgets,
                    mismatches,
                );
            }
            if lp.color() != rp.color()
                || lp.texture() != rp.texture()
                || lp.render_enabled() != rp.render_enabled()
                || lp.visible_from() != rp.visible_from()
                || lp.visible_until() != rp.visible_until()
            {
                mismatch(
                    mismatches,
                    "presentation",
                    field("discrete"),
                    format!("{:?}", lp),
                    format!("{:?}", rp),
                );
            }
        }
    }
}

fn compare_tracks(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let left = ordered_tracks(expected);
    let right = ordered_tracks(actual);
    compare_len("motion", "track.count", left.len(), right.len(), mismatches);
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        let left_owner = expected
            .lines()
            .line(left.owner().value())
            .map(CanonicalLine::document_order);
        let right_owner = actual
            .lines()
            .line(right.owner().value())
            .map(CanonicalLine::document_order);
        if left_owner != right_owner
            || left.name() != right.name()
            || left.target() != right.target()
            || left.blend() != right.blend()
            || left.priority() != right.priority()
            || left.fill() != right.fill()
            || left.extrapolate_before() != right.extrapolate_before()
            || left.extrapolate_after() != right.extrapolate_after()
        {
            mismatch(
                mismatches,
                "motion",
                format!("tracks[{index}].header"),
                format!("{:?}", left),
                format!("{:?}", right),
            );
        }
        compare_len(
            "motion",
            format!("tracks[{index}].pieceCount"),
            left.pieces().len(),
            right.pieces().len(),
            mismatches,
        );
        for (piece_index, (left, right)) in left.pieces().iter().zip(right.pieces()).enumerate() {
            compare_track_piece(index, piece_index, left, right, budgets, mismatches);
        }
    }
}

fn compare_track_piece(
    track: usize,
    piece: usize,
    left: &CanonicalTrackPiece,
    right: &CanonicalTrackPiece,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let field = |name: &str| format!("tracks[{track}].pieces[{piece}].{name}");
    match (left, right) {
        (CanonicalTrackPiece::Segment(left), CanonicalTrackPiece::Segment(right)) => {
            compare_float(
                "motion",
                "timing.track_time",
                field("start"),
                left.start().chart_time_seconds(),
                right.start().chart_time_seconds(),
                budgets,
                mismatches,
            );
            compare_float(
                "motion",
                "timing.track_time",
                field("end"),
                left.end().chart_time_seconds(),
                right.end().chart_time_seconds(),
                budgets,
                mismatches,
            );
            compare_track_value(
                field("startValue"),
                left.start_value(),
                right.start_value(),
                budgets,
                mismatches,
            );
            compare_track_value(
                field("endValue"),
                left.end_value(),
                right.end_value(),
                budgets,
                mismatches,
            );
            if left.interpolation() != right.interpolation()
                || left.document_order() != right.document_order()
            {
                mismatch(
                    mismatches,
                    "motion",
                    field("shape"),
                    format!("{:?}/{}", left.interpolation(), left.document_order()),
                    format!("{:?}/{}", right.interpolation(), right.document_order()),
                );
            }
        }
        (CanonicalTrackPiece::Point(left), CanonicalTrackPiece::Point(right)) => {
            compare_float(
                "motion",
                "timing.track_time",
                field("time"),
                left.time().chart_time_seconds(),
                right.time().chart_time_seconds(),
                budgets,
                mismatches,
            );
            compare_track_value(
                field("value"),
                left.value(),
                right.value(),
                budgets,
                mismatches,
            );
            if left.document_order() != right.document_order() {
                mismatch(
                    mismatches,
                    "entity",
                    field("documentOrder"),
                    left.document_order().to_string(),
                    right.document_order().to_string(),
                );
            }
        }
        _ => mismatch(
            mismatches,
            "motion",
            field("kind"),
            format!("{:?}", left),
            format!("{:?}", right),
        ),
    }
}

fn compare_track_value(
    field: String,
    left: CanonicalTrackValue,
    right: CanonicalTrackValue,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    match (left, right) {
        (CanonicalTrackValue::Float(left), CanonicalTrackValue::Float(right))
        | (CanonicalTrackValue::Angle(left), CanonicalTrackValue::Angle(right)) => compare_float(
            "motion",
            "motion.track_value",
            field,
            left,
            right,
            budgets,
            mismatches,
        ),
        (CanonicalTrackValue::Vec2Float(left), CanonicalTrackValue::Vec2Float(right))
        | (CanonicalTrackValue::Vec2Length(left), CanonicalTrackValue::Vec2Length(right)) => {
            compare_float(
                "motion",
                "motion.track_value",
                format!("{field}.x"),
                left.x(),
                right.x(),
                budgets,
                mismatches,
            );
            compare_float(
                "motion",
                "motion.track_value",
                format!("{field}.y"),
                left.y(),
                right.y(),
                budgets,
                mismatches,
            );
        }
        _ => mismatch(
            mismatches,
            "motion",
            field,
            format!("{:?}", left),
            format!("{:?}", right),
        ),
    }
}

fn compare_scroll(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let left = ordered_scroll(expected);
    let right = ordered_scroll(actual);
    compare_len(
        "scroll",
        "scroll.line_count",
        left.len(),
        right.len(),
        mismatches,
    );
    for (index, (left, right)) in left.iter().zip(right.iter()).enumerate() {
        if left.allow_reverse_scroll() != right.allow_reverse_scroll() {
            mismatch(
                mismatches,
                "scroll",
                format!("scroll[{index}].allowReverse"),
                left.allow_reverse_scroll().to_string(),
                right.allow_reverse_scroll().to_string(),
            );
        }
        for (name, lv, rv) in [
            ("speed", left.speed(), right.speed()),
            ("floorScale", left.floor_scale(), right.floor_scale()),
            (
                "integrationOrigin",
                left.integration_origin(),
                right.integration_origin(),
            ),
            (
                "initialFloor",
                left.initial_floor_position(),
                right.initial_floor_position(),
            ),
        ] {
            compare_float(
                "scroll",
                "scroll.value",
                format!("scroll[{index}].{name}"),
                lv,
                rv,
                budgets,
                mismatches,
            );
        }
        if left.coordinate().points().len() != right.coordinate().points().len() {
            mismatch(
                mismatches,
                "scroll",
                format!("scroll[{index}].tempo.count"),
                left.coordinate().points().len().to_string(),
                right.coordinate().points().len().to_string(),
            );
        }
        for (point_index, (lp, rp)) in left
            .coordinate()
            .points()
            .iter()
            .zip(right.coordinate().points())
            .enumerate()
        {
            compare_float(
                "scroll",
                "scroll.chart_time",
                format!("scroll[{index}].tempo[{point_index}].chartTime"),
                lp.chart_time(),
                rp.chart_time(),
                budgets,
                mismatches,
            );
            compare_float(
                "scroll",
                "scroll.bpm",
                format!("scroll[{index}].tempo[{point_index}].bpm"),
                lp.bpm(),
                rp.bpm(),
                budgets,
                mismatches,
            );
        }
    }
}

fn ordered_lines(chart: &CanonicalChart) -> Vec<&CanonicalLine> {
    let mut lines: Vec<_> = chart.lines().lines().collect();
    lines.sort_by_key(|line| (line.document_order(), line.id().value()));
    lines
}

fn ordered_scroll(chart: &CanonicalChart) -> Vec<&CanonicalScrollLine> {
    let mut lines: Vec<_> = chart.scroll().lines().iter().collect();
    lines.sort_by_key(|line| {
        chart
            .lines()
            .line(line.line_id().value())
            .map_or((u64::MAX, line.line_id().value()), |line| {
                (line.document_order(), line.id().value())
            })
    });
    lines
}

fn ordered_tracks(chart: &CanonicalChart) -> Vec<&CanonicalTrack> {
    let mut tracks: Vec<_> = chart.tracks().tracks().iter().collect();
    tracks.sort_by(|left, right| {
        let key = |track: &CanonicalTrack| {
            chart
                .lines()
                .line(track.owner().value())
                .map_or(u64::MAX, CanonicalLine::document_order)
        };
        key(left)
            .cmp(&key(right))
            .then_with(|| left.name().cmp(right.name()))
    });
    tracks
}

fn compare_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: f64,
    actual: f64,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    compare_float(domain, metric, field, expected, actual, budgets, mismatches);
}

fn compare_optional_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: Option<f64>,
    actual: Option<f64>,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    match (expected, actual) {
        (Some(expected), Some(actual)) => {
            compare_float(domain, metric, field, expected, actual, budgets, mismatches)
        }
        (None, None) => {}
        (expected, actual) => mismatch(
            mismatches,
            domain,
            field,
            format!("{expected:?}"),
            format!("{actual:?}"),
        ),
    }
}

fn compare_float(
    domain: &str,
    metric: &str,
    field: String,
    expected: f64,
    actual: f64,
    budgets: &BTreeMap<String, f64>,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    let exact = expected.to_bits() == actual.to_bits();
    let error = (expected - actual).abs();
    if exact || budgets.get(metric).is_some_and(|budget| error <= *budget) {
        return;
    }
    mismatches.push(ComparisonMismatch::new(
        domain,
        metric,
        field,
        expected.to_string(),
        actual.to_string(),
        Some(error),
    ));
}

fn compare_len(
    domain: &str,
    field: impl Into<String>,
    expected: usize,
    actual: usize,
    mismatches: &mut Vec<ComparisonMismatch>,
) {
    if expected != actual {
        mismatch(
            mismatches,
            domain,
            field,
            expected.to_string(),
            actual.to_string(),
        );
    }
}

fn mismatch(
    mismatches: &mut Vec<ComparisonMismatch>,
    domain: impl Into<String>,
    field: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
) {
    mismatches.push(ComparisonMismatch::new(
        domain, "discrete", field, expected, actual, None,
    ));
}
