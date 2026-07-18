//! Immutable canonical Line values and deterministic parent-graph topology.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{Beat, EntityKind, StableId};

/// A finite two-dimensional canonical coordinate/value pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalVec2 {
    x: f64,
    y: f64,
}

impl CanonicalVec2 {
    pub fn new(x: f64, y: f64) -> Result<Self, LineBaseError> {
        if x.is_finite() && y.is_finite() {
            Ok(Self { x, y })
        } else {
            Err(LineBaseError::NonFinite { field: "vec2" })
        }
    }

    pub const fn x(self) -> f64 {
        self.x
    }

    pub const fn y(self) -> f64 {
        self.y
    }
}

/// Static Line base values owned by the canonical model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalLineBase {
    position: CanonicalVec2,
    rotation: f64,
    scale: CanonicalVec2,
    alpha: f64,
    transform_origin: CanonicalVec2,
    texture_anchor: CanonicalVec2,
    floor_scale: f64,
    integration_origin: f64,
    initial_floor_position: f64,
    allow_reverse_scroll: bool,
    z_order: i32,
}

impl CanonicalLineBase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position: CanonicalVec2,
        rotation: f64,
        scale: CanonicalVec2,
        alpha: f64,
        transform_origin: CanonicalVec2,
        texture_anchor: CanonicalVec2,
        floor_scale: f64,
        integration_origin: f64,
        initial_floor_position: f64,
        allow_reverse_scroll: bool,
        z_order: i32,
    ) -> Result<Self, LineBaseError> {
        let base = Self {
            position,
            rotation,
            scale,
            alpha,
            transform_origin,
            texture_anchor,
            floor_scale,
            integration_origin,
            initial_floor_position,
            allow_reverse_scroll,
            z_order,
        };
        base.validate()?;
        Ok(base)
    }

    pub const fn identity() -> Self {
        Self {
            position: CanonicalVec2 { x: 0.0, y: 0.0 },
            rotation: 0.0,
            scale: CanonicalVec2 { x: 1.0, y: 1.0 },
            alpha: 1.0,
            transform_origin: CanonicalVec2 { x: 0.0, y: 0.0 },
            texture_anchor: CanonicalVec2 { x: 0.5, y: 0.5 },
            floor_scale: 120.0,
            integration_origin: 0.0,
            initial_floor_position: 0.0,
            allow_reverse_scroll: false,
            z_order: 0,
        }
    }

    pub const fn position(&self) -> CanonicalVec2 {
        self.position
    }

    pub const fn rotation(&self) -> f64 {
        self.rotation
    }

    pub const fn scale(&self) -> CanonicalVec2 {
        self.scale
    }

    pub const fn alpha(&self) -> f64 {
        self.alpha
    }

    pub const fn transform_origin(&self) -> CanonicalVec2 {
        self.transform_origin
    }

    pub const fn texture_anchor(&self) -> CanonicalVec2 {
        self.texture_anchor
    }

    pub const fn floor_scale(&self) -> f64 {
        self.floor_scale
    }

    pub const fn integration_origin(&self) -> f64 {
        self.integration_origin
    }

    pub const fn initial_floor_position(&self) -> f64 {
        self.initial_floor_position
    }

    pub const fn allow_reverse_scroll(&self) -> bool {
        self.allow_reverse_scroll
    }

    pub const fn z_order(&self) -> i32 {
        self.z_order
    }

    fn validate(&self) -> Result<(), LineBaseError> {
        for (field, value) in [
            ("rotation", self.rotation),
            ("alpha", self.alpha),
            ("floorScale", self.floor_scale),
            ("integrationOrigin", self.integration_origin),
            ("initialFloorPosition", self.initial_floor_position),
        ] {
            if !value.is_finite() {
                return Err(LineBaseError::NonFinite { field });
            }
        }
        if !(0.0..=1.0).contains(&self.alpha) {
            return Err(LineBaseError::OutOfRange { field: "alpha" });
        }
        if !(self.texture_anchor.x >= 0.0
            && self.texture_anchor.x <= 1.0
            && self.texture_anchor.y >= 0.0
            && self.texture_anchor.y <= 1.0)
        {
            return Err(LineBaseError::OutOfRange {
                field: "textureAnchor",
            });
        }
        if self.floor_scale <= 0.0 {
            return Err(LineBaseError::OutOfRange {
                field: "floorScale",
            });
        }
        Ok(())
    }
}

