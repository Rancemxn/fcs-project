//! Canonical semantic comparison for target reparses.
//!
//! Public comparison uses canonical stable IDs. Target exporters additionally
//! provide the stable IDs their emitted entities will receive on reparse.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Debug,
};

use fcs_model::{
    Beat, CanonicalChart, CanonicalCompilation, CanonicalContentSha256, CanonicalLine,
    CanonicalResourceBundle, CanonicalScrollLine, CanonicalTime, CanonicalTrack,
    CanonicalTrackPiece, CanonicalTrackTarget, CanonicalTrackValue, DropAuthorization, EntityKind,
    StableId,
};
use fcs_runtime::{evaluate_line_scroll, evaluate_line_transform};
use sha2::{Digest, Sha256};

/// Fixed implementation ceiling for owned mismatch/report entries.
///
/// This safety limit is not configurable and does not change comparison
/// semantics while the observed mismatch count stays within the ceiling.
pub(crate) const MAX_REPORT_ENTRIES: usize = 1024;

#[derive(Debug, Clone, Default)]
pub(crate) struct EntityAlignment {
    lines: BTreeMap<u64, (StableId, StableId)>,
    notes: BTreeMap<u64, (StableId, StableId)>,
}

impl EntityAlignment {
    pub(crate) fn new(
        lines: impl IntoIterator<Item = (StableId, StableId)>,
        notes: impl IntoIterator<Item = (StableId, StableId)>,
    ) -> Option<Self> {
        Some(Self {
            lines: Self::unique_map(EntityKind::Line, lines)?,
            notes: Self::unique_map(EntityKind::Note, notes)?,
        })
    }

    fn unique_map(
        kind: EntityKind,
        pairs: impl IntoIterator<Item = (StableId, StableId)>,
    ) -> Option<BTreeMap<u64, (StableId, StableId)>> {
        let mut mapped = BTreeMap::new();
        let mut targets = BTreeSet::new();
        for (source, target) in pairs {
            if source.namespace() != kind
                || target.namespace() != kind
                || mapped.contains_key(&source.value())
                || !targets.insert(target.value())
            {
                return None;
            }
            mapped.insert(source.value(), (source, target));
        }
        Some(mapped)
    }

    fn line_target<'a>(&'a self, expected: &StableId) -> Option<&'a StableId> {
        Self::target(&self.lines, expected)
    }

    fn note_target<'a>(&'a self, expected: &StableId) -> Option<&'a StableId> {
        Self::target(&self.notes, expected)
    }

    fn target<'a>(
        mappings: &'a BTreeMap<u64, (StableId, StableId)>,
        expected: &StableId,
    ) -> Option<&'a StableId> {
        mappings
            .get(&expected.value())
            .and_then(|(source, target)| (source == expected).then_some(target))
    }
}

fn aligned_line_id<'a>(
    alignment: Option<&'a EntityAlignment>,
    expected: &'a StableId,
) -> Option<&'a StableId> {
    alignment.map_or(Some(expected), |alignment| alignment.line_target(expected))
}

