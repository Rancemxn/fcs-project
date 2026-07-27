//! Canonical semantic comparison for target reparses.
//!
//! External importers generate format-specific stable IDs, so comparison aligns
//! Lines by canonical document order and Notes by canonical sort order rather
//! than comparing raw IDs or source array positions.

use std::collections::BTreeMap;

use fcs_model::{
    Beat, CanonicalChart, CanonicalLine, CanonicalScrollLine, CanonicalTime, CanonicalTrack,
    CanonicalTrackPiece, CanonicalTrackTarget, CanonicalTrackValue,
};
use fcs_runtime::{evaluate_line_scroll, evaluate_line_transform};

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
    verified_maximum_errors: BTreeMap<String, f64>,
    unverified_domains: Vec<String>,
}

impl CanonicalComparison {
    pub fn is_equivalent(&self) -> bool {
        self.mismatches.is_empty() && self.unverified_domains.is_empty()
    }

    pub fn mismatches(&self) -> &[ComparisonMismatch] {
        &self.mismatches
    }

    /// Maximum absolute error observed for every budgeted metric that was
    /// actually exercised by canonical comparison.
    pub fn verified_maximum_errors(&self) -> &BTreeMap<String, f64> {
        &self.verified_maximum_errors
    }

    pub fn verified_maximum_error(&self, metric: &str) -> Option<f64> {
        self.verified_maximum_errors.get(metric).copied()
    }