impl Default for CanonicalLineBase {
    fn default() -> Self {
        Self::identity()
    }
}

/// Component-level parent inheritance flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalLineInherit {
    position: bool,
    rotation: bool,
    scale: bool,
    alpha: bool,
    scroll: bool,
}

impl CanonicalLineInherit {
    pub const fn new(
        position: bool,
        rotation: bool,
        scale: bool,
        alpha: bool,
        scroll: bool,
    ) -> Self {
        Self {
            position,
            rotation,
            scale,
            alpha,
            scroll,
        }
    }

    pub const fn position(self) -> bool {
        self.position
    }

    pub const fn rotation(self) -> bool {
        self.rotation
    }

    pub const fn scale(self) -> bool {
        self.scale
    }

    pub const fn alpha(self) -> bool {
        self.alpha
    }

    pub const fn scroll(self) -> bool {
        self.scroll
    }
}

impl Default for CanonicalLineInherit {
    fn default() -> Self {
        Self::new(true, true, true, true, false)
    }
}

/// The key domain of a declared Line scroll-tempo map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollTempoDomain {
    Beat,
    Time,
}

/// A validated source-domain key in a Line scroll-tempo map.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollTempoKey {
    Beat(Beat),
    Time(f64),
}

impl ScrollTempoKey {
    pub const fn domain(self) -> ScrollTempoDomain {
        match self {
            Self::Beat(_) => ScrollTempoDomain::Beat,
            Self::Time(_) => ScrollTempoDomain::Time,
        }
    }

    pub fn is_zero(self) -> bool {
        match self {
            Self::Beat(value) => value == Beat::zero(),
            Self::Time(value) => value == 0.0,
        }
    }

    fn cmp_same_domain(self, other: Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Beat(left), Self::Beat(right)) => Some(left.cmp(&right)),
            (Self::Time(left), Self::Time(right)) => left.partial_cmp(&right),
            _ => None,
        }
    }
}

/// One validated scroll-tempo point. Duplicate keys are allowed and use the
/// final point at that key as the active step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalScrollTempoPoint {
    key: ScrollTempoKey,
    bpm: f64,
}

impl CanonicalScrollTempoPoint {
    pub fn new(key: ScrollTempoKey, bpm: f64) -> Result<Self, ScrollTempoError> {
        if !bpm.is_finite() || bpm <= 0.0 {
            return Err(ScrollTempoError::InvalidBpm);
        }
        if let ScrollTempoKey::Time(value) = key
            && !value.is_finite()
        {
            return Err(ScrollTempoError::NonFiniteKey);
        }
        Ok(Self { key, bpm })
    }

    pub const fn key(self) -> ScrollTempoKey {
        self.key
    }

    pub const fn bpm(self) -> f64 {
        self.bpm
    }
}

/// A validated explicit Line scroll-tempo declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalScrollTempoMap {
    domain: ScrollTempoDomain,
    points: Vec<CanonicalScrollTempoPoint>,
}

impl CanonicalScrollTempoMap {
    pub fn new(
        points: impl IntoIterator<Item = CanonicalScrollTempoPoint>,
    ) -> Result<Self, ScrollTempoError> {
        let points: Vec<_> = points.into_iter().collect();
        let Some(first) = points.first() else {
            return Err(ScrollTempoError::Empty);
        };
        let domain = first.key.domain();
        if !first.key.is_zero() {
            return Err(ScrollTempoError::FirstKeyNotZero);
        }
        let mut previous = first.key;
        for point in &points {
            if point.key.domain() != domain {
                return Err(ScrollTempoError::MixedKeyDomain);
            }
            if let Some(ordering) = point.key.cmp_same_domain(previous)
                && ordering.is_lt()
            {
                return Err(ScrollTempoError::NonMonotonic);
            }
            previous = point.key;
        }
        Ok(Self { domain, points })
    }