fn aligned_note_id<'a>(
    alignment: Option<&'a EntityAlignment>,
    expected: &'a StableId,
) -> Option<&'a StableId> {
    alignment.map_or(Some(expected), |alignment| alignment.note_target(expected))
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonMismatch {
    domain: String,
    selector: String,
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
        let domain = domain.into();
        let field = field.into();
        Self {
            selector: comparison_selector(&domain, &field),
            domain,
            metric: metric.into(),
            field,
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

    pub fn selector(&self) -> &str {
        &self.selector
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

fn comparison_selector(domain: &str, field: &str) -> String {
    let (entity, property) = match field.split_once('.') {
        Some((entity, property)) => (entity, property),
        None if field.contains('[') => (field, "entity"),
        None => ("chart", field),
    };
    let entity = entity.split('[').next().unwrap_or(entity);
    let entity = match entity {
        "lines" | "line" | "scroll" => "line",
        "notes" | "note" => "note",
        "tracks" | "track" => "track",
        "resources" | "resource" => "resource",
        other => other,
    };
    let property = property.split(['.', '[', '@']).next().unwrap_or(property);
    format!("{domain}.{entity}.{property}")
}

#[derive(Debug, Clone, PartialEq, Default)]
struct VerifiedMetricObservations {
    maximum_errors: BTreeMap<String, f64>,
    sample_counts: BTreeMap<String, u64>,
}

impl VerifiedMetricObservations {
    fn observe(&mut self, metric: &str, error: f64) {
        if !error.is_finite() {
            return;
        }
        self.maximum_errors
            .entry(metric.to_owned())
            .and_modify(|maximum| *maximum = maximum.max(error))
            .or_insert(error);
        self.sample_counts
            .entry(metric.to_owned())
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalComparison {
    mismatches: Vec<ComparisonMismatch>,
    observed_mismatch_count: usize,
    verified_maximum_errors: BTreeMap<String, f64>,
    verified_sample_counts: BTreeMap<String, u64>,
    unverified_selectors: Vec<String>,
}

impl CanonicalComparison {
    pub fn is_equivalent(&self) -> bool {
        self.mismatches.is_empty() && self.unverified_selectors.is_empty()
    }

    pub fn mismatches(&self) -> &[ComparisonMismatch] {
        &self.mismatches
    }

    pub(crate) const fn observed_mismatch_count(&self) -> usize {
        self.observed_mismatch_count
    }

    pub(crate) const fn report_limit_exceeded(&self) -> bool {
        self.observed_mismatch_count > MAX_REPORT_ENTRIES
    }

    /// Maximum absolute error observed for every budgeted metric that was
    /// actually exercised by canonical comparison.
    pub fn verified_maximum_errors(&self) -> &BTreeMap<String, f64> {
        &self.verified_maximum_errors
    }

    pub fn verified_maximum_error(&self, metric: &str) -> Option<f64> {
        self.verified_maximum_errors.get(metric).copied()
    }

    pub fn verified_sample_count(&self, metric: &str) -> Option<u64> {
        self.verified_sample_counts.get(metric).copied()
    }

    pub fn unverified_selectors(&self) -> &[String] {
        &self.unverified_selectors
    }

    /// Compatibility accessor; values are stable selectors, not bare domains.
    pub fn unverified_domains(&self) -> &[String] {
        self.unverified_selectors()
    }
}

/// Compare all currently materialized canonical chart fields exactly.
pub fn compare_canonical_charts(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
) -> CanonicalComparison {
    compare_canonical_charts_with_budgets(expected, actual, &BTreeMap::new(), &[])
}

/// Compare canonical charts and their exact opaque resource payloads.
pub fn compare_canonical_compilations(
    expected: &CanonicalCompilation,
    actual: &CanonicalCompilation,
) -> CanonicalComparison {
    compare_canonical_compilations_with_budgets(expected, actual, &BTreeMap::new(), &[])
}

/// Compare canonical products with explicit metric budgets and dropped selectors.
pub fn compare_canonical_compilations_with_budgets(
    expected: &CanonicalCompilation,
    actual: &CanonicalCompilation,
    budgets: &BTreeMap<String, f64>,
    dropped_selectors: &[String],
) -> CanonicalComparison {
    compare_canonical_charts_with_resources_with_budgets(
        expected.chart(),
        actual.chart(),
        Some(expected.resources()),
        Some(actual.resources()),
        budgets,
        dropped_selectors,
        None,
    )
}

/// Compare canonical fields with explicit metric budgets and explicitly dropped
/// selectors. A missing budget remains exact; no implicit epsilon is used.
pub fn compare_canonical_charts_with_budgets(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    dropped_selectors: &[String],
) -> CanonicalComparison {
    compare_canonical_charts_with_resources_with_budgets(
        expected,
        actual,
        None,
        None,
        budgets,
        dropped_selectors,
        None,
    )
}

pub(crate) fn compare_canonical_charts_with_resources_with_budgets(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    actual_resources: Option<&CanonicalResourceBundle>,
    budgets: &BTreeMap<String, f64>,
    dropped_selectors: &[String],
    alignment: Option<&EntityAlignment>,
) -> CanonicalComparison {
    compare_canonical_charts_with_resources_with_budgets_and_ignored(
        expected,
        actual,
        expected_resources,
        actual_resources,
        budgets,
        ComparisonFilters {
            dropped_selectors,
            ignored_selectors: dropped_selectors,
            ignored_structural_selectors: &[],
        },
        alignment,
    )
}

#[derive(Clone, Copy)]
pub(crate) struct ComparisonFilters<'a> {
    pub(crate) dropped_selectors: &'a [String],
    pub(crate) ignored_selectors: &'a [String],
    pub(crate) ignored_structural_selectors: &'a [String],
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_canonical_charts_with_resources_with_budgets_and_ignored(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    expected_resources: Option<&CanonicalResourceBundle>,
    actual_resources: Option<&CanonicalResourceBundle>,
    budgets: &BTreeMap<String, f64>,
    filters: ComparisonFilters<'_>,
    alignment: Option<&EntityAlignment>,
) -> CanonicalComparison {
    let mut mismatches = Mismatches::with_structural(
        filters.ignored_selectors,
        filters.ignored_structural_selectors,
    );
    let mut verified_maximum_errors = VerifiedMetricObservations::default();

    if expected.source_version() != actual.source_version() {
        mismatch(
            &mut mismatches,
            "profile",
            "chart.sourceVersion",
            expected.source_version().to_string(),
            actual.source_version().to_string(),
        );
    }
    if expected.profile() != actual.profile() || expected.features() != actual.features() {
        mismatch(
            &mut mismatches,
            "profile",
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
            aggregate_fingerprint(expected.metadata().resources()),
            aggregate_fingerprint(actual.metadata().resources()),
        );
    }
    if let (Some(expected), Some(actual)) = (expected_resources, actual_resources) {
        compare_resource_bundles(expected, actual, &mut mismatches);
    }
    // These routines always run; the sink drops only explicitly authorized
    // selectors.
    compare_lines(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
        alignment,
    );
    compare_notes(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
        alignment,
    );
    compare_tracks(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
        alignment,
    );
    compare_scroll(
        expected,
        actual,
        budgets,
        &mut verified_maximum_errors,
        &mut mismatches,
        alignment,
    );
    if expected.descriptors() != actual.descriptors() {
        mismatch(
            &mut mismatches,
            "profile",
            "descriptor.structure",
            aggregate_fingerprint(expected.descriptors()),
            aggregate_fingerprint(actual.descriptors()),
        );
    }
    if expected.required_extensions() != actual.required_extensions() {
        mismatch(
            &mut mismatches,
            "profile",
            "required_extensions",
            aggregate_fingerprint(expected.required_extensions()),
            aggregate_fingerprint(actual.required_extensions()),
        );
    }

    let (mismatch_items, observed_mismatch_count) = mismatches.into_parts();
    let VerifiedMetricObservations {
        maximum_errors,
        sample_counts,
    } = verified_maximum_errors;
    CanonicalComparison {
        mismatches: mismatch_items,
        observed_mismatch_count,
        verified_maximum_errors: maximum_errors,
        verified_sample_counts: sample_counts,
        unverified_selectors: {
            let mut selectors = filters.dropped_selectors.to_vec();
            selectors.sort();
            selectors.dedup();
            selectors
        },
    }
}

fn compare_resource_bundles(
    expected: &CanonicalResourceBundle,
    actual: &CanonicalResourceBundle,
    mismatches: &mut Mismatches<'_>,
) {
    if expected.len() != actual.len() {
        mismatch(
            mismatches,
            "resource",
            "resource.bundle.count",
            expected.len().to_string(),
            actual.len().to_string(),
        );
    }

    let ids: BTreeSet<_> = expected
        .resources()
        .keys()
        .chain(actual.resources().keys())
        .collect();
    for id in ids {
        match (expected.get(id), actual.get(id)) {
            (Some(expected), Some(actual)) => {
                if expected.content_sha256() != actual.content_sha256() {
                    metric_mismatch(
                        mismatches,
                        "resource",
                        "resource.raw_byte_hash",
                        format!("resources[{id}].rawByteHash"),
                        format_content_sha256(expected.content_sha256()),
                        format_content_sha256(actual.content_sha256()),
                    );
                }
            }
            (Some(_), None) => structural_mismatch(
                mismatches,
                "resource",
                format!("resources[{id}]"),
                "present",
                "missing",
            ),
            (None, Some(_)) => structural_mismatch(
                mismatches,
                "resource",
                format!("resources[{id}]"),
                "missing",
                "present",
            ),
            (None, None) => unreachable!("resource ID came from one of the bundles"),
        }
    }
}

fn format_content_sha256(digest: CanonicalContentSha256) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest.as_bytes() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn aggregate_fingerprint(value: impl Debug) -> String {
    let digest = Sha256::digest(format!("{value:?}"));
    let mut output = String::with_capacity(71);
    output.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

/// The single sink every canonical mismatch is recorded through.
///
/// One comparison routine can report several domain/entity/field selectors.
/// Filtering here keeps an authorization from suppressing sibling facts.
struct Mismatches<'a> {
    ignored_selectors: &'a [String],
    ignored_structural_selectors: &'a [String],
    items: Vec<ComparisonMismatch>,
    observed_count: usize,
}

impl<'a> Mismatches<'a> {
    #[cfg(test)]
    fn new(ignored_selectors: &'a [String]) -> Self {
        Self::with_structural(ignored_selectors, &[])
    }

    fn with_structural(
        ignored_selectors: &'a [String],
        ignored_structural_selectors: &'a [String],
    ) -> Self {
        Self {
            ignored_selectors,
            ignored_structural_selectors,
            items: Vec::new(),
            observed_count: 0,
        }
    }

    fn push(&mut self, mismatch: ComparisonMismatch) {
        if self
            .ignored_selectors
            .iter()
            .any(|selector| DropAuthorization::selector_matches(selector, mismatch.selector()))
        {
            return;
        }
        self.record(mismatch);
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
        if self
            .ignored_structural_selectors
            .iter()
            .any(|selector| DropAuthorization::selector_matches(selector, mismatch.selector()))
        {
            return;
        }
        self.record(mismatch);
    }

    fn record(&mut self, mismatch: ComparisonMismatch) {
        self.observed_count = self.observed_count.saturating_add(1);
        if self.items.len() < MAX_REPORT_ENTRIES {
            self.items.push(mismatch);
        }
    }

    fn into_parts(self) -> (Vec<ComparisonMismatch>, usize) {
        (self.items, self.observed_count)
    }

    #[cfg(test)]
    fn into_inner(self) -> Vec<ComparisonMismatch> {
        self.items
    }
}

fn compare_time_map(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut VerifiedMetricObservations,
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
        compare_exact_float(
            "timing",
            "timing.chart_time",
            format!("tempo[{index}].chartTime"),
            left.1,
            right.1,
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
            aggregate_fingerprint(left.meta()),
            aggregate_fingerprint(right.meta()),
        );
    }
    if left.contributors() != right.contributors() {
        mismatch(
            mismatches,
            "metadata",
            "contributors",
            aggregate_fingerprint(left.contributors()),
            aggregate_fingerprint(right.contributors()),
        );
    }
    if left.credits() != right.credits() {
        mismatch(
            mismatches,
            "metadata",
            "credits",
            aggregate_fingerprint(left.credits()),
            aggregate_fingerprint(right.credits()),
        );
    }
    if left.artwork() != right.artwork() {
        mismatch(
            mismatches,
            "metadata",
            "artwork",
            aggregate_fingerprint(left.artwork()),
            aggregate_fingerprint(right.artwork()),
        );
    }
}

fn compare_sync(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut VerifiedMetricObservations,
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
            if left.primary_audio() != right.primary_audio() {
                mismatch(
                    mismatches,
                    "resource",
                    "sync.primaryAudio",
                    aggregate_fingerprint(left.primary_audio()),
                    aggregate_fingerprint(right.primary_audio()),
                );
            }
            if left.preview() != right.preview() {
                mismatch(
                    mismatches,
                    "metadata",
                    "sync.preview",
                    aggregate_fingerprint(left.preview()),
                    aggregate_fingerprint(right.preview()),
                );
            }
        }
        (None, None) => {}
        (left, right) => structural_mismatch(
            mismatches,
            "timing",
            "sync",
            aggregate_fingerprint(left),
            aggregate_fingerprint(right),
        ),
    }
}

fn compare_lines(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut VerifiedMetricObservations,
    mismatches: &mut Mismatches<'_>,
    alignment: Option<&EntityAlignment>,
) {
    let left = ordered_lines(expected);
    let right = ordered_lines(actual);
    let right_by_id: BTreeMap<_, _> = right
        .iter()
        .map(|line| (line.id().value(), *line))
        .collect();
    let mut matched = BTreeSet::new();
    let test_times = line_transform_test_times(expected, actual);
    compare_len("motion", "line.count", left.len(), right.len(), mismatches);
    for (index, left) in left.iter().enumerate() {
        let Some(target_id) = aligned_line_id(alignment, left.id()) else {
            structural_mismatch(
                mismatches,
                "motion",
                format!("lines[{}]", left.id().value()),
                "present",
                "missing",
            );
            continue;
        };
        let Some(right) = right_by_id
            .get(&target_id.value())
            .copied()
            .filter(|line| line.id() == target_id)
        else {
            structural_mismatch(
                mismatches,
                "motion",
                format!("lines[{}]", left.id().value()),
                "present",
                "missing",
            );
            continue;
        };
        matched.insert(right.id().value());
        let field = |name: &str| format!("lines[{index}].{name}");
        if left.document_order() != right.document_order() {
            structural_mismatch(
                mismatches,
                "motion",
                field("documentOrder"),
                left.document_order().to_string(),
                right.document_order().to_string(),
            );
        }
        let left_parent = left.parent().and_then(|id| aligned_line_id(alignment, id));
        let right_parent = right.parent();
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
    for line in right {
        if !matched.contains(&line.id().value()) {
            structural_mismatch(
                mismatches,
                "motion",
                format!("lines[{}]", line.id().value()),
                "missing",
                "present",
            );
        }
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
    mismatches: &mut Mismatches<'_>,
    alignment: Option<&EntityAlignment>,
) {
    let left = expected.notes().notes();
    let right = actual.notes().notes();
    let right_by_id: BTreeMap<_, _> = right.iter().map(|note| (note.id().value(), note)).collect();
    let mut matched = BTreeSet::new();
    compare_len(
        "gameplay",
        "note.count",
        left.len(),
        right.len(),
        mismatches,
    );
    for (index, left) in left.iter().enumerate() {
        let Some(target_id) = aligned_note_id(alignment, left.id()) else {
            structural_mismatch(
                mismatches,
                "gameplay",
                format!("notes[{}]", left.id().value()),
                "present",
                "missing",
            );
            continue;
        };
        let Some(right) = right_by_id
            .get(&target_id.value())
            .copied()
            .filter(|note| note.id() == target_id)
        else {
            structural_mismatch(
                mismatches,
                "gameplay",
                format!("notes[{}]", left.id().value()),
                "present",
                "missing",
            );
            continue;
        };
        matched.insert(right.id().value());
        let field = |name: &str| format!("notes[{index}].{name}");
        let lg = left.gameplay();
        let rg = right.gameplay();
        if left.document_order() != right.document_order() {
            structural_mismatch(
                mismatches,
                "gameplay",
                field("documentOrder"),
                left.document_order().to_string(),
                right.document_order().to_string(),
            );
        }
        compare_discrete("gameplay", field("kind"), lg.kind(), rg.kind(), mismatches);
        compare_discrete("gameplay", field("side"), lg.side(), rg.side(), mismatches);
        compare_discrete(
            "gameplay",
            field("judgmentEnabled"),
            lg.judgment_enabled(),
            rg.judgment_enabled(),
            mismatches,
        );
        compare_discrete(
            "gameplay",
            field("judgeShape"),
            lg.judge_shape(),
            rg.judge_shape(),
            mismatches,
        );
        compare_discrete(
            "gameplay",
            field("soundPolicy"),
            lg.sound_policy(),
            rg.sound_policy(),
            mismatches,
        );
        compare_discrete(
            "gameplay",
            field("scorePolicy"),
            lg.score_policy(),
            rg.score_policy(),
            mismatches,
        );
        let left_line = aligned_line_id(alignment, lg.line());
        let right_line = Some(rg.line());
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
            mismatches,
        );
        compare_optional_time(
            "gameplay",
            "gameplay.hold_time",
            field("endTime"),
            lg.end_time(),
            rg.end_time(),
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
        compare_discrete(
            "presentation",
            field("color"),
            lp.color(),
            rp.color(),
            mismatches,
        );
        compare_discrete(
            "presentation",
            field("texture"),
            lp.texture(),
            rp.texture(),
            mismatches,
        );
        compare_discrete(
            "presentation",
            field("renderEnabled"),
            lp.render_enabled(),
            rp.render_enabled(),
            mismatches,
        );
        compare_discrete(
            "presentation",
            field("visibleFrom"),
            lp.visible_from(),
            rp.visible_from(),
            mismatches,
        );
        compare_discrete(
            "presentation",
            field("visibleUntil"),
            lp.visible_until(),
            rp.visible_until(),
            mismatches,
        );
        compare_optional_source_beat(
            field("visibleFrom"),
            lp.visible_from(),
            rp.visible_from(),
            mismatches,
        );
        compare_optional_source_beat(
            field("visibleUntil"),
            lp.visible_until(),
            rp.visible_until(),
            mismatches,
        );
    }
    for note in right {
        if !matched.contains(&note.id().value()) {
            structural_mismatch(
                mismatches,
                "gameplay",
                format!("notes[{}]", note.id().value()),
                "missing",
                "present",
            );
        }
    }
}

fn compare_tracks(
    expected: &CanonicalChart,
    actual: &CanonicalChart,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut VerifiedMetricObservations,
    mismatches: &mut Mismatches<'_>,
    alignment: Option<&EntityAlignment>,
) {
    let left = ordered_tracks(expected);
    let right = ordered_tracks(actual);
    let right_by_id: BTreeMap<_, _> = right
        .iter()
        .map(|track| ((track.owner().value(), track.name()), *track))
        .collect();
    let mut matched = BTreeSet::new();
    compare_len("motion", "track.count", left.len(), right.len(), mismatches);
    for (index, left) in left.iter().enumerate() {
        let field_key = format!("{}/{}", left.owner().value(), left.name());
        let Some(target_owner) = aligned_line_id(alignment, left.owner()) else {
            structural_mismatch(
                mismatches,
                "motion",
                format!("tracks[{field_key}]"),
                "present",
                "missing",
            );
            continue;
        };
        let Some(right) = right_by_id
            .get(&(target_owner.value(), left.name()))
            .copied()
            .filter(|track| track.owner() == target_owner)
        else {
            structural_mismatch(
                mismatches,
                "motion",
                format!("tracks[{field_key}]"),
                "present",
                "missing",
            );
            continue;
        };
        matched.insert((right.owner().value(), right.name().to_owned()));
        if target_owner != right.owner()
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
                aggregate_fingerprint(left),
                aggregate_fingerprint(right),
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
    for track in right {
        if !matched.contains(&(track.owner().value(), track.name().to_owned())) {
            structural_mismatch(
                mismatches,
                "motion",
                format!("tracks[{}/{}]", track.owner().value(), track.name()),
                "missing",
                "present",
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
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
                mismatches,
            );
            compare_time(
                "motion",
                "motion.track_time",
                field("end"),
                left.end(),
                right.end(),
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
            compare_discrete(
                "motion",
                field("interpolation"),
                left.interpolation(),
                right.interpolation(),
                mismatches,
            );
            if left.document_order() != right.document_order() {
                structural_mismatch(
                    mismatches,
                    "motion",
                    field("documentOrder"),
                    left.document_order().to_string(),
                    right.document_order().to_string(),
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
                structural_mismatch(
                    mismatches,
                    "motion",
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
            aggregate_fingerprint(left),
            aggregate_fingerprint(right),
        ),
    }
}

fn compare_track_value(
    field: String,
    left: CanonicalTrackValue,
    right: CanonicalTrackValue,
    budgets: &BTreeMap<String, f64>,
    verified_maximum_errors: &mut VerifiedMetricObservations,
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
    mismatches: &mut Mismatches<'_>,
    alignment: Option<&EntityAlignment>,
) {
    let left = ordered_scroll(expected);
    let right = ordered_scroll(actual);
    let right_by_id: BTreeMap<_, _> = right
        .iter()
        .map(|line| (line.line_id().value(), *line))
        .collect();
    let mut matched = BTreeSet::new();
    compare_len(
        "scroll",
        "scroll.line_count",
        left.len(),
        right.len(),
        mismatches,
    );
    let test_times = scroll_distance_test_times(expected, actual);
    for (index, left) in left.iter().enumerate() {
        let Some(target_id) = aligned_line_id(alignment, left.line_id()) else {
            structural_mismatch(
                mismatches,
                "scroll",
                format!("scroll[{}]", left.line_id().value()),
                "present",
                "missing",
            );
            continue;
        };
        let Some(right) = right_by_id
            .get(&target_id.value())
            .copied()
            .filter(|line| line.line_id() == target_id)
        else {
            structural_mismatch(
                mismatches,
                "scroll",
                format!("scroll[{}]", left.line_id().value()),
                "present",
                "missing",
            );
            continue;
        };
        matched.insert(right.line_id().value());
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
            compare_exact_float(
                "scroll",
                "scroll.chart_time",
                format!("scroll[{index}].tempo[{point_index}].chartTime"),
                lp.chart_time(),
                rp.chart_time(),
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
    for line in right {
        if !matched.contains(&line.line_id().value()) {
            structural_mismatch(
                mismatches,
                "scroll",
                format!("scroll[{}]", line.line_id().value()),
                "missing",
                "present",
            );
        }
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
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

fn compare_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: CanonicalTime,
    actual: CanonicalTime,
    mismatches: &mut Mismatches<'_>,
) {
    compare_exact_float(
        domain,
        metric,
        field.clone(),
        expected.chart_time_seconds(),
        actual.chart_time_seconds(),
        mismatches,
    );
    compare_source_beat(
        field,
        expected.source_beat(),
        actual.source_beat(),
        mismatches,
    );
}

fn compare_optional_time(
    domain: &str,
    metric: &str,
    field: String,
    expected: Option<CanonicalTime>,
    actual: Option<CanonicalTime>,
    mismatches: &mut Mismatches<'_>,
) {
    match (expected, actual) {
        (Some(expected), Some(actual)) => {
            compare_time(domain, metric, field, expected, actual, mismatches)
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

fn compare_optional_source_beat(
    field: String,
    expected: Option<CanonicalTime>,
    actual: Option<CanonicalTime>,
    mismatches: &mut Mismatches<'_>,
) {
    if let (Some(expected), Some(actual)) = (expected, actual) {
        compare_source_beat(
            field,
            expected.source_beat(),
            actual.source_beat(),
            mismatches,
        );
    }
}

fn compare_exact_float(
    domain: &str,
    metric: &str,
    field: String,
    expected: f64,
    actual: f64,
    mismatches: &mut Mismatches<'_>,
) {
    if expected.to_bits() != actual.to_bits() {
        mismatches.push(ComparisonMismatch::new(
            domain,
            metric,
            field,
            expected.to_string(),
            actual.to_string(),
            Some((expected - actual).abs()),
        ));
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
    verified_maximum_errors: &mut VerifiedMetricObservations,
    mismatches: &mut Mismatches<'_>,
) {
    let exact = expected.to_bits() == actual.to_bits();
    let error = (expected - actual).abs();
    if let Some(budget) = budgets.get(metric) {
        verified_maximum_errors.observe(metric, error);
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

fn structural_mismatch(
    mismatches: &mut Mismatches<'_>,
    domain: impl Into<String>,
    field: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
) {
    mismatches.push_structural(ComparisonMismatch::new(
        domain, "discrete", field, expected, actual, None,
    ));
}

fn compare_discrete<T: std::fmt::Debug + PartialEq>(
    domain: &str,
    field: String,
    expected: T,
    actual: T,
    mismatches: &mut Mismatches<'_>,
) {
    if expected != actual {
        mismatch(
            mismatches,
            domain,
            field,
            format!("{expected:?}"),
            format!("{actual:?}"),
        );
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
        AudioOffset, Beat, CanonicalBundledResource, CanonicalChartScrollTempoPoint,
        CanonicalColor, CanonicalCompilation, CanonicalJudgeShape, CanonicalLineBase,
        CanonicalLineGraph, CanonicalLineInherit, CanonicalMetadata, CanonicalNote,
        CanonicalNoteGameplay, CanonicalNoteKind, CanonicalNotePresentation,
        CanonicalNoteScorePolicy, CanonicalNoteSet, CanonicalNoteSide, CanonicalNoteSoundPolicy,
        CanonicalObject, CanonicalPreview, CanonicalProfile, CanonicalResource,
        CanonicalResourceBundle, CanonicalResourceKind, CanonicalScrollCoordinate,
        CanonicalScrollLine, CanonicalScrollSet, CanonicalScrollTempo, CanonicalSourceVersion,
        CanonicalSync, CanonicalTextualId, CanonicalTime, CanonicalTrackSet, CanonicalValue,
        CanonicalVec2, ChartTimeMap, DistributionMetadata, EntityKind, StableId, StableIdRegistry,
        TempoPoint,
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
            mismatch.metric() == "motion.world_transform"
                && mismatch.field().contains("worldTransform")
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
}