    pub fn unverified_domains(&self) -> &[String] {
        &self.unverified_domains
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
    let mut mismatches = Mismatches::new(dropped_domains);
    let mut verified_maximum_errors = BTreeMap::new();

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

    compare_time_map(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    compare_sync(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    compare_metadata(expected, actual, &mut mismatches);
    if expected.metadata().resources() != actual.metadata().resources() {
        mismatch(
            &mut mismatches,
            "resource",
            "metadata.resources",
            format!("{:?}", expected.metadata().resources()),
            format!("{:?}", actual.metadata().resources()),
        );
    }
    // These four routines each report more than one domain, so they always run
    // and the sink drops only the domains that were actually authorized.
    compare_lines(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    compare_notes(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    compare_tracks(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    compare_scroll(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
    );
    if expected.descriptors() != actual.descriptors() {
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
    if expected.required_extensions() != actual.required_extensions() {
        mismatch(
            &mut mismatches,
            "expression",
            "required_extensions",
            format!("{:?}", expected.required_extensions()),
            format!("{:?}", actual.required_extensions()),
        );
    }

    CanonicalComparison {
        mismatches: mismatches.into_inner(),
        verified_maximum_errors,
        unverified_domains: {
            let mut domains = dropped_domains.to_vec();
            domains.sort();
            domains.dedup();
            domains
        },
    }
}

/// The single sink every canonical mismatch is recorded through.
///
/// Drop authorization is per domain, but one comparison routine can report
/// facts belonging to several domains: `compare_lines` is the only checker of
/// `entity` documentOrder and `scroll` scrollTempo as well as `motion` fields.
/// Filtering here, rather than at the call sites, keeps an authorization for
/// one domain from silently suppressing verification of another.
struct Mismatches<'a> {
    dropped: &'a [String],
    items: Vec<ComparisonMismatch>,
}

impl<'a> Mismatches<'a> {
    fn new(dropped: &'a [String]) -> Self {
        Self {
            dropped,
            items: Vec::new(),
        }
    }

    /// A domain is dropped by an exact match or by a dotted-prefix ancestor,
    /// so `motion` covers `motion.transform` but never `motionBlur`.
    fn is_dropped(dropped: &[String], domain: &str) -> bool {
        dropped.iter().any(|allowed| {
            domain == allowed
                || domain
                    .strip_prefix(allowed)
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    }

    fn push(&mut self, mismatch: ComparisonMismatch) {
        if Self::is_dropped(self.dropped, mismatch.domain()) {
            return;
        }
        self.items.push(mismatch);
    }

    /// Record a structural existence fact, which no authorization can drop.
    ///
    /// Section 6.3 scopes a `DropAuthorization` to domain/entity/field, so it
    /// authorizes losing named content *within* entities that still exist. An
    /// entity count is the check that they exist at all, so filtering it would
    /// let the strongest authorization turn the section 14 round-trip oracle
    /// into a no-op for that domain. The mismatch keeps its own domain so the
    /// report still names the domain that lost the entities.
    fn push_structural(&mut self, mismatch: ComparisonMismatch) {
        self.items.push(mismatch);
    }

    fn into_inner(self) -> Vec<ComparisonMismatch> {
        self.items
    }
}

fn compare_time_map(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
            verified_maximum_errors,
            mismatches,
        );
        compare_float(
            "timing",
            "timing.bpm",
            format!("tempo[{index}].bpm"),
            left.2,
            right.2,
            budgets,
            verified_maximum_errors,
            mismatches,
        );
    }
}

fn compare_metadata(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    mismatches: &mut Mismatches<'_>,
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
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
                verified_maximum_errors,
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
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    let left = ordered_lines(expected);
    let right = ordered_lines(actual);
    let test_times = line_transform_test_times(expected, actual);
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
                verified_maximum_errors,
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
        compare_world_transform(
            expected,
            actual,
            left,
            right,
            index,
            &test_times,
            budgets,
            verified_maximum_errors,
            mismatches,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_world_transform(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    expected_line: &CanonicalLine,
    actual_line: &CanonicalLine,
    line_index: usize,
    test_times: &[f64],
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    for &chart_time in test_times {
        let expected_transform = evaluate_line_transform(
            expected.lines(),
            expected.tracks(),
            expected_line.id(),
            chart_time,
        );
        let actual_transform = evaluate_line_transform(
            actual.lines(),
            actual.tracks(),
            actual_line.id(),
            chart_time,
        );
        let field = |name: &str| {
            format!("lines[{line_index}].worldTransform@chartTime={chart_time}.{name}")
        };
        match (expected_transform, actual_transform) {
            (Ok(expected), Ok(actual)) => {
                let expected_world = expected.world();
                let actual_world = actual.world();
                for (name, expected, actual) in [
                    (
                        "position.x",
                        expected_world.position().x(),
                        actual_world.position().x(),
                    ),
                    (
                        "position.y",
                        expected_world.position().y(),
                        actual_world.position().y(),
                    ),
                    (
                        "rotation",
                        expected_world.rotation(),
                        actual_world.rotation(),
                    ),
                    (
                        "scale.x",
                        expected_world.scale().x(),
                        actual_world.scale().x(),
                    ),
                    (
                        "scale.y",
                        expected_world.scale().y(),
                        actual_world.scale().y(),
                    ),
                    ("alpha", expected_world.alpha(), actual_world.alpha()),
                ] {
                    compare_float(
                        "motion",
                        "motion.world_transform",
                        field(name),
                        expected,
                        actual,
                        budgets,
                        verified_maximum_errors,
                        mismatches,
                    );
                }
                let expected_matrix = expected.world_matrix().rows();
                let actual_matrix = actual.world_matrix().rows();
                for (row, (expected_row, actual_row)) in
                    expected_matrix.iter().zip(actual_matrix).enumerate()
                {
                    for (column, (expected, actual)) in
                        expected_row.iter().zip(actual_row).enumerate()
                    {
                        compare_float(
                            "motion",
                            "motion.world_transform",
                            field(&format!("matrix[{row}][{column}]")),
                            *expected,
                            actual,
                            budgets,
                            verified_maximum_errors,
                            mismatches,
                        );
                    }
                }
            }
            (expected, actual) => metric_mismatch(
                mismatches,
                "motion",
                "motion.world_transform",
                field("evaluation"),
                format!("{expected:?}"),
                format!("{actual:?}"),
            ),
        }
    }
}

fn compare_notes(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
            "gameplay.note_time",
            field("time"),
            lg.time(),
            rg.time(),
            budgets,
            verified_maximum_errors,
            mismatches,
        );
        compare_optional_time(
            "gameplay",
            "gameplay.hold_time",
            field("endTime"),
            lg.end_time(),
            rg.end_time(),
            budgets,
            verified_maximum_errors,
            mismatches,
        );
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
                verified_maximum_errors,
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

fn compare_tracks(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
            compare_track_piece(
                index,
                piece_index,
                left,
                right,
                budgets,
                verified_maximum_errors,
                mismatches,
            );
        }
    }
}

fn compare_track_piece(
    track: usize,
    piece: usize,
    left: &CanonicalTrackPiece,
    right: &CanonicalTrackPiece,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    let field = |name: &str| format!("tracks[{track}].pieces[{piece}].{name}");
    match (left, right) {
        (CanonicalTrackPiece::Segment(left), CanonicalTrackPiece::Segment(right)) => {
            compare_time(
                "motion",
                "motion.track_time",
                field("start"),
                left.start(),
                right.start(),
                budgets,
                verified_maximum_errors,
                mismatches,
            );
            compare_time(
                "motion",
                "motion.track_time",
                field("end"),
                left.end(),
                right.end(),
                budgets,
                verified_maximum_errors,
                mismatches,
            );
            compare_track_value(
                field("startValue"),
                left.start_value(),
                right.start_value(),
                budgets,
                verified_maximum_errors,
                mismatches,
            );
            compare_track_value(
                field("endValue"),
                left.end_value(),
                right.end_value(),
                budgets,
                verified_maximum_errors,
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
            compare_time(
                "motion",
                "motion.track_time",
                field("time"),
                left.time(),
                right.time(),
                budgets,
                verified_maximum_errors,
                mismatches,
            );
            compare_track_value(
                field("value"),
                left.value(),
                right.value(),
                budgets,
                verified_maximum_errors,
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
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
            verified_maximum_errors,
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
                verified_maximum_errors,
                mismatches,
            );
            compare_float(
                "motion",
                "motion.track_value",
                format!("{field}.y"),
                left.y(),
                right.y(),
                budgets,
                verified_maximum_errors,
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
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
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
    let test_times = scroll_distance_test_times(expected, actual);
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
                verified_maximum_errors,
                mismatches,
            );
        }
        compare_len(
            "scroll",
            format!("scroll[{index}].tempo.count"),
            left.coordinate().points().len(),
            right.coordinate().points().len(),
            mismatches,
        );
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
                verified_maximum_errors,
                mismatches,
            );
            compare_float(
                "scroll",
                "scroll.bpm",
                format!("scroll[{index}].tempo[{point_index}].bpm"),
                lp.bpm(),
                rp.bpm(),
                budgets,
                verified_maximum_errors,
                mismatches,
            );
        }
        compare_scroll_distance(
            expected,
            actual,
            left,
            right,
            index,
            &test_times,
            budgets,
            verified_maximum_errors,
            mismatches,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_scroll_distance(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    expected_line: &CanonicalScrollLine,
    actual_line: &CanonicalScrollLine,
    line_index: usize,
    test_times: &[f64],
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    for &chart_time in test_times {
        let expected_floor = evaluate_line_scroll(
            expected.lines(),
            expected.scroll(),
            expected.tracks(),
            expected_line.line_id(),
            chart_time,
        )
        .map(|scroll| scroll.effective_floor());
        let actual_floor = evaluate_line_scroll(
            actual.lines(),
            actual.scroll(),
            actual.tracks(),
            actual_line.line_id(),
            chart_time,
        )
        .map(|scroll| scroll.effective_floor());
        let field = format!("scroll[{line_index}].distance@chartTime={chart_time}");
        match (expected_floor, actual_floor) {
            (Ok(expected), Ok(actual)) => compare_float(
                "scroll",
                "scroll.distance",
                field,
                expected,
                actual,
                budgets,
                verified_maximum_errors,
                mismatches,
            ),
            (expected, actual) => mismatch(
                mismatches,
                "scroll",
                field,
                format!("{expected:?}"),
                format!("{actual:?}"),
            ),
        }
    }
}

fn scroll_distance_test_times(expected: &CanonicalChart, actual: &CanonicalChart) -> Vec<f64> {
    let mut times = vec![0.0];
    for chart in [expected, actual] {
        for line in chart.scroll().lines() {
            times.push(line.integration_origin());
            times.extend(
                line.coordinate()
                    .points()
                    .iter()
                    .map(|point| point.chart_time()),
            );
        }
        for track in chart
            .tracks()
            .tracks()
            .iter()
            .filter(|track| track.target() == CanonicalTrackTarget::ScrollSpeed)
        {
            for piece in track.pieces() {
                match piece {
                    CanonicalTrackPiece::Segment(segment) => {
                        times.push(segment.start().chart_time_seconds());
                        times.push(segment.end().chart_time_seconds());
                    }
                    CanonicalTrackPiece::Point(point) => {
                        times.push(point.time().chart_time_seconds());
                    }
                }
            }
        }
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| *left == *right);
    if let Some(last) = times.last().copied() {
        let after_last = last + 1.0;
        if after_last.is_finite() && after_last > last {
            times.push(after_last);
        }
    }
    times
}

fn line_transform_test_times(expected: &CanonicalChart, actual: &CanonicalChart) -> Vec<f64> {
    let mut times = scroll_distance_test_times(expected, actual);
    for chart in [expected, actual] {
        for track in chart.tracks().tracks().iter().filter(|track| {
            matches!(
                track.target(),
                CanonicalTrackTarget::Position
                    | CanonicalTrackTarget::Rotation
                    | CanonicalTrackTarget::Scale
                    | CanonicalTrackTarget::Alpha
            )
        }) {
            for piece in track.pieces() {
                match piece {
                    CanonicalTrackPiece::Segment(segment) => {
                        times.push(segment.start().chart_time_seconds());
                        times.push(segment.end().chart_time_seconds());
                    }
                    CanonicalTrackPiece::Point(point) => {
                        times.push(point.time().chart_time_seconds());
                    }
                }
            }
        }
    }
    times.sort_by(f64::total_cmp);
    times.dedup_by(|left, right| *left == *right);
    if let Some(last) = times.last().copied() {
        let after_last = last + 1.0;
        if after_last.is_finite() && after_last > last {
            times.push(after_last);
        }
    }
    times
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

#[allow(clippy::too_many_arguments)]
fn compare_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: CanonicalTime,
    actual: CanonicalTime,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    compare_float(
        domain,
        metric,
        field.clone(),
        expected.chart_time_seconds(),
        actual.chart_time_seconds(),
        budgets,
        verified_maximum_errors,
        mismatches,
    );
    compare_source_beat(
        field,
        expected.source_beat(),
        actual.source_beat(),
        mismatches,
    );
}

#[allow(clippy::too_many_arguments)]
fn compare_optional_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: Option<CanonicalTime>,
    actual: Option<CanonicalTime>,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    match (expected, actual) {
        (Some(expected), Some(actual)) => compare_time(
            domain,
            metric,
            field,
            expected,
            actual,
            budgets,
            verified_maximum_errors,
            mismatches,
        ),
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

fn compare_source_beat(
    field: String,
    expected: Option<Beat>,
    actual: Option<Beat>,
    mismatches: &mut Mismatches<'_>,
) {
    if expected != actual {
        metric_mismatch(
            mismatches,
            "timing",
            "timing.source_beat",
            format!("{field}.sourceBeat"),
            format!("{expected:?}"),
            format!("{actual:?}"),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_float(
    domain: &str,
    metric: &str,
    field: String,
    expected: f64,
    actual: f64,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut BTreeMap<String, f64>,
    mismatches: &mut Mismatches<'_>,
) {
    let exact = expected.to_bits() == actual.to_bits();
    let error = (expected - actual).abs();
    if let Some(budget) = budgets.get(metric) {
        verified_maximum_errors
            .entry(metric.to_owned())
            .and_modify(|maximum| *maximum = maximum.max(error))
            .or_insert(error);
        if error <= *budget {
            return;
        }
    } else if exact {
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
    mismatches: &mut Mismatches<'_>,
) {
    if expected != actual {
        mismatches.push_structural(ComparisonMismatch::new(
            domain,
            "discrete",
            field,
            expected.to_string(),
            actual.to_string(),
            None,
        ));
    }
}

fn mismatch(
    mismatches: &mut Mismatches<'_>,
    domain: impl Into<String>,
    field: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
) {
    metric_mismatch(mismatches, domain, "discrete", field, expected, actual);
}

fn metric_mismatch(
    mismatches: &mut Mismatches<'_>,
    domain: impl Into<String>,
    metric: impl Into<String>,
    field: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
) {
    mismatches.push(ComparisonMismatch::new(
        domain, metric, field, expected, actual, None,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_model::{
        Beat, CanonicalChartScrollTempoPoint, CanonicalColor, CanonicalJudgeShape,
        CanonicalLineBase, CanonicalLineGraph, CanonicalLineInherit, CanonicalMetadata,
        CanonicalNote, CanonicalNoteGameplay, CanonicalNoteKind, CanonicalNotePresentation,
        CanonicalNoteScorePolicy, CanonicalNoteSet, CanonicalNoteSide, CanonicalNoteSoundPolicy,
        CanonicalProfile, CanonicalScrollCoordinate, CanonicalScrollLine, CanonicalScrollSet,
        CanonicalScrollTempo, CanonicalSourceVersion, CanonicalTextualId, CanonicalTime,
        CanonicalTrackSet, CanonicalVec2, ChartTimeMap, EntityKind, StableId, StableIdRegistry,
        TempoPoint,
    };

    fn record(sink: &mut Mismatches<'_>, domain: &str) {
        sink.push(ComparisonMismatch::new(
            domain, "discrete", "field", "expected", "actual", None,
        ));
    }

    #[test]
    fn a_drop_authorization_only_suppresses_its_own_domain() {
        // compare_lines is the sole checker of entity documentOrder and scroll
        // scrollTempo as well as motion fields, so a motion-only authorization
        // must not take those other domains down with it.
        let dropped = vec!["motion".to_owned()];
        let mut sink = Mismatches::new(&dropped);
        record(&mut sink, "motion");
        record(&mut sink, "entity");
        record(&mut sink, "scroll");
        let kept: Vec<String> = sink
            .into_inner()
            .iter()
            .map(|mismatch| mismatch.domain().to_owned())
            .collect();
        assert_eq!(kept, ["entity", "scroll"]);
    }

    #[test]
    fn a_dropped_domain_covers_its_dotted_descendants_only() {
        let dropped = vec!["motion".to_owned()];
        let mut sink = Mismatches::new(&dropped);
        record(&mut sink, "motion.transform");
        record(&mut sink, "motionBlur");
        let kept = sink.into_inner();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].domain(), "motionBlur");
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

    fn scroll_coordinate(
        points: impl IntoIterator<Item = (f64, f64)>,
    ) -> CanonicalScrollCoordinate {
        CanonicalScrollCoordinate::new(
            points.into_iter().map(|(chart_time, bpm)| {
                CanonicalChartScrollTempoPoint::new(chart_time, bpm).unwrap()
            }),
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

    fn tap_note_with_time(time: CanonicalTime) -> CanonicalNote {
        let (line, note) = ids();
        let gameplay = CanonicalNoteGameplay::new(
            CanonicalNoteKind::Tap,
            line,
            time,
            None,
            CanonicalNoteSide::Above,
            true,
            CanonicalJudgeShape::LineDefault,
            CanonicalNoteSoundPolicy::Default,
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
        CanonicalNote::new(note, CanonicalNoteKind::Tap, 0, gameplay, presentation).unwrap()
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
            &["gameplay".to_owned()],
        );
        assert!(!comparison.is_equivalent());
        let counts: Vec<&str> = comparison
            .mismatches()
            .iter()
            .map(ComparisonMismatch::field)
            .collect();
        assert_eq!(counts, ["note.count"]);
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
    fn world_transform_comparison_catches_parent_identity_hidden_by_document_order() {
        let expected = chart_with_parent("parent-a");
        let actual = chart_with_parent("parent-b");

        let comparison = compare_canonical_charts(&expected, &actual);

        assert!(!comparison.is_equivalent());
        assert!(comparison.mismatches().iter().any(|mismatch| {
            mismatch.metric() == "motion.world_transform"
                && mismatch.field().contains("worldTransform")
        }));
        assert!(compare_canonical_charts(&expected, &expected).is_equivalent());
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
            &["timing".to_owned()],
        );

        assert!(!comparison.is_equivalent());
        assert_eq!(comparison.mismatches()[0].field(), "tempo.segment_count");
    }

    #[test]
    fn a_filtered_domain_is_unverified_instead_of_equivalent() {
        let comparison = compare_canonical_charts_with_budgets(
            &chart_with_notes(vec![tap_note()]),
            &chart_with_notes(vec![tap_note()]),
            &BTreeMap::new(),
            &["metadata".to_owned()],
        );

        assert!(!comparison.is_equivalent());
        assert_eq!(comparison.unverified_domains(), ["metadata"]);
    }

    #[test]
    fn an_empty_authorization_keeps_every_domain() {
        let dropped: Vec<String> = Vec::new();
        let mut sink = Mismatches::new(&dropped);
        record(&mut sink, "motion");
        record(&mut sink, "entity");
        assert_eq!(sink.into_inner().len(), 2);
    }
}