    pub const fn domain(&self) -> ScrollTempoDomain {
        self.domain
    }

    pub fn points(&self) -> &[CanonicalScrollTempoPoint] {
        &self.points
    }
}

/// Line scroll configuration before the I3.7 integrator lowers it to runtime
/// descriptors. `Global` refers to the single global chart-time tempo model.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalScrollTempo {
    Global,
    Override(CanonicalScrollTempoMap),
}

/// A canonical Line value. Parent links are IDs; source AST and spans do not
/// cross this boundary.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalLine {
    id: StableId,
    parent: Option<StableId>,
    document_order: u64,
    base: CanonicalLineBase,
    inherit: CanonicalLineInherit,
    scroll_tempo: CanonicalScrollTempo,
}

impl CanonicalLine {
    pub fn new(
        id: StableId,
        parent: Option<StableId>,
        document_order: u64,
        base: CanonicalLineBase,
        inherit: CanonicalLineInherit,
        scroll_tempo: CanonicalScrollTempo,
    ) -> Result<Self, LineGraphError> {
        if id.namespace() != EntityKind::Line {
            return Err(LineGraphError::WrongNamespace { id: id.value() });
        }
        Ok(Self {
            id,
            parent,
            document_order,
            base,
            inherit,
            scroll_tempo,
        })
    }

    pub fn id(&self) -> &StableId {
        &self.id
    }

    pub fn parent(&self) -> Option<&StableId> {
        self.parent.as_ref()
    }

    pub const fn document_order(&self) -> u64 {
        self.document_order
    }

    pub const fn base(&self) -> &CanonicalLineBase {
        &self.base
    }

    pub const fn inherit(&self) -> &CanonicalLineInherit {
        &self.inherit
    }

    pub const fn scroll_tempo(&self) -> &CanonicalScrollTempo {
        &self.scroll_tempo
    }
}

/// Static component state obtained by recursively composing parent Lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalLineWorldState {
    origin: CanonicalVec2,
    rotation: f64,
    scale: CanonicalVec2,
    alpha: f64,
}

impl CanonicalLineWorldState {
    pub const fn origin(self) -> CanonicalVec2 {
        self.origin
    }

    pub const fn rotation(self) -> f64 {
        self.rotation
    }

    pub const fn scale(self) -> CanonicalVec2 {
        self.scale
    }

    pub const fn alpha(self) -> f64 {
        self.alpha
    }
}

/// An immutable Line graph with deterministic map storage and stable-ID topo order.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalLineGraph {
    lines: BTreeMap<u64, CanonicalLine>,
    topological_order: Vec<StableId>,
}

impl CanonicalLineGraph {
    pub fn new(lines: impl IntoIterator<Item = CanonicalLine>) -> Result<Self, LineGraphError> {
        let mut by_id = BTreeMap::new();
        for line in lines {
            let id = line.id.value();
            if by_id.insert(id, line).is_some() {
                return Err(LineGraphError::DuplicateId { id });
            }
        }

        let mut children = BTreeMap::<u64, Vec<u64>>::new();
        let mut indegree = BTreeMap::<u64, usize>::new();
        for id in by_id.keys().copied() {
            indegree.insert(id, 0);
        }
        for line in by_id.values() {
            let id = line.id.value();
            let Some(parent) = line.parent.as_ref() else {
                continue;
            };
            if parent.namespace() != EntityKind::Line || !by_id.contains_key(&parent.value()) {
                return Err(LineGraphError::UnknownParent {
                    line: id,
                    parent: parent.value(),
                });
            }
            if parent.value() == id {
                return Err(LineGraphError::SelfParent { line: id });
            }
            children.entry(parent.value()).or_default().push(id);
            *indegree.get_mut(&id).expect("line was inserted above") += 1;
        }
        for child_ids in children.values_mut() {
            child_ids.sort_unstable();
        }

        let mut ready = BTreeSet::new();
        for (&id, &degree) in &indegree {
            if degree == 0 {
                ready.insert(id);
            }
        }
        let mut topological_order = Vec::with_capacity(by_id.len());
        while let Some(id) = ready.pop_first() {
            topological_order.push(
                by_id
                    .get(&id)
                    .expect("ready ID must be in the line map")
                    .id
                    .clone(),
            );
            if let Some(child_ids) = children.get(&id) {
                for child in child_ids {
                    let degree = indegree.get_mut(child).expect("child was inserted above");
                    *degree -= 1;
                    if *degree == 0 {
                        ready.insert(*child);
                    }
                }
            }
        }
        if topological_order.len() != by_id.len() {
            return Err(LineGraphError::Cycle {
                lines: indegree
                    .into_iter()
                    .filter_map(|(id, degree)| (degree != 0).then_some(id))
                    .collect(),
            });
        }

        Ok(Self {
            lines: by_id,
            topological_order,
        })
    }

    pub fn lines(&self) -> impl Iterator<Item = &CanonicalLine> {
        self.lines.values()
    }

    pub fn line(&self, id: u64) -> Option<&CanonicalLine> {
        self.lines.get(&id)
    }

    pub fn line_by_textual_id(&self, textual_id: &str) -> Option<&CanonicalLine> {
        self.lines
            .values()
            .find(|line| line.id.textual().as_str() == textual_id)
    }

    pub fn topological_order(&self) -> &[StableId] {
        &self.topological_order
    }

    /// Computes the static component state specified by FCS §11.4.
    pub fn world_state(&self, textual_id: &str) -> Option<CanonicalLineWorldState> {
        let target = self.line_by_textual_id(textual_id)?.id.value();
        let mut states = BTreeMap::<u64, CanonicalLineWorldState>::new();
        for id in &self.topological_order {
            let line = self.lines.get(&id.value()).expect("topology ID is in map");
            let (parent_position, parent_rotation, parent_scale, parent_alpha) = line
                .parent
                .as_ref()
                .and_then(|parent| {
                    states
                        .get(&parent.value())
                        .copied()
                        .map(|state| (parent, state))
                })
                .map(|(_, state)| {
                    (
                        if line.inherit.position {
                            state.origin
                        } else {
                            vec2(0.0, 0.0)
                        },
                        if line.inherit.rotation {
                            state.rotation
                        } else {
                            0.0
                        },
                        if line.inherit.scale {
                            state.scale
                        } else {
                            vec2(1.0, 1.0)
                        },
                        if line.inherit.alpha { state.alpha } else { 1.0 },
                    )
                })
                .unwrap_or((vec2(0.0, 0.0), 0.0, vec2(1.0, 1.0), 1.0));

            let local_origin = local_origin(&line.base);
            let origin = apply_parent_transform(
                local_origin,
                parent_position,
                parent_rotation,
                parent_scale,
            );
            let state = CanonicalLineWorldState {
                origin: CanonicalVec2::new(origin.x, origin.y).ok()?,
                rotation: parent_rotation + line.base.rotation,
                scale: vec2(
                    parent_scale.x * line.base.scale.x,
                    parent_scale.y * line.base.scale.y,
                ),
                alpha: parent_alpha * line.base.alpha,
            };
            states.insert(id.value(), state);
            if id.value() == target {
                return Some(state);
            }
        }
        None
    }
}

fn vec2(x: f64, y: f64) -> CanonicalVec2 {
    CanonicalVec2 { x, y }
}

fn local_origin(base: &CanonicalLineBase) -> CanonicalVec2 {
    let scaled_origin = vec2(
        base.transform_origin.x * base.scale.x,
        base.transform_origin.y * base.scale.y,
    );
    let rotated = rotate(scaled_origin, base.rotation);
    vec2(
        base.position.x + base.transform_origin.x - rotated.x,
        base.position.y + base.transform_origin.y - rotated.y,
    )
}

fn apply_parent_transform(
    point: CanonicalVec2,
    position: CanonicalVec2,
    rotation: f64,
    scale: CanonicalVec2,
) -> CanonicalVec2 {
    rotate(vec2(point.x * scale.x, point.y * scale.y), rotation).add(position)
}

fn rotate(point: CanonicalVec2, angle: f64) -> CanonicalVec2 {
    let (sin, cos) = angle.sin_cos();
    vec2(cos * point.x - sin * point.y, sin * point.x + cos * point.y)
}

impl CanonicalVec2 {
    fn add(self, other: Self) -> Self {
        vec2(self.x + other.x, self.y + other.y)
    }
}

/// Invalid static Line base data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBaseError {
    NonFinite { field: &'static str },
    OutOfRange { field: &'static str },
}

/// Invalid explicit scroll-tempo declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollTempoError {
    Empty,
    FirstKeyNotZero,
    MixedKeyDomain,
    NonMonotonic,
    InvalidBpm,
    NonFiniteKey,
}

/// Invalid canonical Line graph structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineGraphError {
    WrongNamespace { id: u64 },
    DuplicateId { id: u64 },
    UnknownParent { line: u64, parent: u64 },
    SelfParent { line: u64 },
    Cycle { lines: Vec<u64> },
}

impl fmt::Display for LineBaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { field } => write!(formatter, "Line field {field} must be finite"),
            Self::OutOfRange { field } => write!(formatter, "Line field {field} is out of range"),
        }
    }
}

impl std::error::Error for LineBaseError {}

impl fmt::Display for ScrollTempoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "scroll tempo map must contain at least one point",
            Self::FirstKeyNotZero => "scroll tempo map must start at its domain zero",
            Self::MixedKeyDomain => "scroll tempo map cannot mix beat and time keys",
            Self::NonMonotonic => "scroll tempo map keys must be non-decreasing",
            Self::InvalidBpm => "scroll tempo BPM must be finite and positive",
            Self::NonFiniteKey => "scroll tempo key must be finite",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ScrollTempoError {}

impl fmt::Display for LineGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongNamespace { id } => write!(formatter, "stable ID {id} is not a Line ID"),
            Self::DuplicateId { id } => write!(formatter, "duplicate canonical Line ID {id}"),
            Self::UnknownParent { line, parent } => {
                write!(formatter, "Line {line} refers to unknown parent {parent}")
            }
            Self::SelfParent { line } => write!(formatter, "Line {line} cannot parent itself"),
            Self::Cycle { lines } => {
                write!(formatter, "Line parent graph contains a cycle: {lines:?}")
            }
        }
    }
}

impl std::error::Error for LineGraphError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalTextualId, StableIdRegistry, derive_stable_id};

    fn id(name: &str) -> StableId {
        let mut registry = StableIdRegistry::new();
        registry
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit(name).unwrap(),
            )
            .unwrap()
    }

    fn line(name: &str, parent: Option<&StableId>) -> CanonicalLine {
        CanonicalLine::new(
            id(name),
            parent.cloned(),
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap()
    }

    #[test]
    fn generated_line_ids_use_the_existing_typed_namespace() {
        let path = crate::ExpansionPath::new("lines", 0).unwrap();
        let textual = CanonicalTextualId::generated(EntityKind::Line, &path, 0);
        let mut registry = StableIdRegistry::new();
        let stable = registry.insert(EntityKind::Line, textual.clone()).unwrap();
        assert_eq!(stable.textual(), &textual);
        assert_eq!(stable.namespace(), EntityKind::Line);
    }

    #[test]
    fn graph_topology_uses_stable_id_tie_breaks_and_rejects_cycles() {
        let root_a = line("a", None);
        let root_b = line("b", None);
        let child = line("child", Some(root_a.id()));
        let graph = CanonicalLineGraph::new([child, root_b, root_a]).unwrap();
        let order = graph
            .topological_order()
            .iter()
            .map(|id| id.textual().as_str())
            .collect::<Vec<_>>();
        let mut roots = vec!["a", "b"];
        roots.sort_by_key(|name| derive_stable_id(EntityKind::Line, name));
        let expected = [roots.as_slice(), &["child"]].concat();
        assert_eq!(order, expected);

        let a = id("a");
        let b = id("b");
        let cycle_a = CanonicalLine::new(
            a.clone(),
            Some(b.clone()),
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap();
        let cycle_b = CanonicalLine::new(
            b,
            Some(a),
            1,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap();
        assert!(matches!(
            CanonicalLineGraph::new([cycle_a, cycle_b]),
            Err(LineGraphError::Cycle { .. })
        ));
    }
}
