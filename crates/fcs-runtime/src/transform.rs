//! Deterministic evaluation of canonical Line transforms.

use std::collections::BTreeMap;
use std::fmt;

use fcs_model::{
    CanonicalLine, CanonicalLineGraph, CanonicalTrackSet, CanonicalTrackTarget,
    CanonicalTrackValue, CanonicalVec2, EntityKind, StableId,
};
use nalgebra::Matrix3;

use crate::{TrackEvaluationError, evaluate_track_set};

/// Project-owned row-major representation of an affine 3x3 column-vector matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineTransformMatrix {
    rows: [[f64; 3]; 3],
}

impl LineTransformMatrix {
    pub const fn rows(self) -> [[f64; 3]; 3] {
        self.rows
    }
}

/// Evaluated Line components before or after parent inheritance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedLineComponents {
    position: CanonicalVec2,
    rotation: f64,
    scale: CanonicalVec2,
    alpha: f64,
}

impl EvaluatedLineComponents {
    pub const fn position(self) -> CanonicalVec2 {
        self.position
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

/// Complete local and world transform result for one canonical Line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvaluatedLineTransform {
    line_id: u64,
    local: EvaluatedLineComponents,
    world: EvaluatedLineComponents,
    local_matrix: LineTransformMatrix,
    world_matrix: LineTransformMatrix,
}

impl EvaluatedLineTransform {
    pub const fn line_id(self) -> u64 {
        self.line_id
    }

    pub const fn local(self) -> EvaluatedLineComponents {
        self.local
    }

    pub const fn world(self) -> EvaluatedLineComponents {
        self.world
    }

    pub const fn local_matrix(self) -> LineTransformMatrix {
        self.local_matrix
    }

    pub const fn world_matrix(self) -> LineTransformMatrix {
        self.world_matrix
    }

    pub fn transform_world_point(
        self,
        point: CanonicalVec2,
    ) -> Result<CanonicalVec2, LineTransformError> {
        apply_public_matrix(self.line_id, self.world_matrix, point, "world point")
    }
}

/// Evaluates one Line's local and inherited world transform at `chart_time`.
pub fn evaluate_line_transform(
    lines: &CanonicalLineGraph,
    tracks: &CanonicalTrackSet,
    line_id: &StableId,
    chart_time: f64,
) -> Result<EvaluatedLineTransform, LineTransformError> {
    if !chart_time.is_finite() {
        return Err(LineTransformError::NonFiniteChartTime);
    }
    if line_id.namespace() != EntityKind::Line {
        return Err(LineTransformError::WrongLineNamespace {
            id: line_id.value(),
        });
    }
    if lines.line(line_id.value()).is_none() {
        return Err(LineTransformError::UnknownLine {
            id: line_id.value(),
        });
    }

    let mut evaluated = BTreeMap::<u64, EvaluatedLineTransform>::new();
    for id in lines.topological_order() {
        let line = lines
            .line(id.value())
            .expect("canonical topology only contains graph Lines");
        let local = evaluate_local_components(line, tracks, chart_time)?;
        let local_matrix = local_matrix(line.id().value(), local, line.base().transform_origin())?;
        let parent = parent_components(line, &evaluated)?;
        let inherited_matrix = inherited_matrix(line.id().value(), parent)?;
        let world_private = multiply(
            line.id().value(),
            &inherited_matrix,
            &private_matrix(local_matrix),
            "world matrix",
        )?;
        let world_position = apply_private_matrix(
            line.id().value(),
            &world_private,
            canonical_vec2(line.id().value(), "world origin", 0.0, 0.0)?,
            "world origin",
        )?;
        let world_rotation = finite_scalar(
            line.id().value(),
            "world rotation",
            parent.rotation + local.rotation,
        )?;
        let world_scale = canonical_vec2(
            line.id().value(),
            "world scale",
            finite_scalar(
                line.id().value(),
                "world scale x",
                parent.scale.x() * local.scale.x(),
            )?,
            finite_scalar(
                line.id().value(),
                "world scale y",
                parent.scale.y() * local.scale.y(),
            )?,
        )?;
        let world_alpha =
            finite_scalar(line.id().value(), "world alpha", parent.alpha * local.alpha)?;
        let result = EvaluatedLineTransform {
            line_id: line.id().value(),
            local,
            world: EvaluatedLineComponents {
                position: world_position,
                rotation: world_rotation,
                scale: world_scale,
                alpha: world_alpha,
            },
            local_matrix,
            world_matrix: public_matrix(line.id().value(), &world_private, "world matrix")?,
        };
        evaluated.insert(line.id().value(), result);
        if line.id().value() == line_id.value() {
            return Ok(result);
        }
    }

    Err(LineTransformError::UnknownLine {
        id: line_id.value(),
    })
}

fn evaluate_local_components(
    line: &CanonicalLine,
    tracks: &CanonicalTrackSet,
    chart_time: f64,
) -> Result<EvaluatedLineComponents, LineTransformError> {
    let line_id = line.id().value();
    let position = match evaluate_component(
        line,
        tracks,
        CanonicalTrackTarget::Position,
        chart_time,
        CanonicalTrackValue::Vec2Length(line.base().position()),
    )? {
        CanonicalTrackValue::Vec2Length(value) => value,
        _ => return Err(LineTransformError::ComponentTypeMismatch { line: line_id }),
    };
    let rotation = match evaluate_component(
        line,
        tracks,
        CanonicalTrackTarget::Rotation,
        chart_time,
        CanonicalTrackValue::Angle(line.base().rotation()),
    )? {
        CanonicalTrackValue::Angle(value) => value,
        _ => return Err(LineTransformError::ComponentTypeMismatch { line: line_id }),
    };
    let scale = match evaluate_component(
        line,
        tracks,
        CanonicalTrackTarget::Scale,
        chart_time,
        CanonicalTrackValue::Vec2Float(line.base().scale()),
    )? {
        CanonicalTrackValue::Vec2Float(value) => value,
        _ => return Err(LineTransformError::ComponentTypeMismatch { line: line_id }),
    };
    let alpha = match evaluate_component(
        line,
        tracks,
        CanonicalTrackTarget::Alpha,
        chart_time,
        CanonicalTrackValue::Float(line.base().alpha()),
    )? {
        CanonicalTrackValue::Float(value) => value,
        _ => return Err(LineTransformError::ComponentTypeMismatch { line: line_id }),
    };
    Ok(EvaluatedLineComponents {
        position,
        rotation,
        scale,
        alpha,
    })
}

fn evaluate_component(
    line: &CanonicalLine,
    tracks: &CanonicalTrackSet,
    target: CanonicalTrackTarget,
    chart_time: f64,
    base: CanonicalTrackValue,
) -> Result<CanonicalTrackValue, LineTransformError> {
    evaluate_track_set(tracks, line.id(), target, chart_time, base).map_err(|source| {
        LineTransformError::Track {
            line: line.id().value(),
            target,
            source,
        }
    })
}

fn parent_components(
    line: &CanonicalLine,
    evaluated: &BTreeMap<u64, EvaluatedLineTransform>,
) -> Result<EvaluatedLineComponents, LineTransformError> {
    let Some(parent_id) = line.parent() else {
        return identity_components(line.id().value());
    };
    let parent =
        evaluated
            .get(&parent_id.value())
            .ok_or(LineTransformError::MissingParentState {
                line: line.id().value(),
                parent: parent_id.value(),
            })?;
    let parent = parent.world;
    Ok(EvaluatedLineComponents {
        position: if line.inherit().position() {
            parent.position
        } else {
            canonical_vec2(line.id().value(), "parent position identity", 0.0, 0.0)?
        },
        rotation: if line.inherit().rotation() {
            parent.rotation
        } else {
            0.0
        },
        scale: if line.inherit().scale() {
            parent.scale
        } else {
            canonical_vec2(line.id().value(), "parent scale identity", 1.0, 1.0)?
        },
        alpha: if line.inherit().alpha() {
            parent.alpha
        } else {
            1.0
        },
    })
}

fn identity_components(line: u64) -> Result<EvaluatedLineComponents, LineTransformError> {
    Ok(EvaluatedLineComponents {
        position: canonical_vec2(line, "position identity", 0.0, 0.0)?,
        rotation: 0.0,
        scale: canonical_vec2(line, "scale identity", 1.0, 1.0)?,
        alpha: 1.0,
    })
}

fn local_matrix(
    line: u64,
    local: EvaluatedLineComponents,
    origin: CanonicalVec2,
) -> Result<LineTransformMatrix, LineTransformError> {
    let translated = multiply(
        line,
        &translation(local.position),
        &translation(origin),
        "local position/origin matrix",
    )?;
    let rotated = multiply(
        line,
        &translated,
        &rotation(line, local.rotation)?,
        "local rotation matrix",
    )?;
    let scaled = multiply(line, &rotated, &scale(local.scale), "local scale matrix")?;
    let matrix = multiply(
        line,
        &scaled,
        &translation(canonical_vec2(
            line,
            "negative transform origin",
            -origin.x(),
            -origin.y(),
        )?),
        "local matrix",
    )?;
    public_matrix(line, &matrix, "local matrix")
}

fn inherited_matrix(
    line: u64,
    parent: EvaluatedLineComponents,
) -> Result<Matrix3<f64>, LineTransformError> {
    let translated = multiply(
        line,
        &translation(parent.position),
        &rotation(line, parent.rotation)?,
        "inherited rotation matrix",
    )?;
    multiply(line, &translated, &scale(parent.scale), "inherited matrix")
}

fn translation(value: CanonicalVec2) -> Matrix3<f64> {
    Matrix3::from_row_slice(&[1.0, 0.0, value.x(), 0.0, 1.0, value.y(), 0.0, 0.0, 1.0])
}

fn rotation(line: u64, angle: f64) -> Result<Matrix3<f64>, LineTransformError> {
    let (sin, cos) = angle.sin_cos();
    let sin = finite_scalar(line, "rotation sine", sin)?;
    let cos = finite_scalar(line, "rotation cosine", cos)?;
    Ok(Matrix3::from_row_slice(&[
        cos, -sin, 0.0, sin, cos, 0.0, 0.0, 0.0, 1.0,
    ]))
}

fn scale(value: CanonicalVec2) -> Matrix3<f64> {
    Matrix3::from_row_slice(&[value.x(), 0.0, 0.0, 0.0, value.y(), 0.0, 0.0, 0.0, 1.0])
}

fn multiply(
    line: u64,
    left: &Matrix3<f64>,
    right: &Matrix3<f64>,
    field: &'static str,
) -> Result<Matrix3<f64>, LineTransformError> {
    let mut values = [0.0; 9];
    for row in 0..3 {
        for column in 0..3 {
            let first = finite_scalar(line, field, left[(row, 0)] * right[(0, column)])?;
            let second = finite_scalar(line, field, left[(row, 1)] * right[(1, column)])?;
            let first_two = finite_scalar(line, field, first + second)?;
            let third = finite_scalar(line, field, left[(row, 2)] * right[(2, column)])?;
            values[(row * 3) + column] = finite_scalar(line, field, first_two + third)?;
        }
    }
    Ok(Matrix3::from_row_slice(&values))
}

fn private_matrix(matrix: LineTransformMatrix) -> Matrix3<f64> {
    Matrix3::from_row_slice(&[
        matrix.rows[0][0],
        matrix.rows[0][1],
        matrix.rows[0][2],
        matrix.rows[1][0],
        matrix.rows[1][1],
        matrix.rows[1][2],
        matrix.rows[2][0],
        matrix.rows[2][1],
        matrix.rows[2][2],
    ])
}

fn public_matrix(
    line: u64,
    matrix: &Matrix3<f64>,
    field: &'static str,
) -> Result<LineTransformMatrix, LineTransformError> {
    let rows = [
        [matrix[(0, 0)], matrix[(0, 1)], matrix[(0, 2)]],
        [matrix[(1, 0)], matrix[(1, 1)], matrix[(1, 2)]],
        [matrix[(2, 0)], matrix[(2, 1)], matrix[(2, 2)]],
    ];
    for value in rows.into_iter().flatten() {
        finite_scalar(line, field, value)?;
    }
    Ok(LineTransformMatrix { rows })
}

fn apply_private_matrix(
    line: u64,
    matrix: &Matrix3<f64>,
    point: CanonicalVec2,
    field: &'static str,
) -> Result<CanonicalVec2, LineTransformError> {
    apply_rows(
        line,
        [
            [matrix[(0, 0)], matrix[(0, 1)], matrix[(0, 2)]],
            [matrix[(1, 0)], matrix[(1, 1)], matrix[(1, 2)]],
            [matrix[(2, 0)], matrix[(2, 1)], matrix[(2, 2)]],
        ],
        point,
        field,
    )
}

fn apply_public_matrix(
    line: u64,
    matrix: LineTransformMatrix,
    point: CanonicalVec2,
    field: &'static str,
) -> Result<CanonicalVec2, LineTransformError> {
    apply_rows(line, matrix.rows, point, field)
}

fn apply_rows(
    line: u64,
    rows: [[f64; 3]; 3],
    point: CanonicalVec2,
    field: &'static str,
) -> Result<CanonicalVec2, LineTransformError> {
    let evaluate_row = |row: [f64; 3]| {
        let first = finite_scalar(line, field, row[0] * point.x())?;
        let second = finite_scalar(line, field, row[1] * point.y())?;
        let first_two = finite_scalar(line, field, first + second)?;
        finite_scalar(line, field, first_two + row[2])
    };
    let x = evaluate_row(rows[0])?;
    let y = evaluate_row(rows[1])?;
    let homogeneous = evaluate_row(rows[2])?;
    if homogeneous != 1.0 {
        return Err(LineTransformError::InvalidHomogeneousResult { line });
    }
    canonical_vec2(line, field, x, y)
}

fn finite_scalar(line: u64, field: &'static str, value: f64) -> Result<f64, LineTransformError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(LineTransformError::NonFiniteResult { line, field })
}

fn canonical_vec2(
    line: u64,
    field: &'static str,
    x: f64,
    y: f64,
) -> Result<CanonicalVec2, LineTransformError> {
    CanonicalVec2::new(x, y).map_err(|_| LineTransformError::NonFiniteResult { line, field })
}

/// Errors raised while evaluating a canonical Line transform.
#[derive(Debug, Clone, PartialEq)]
pub enum LineTransformError {
    NonFiniteChartTime,
    WrongLineNamespace {
        id: u64,
    },
    UnknownLine {
        id: u64,
    },
    MissingParentState {
        line: u64,
        parent: u64,
    },
    Track {
        line: u64,
        target: CanonicalTrackTarget,
        source: TrackEvaluationError,
    },
    ComponentTypeMismatch {
        line: u64,
    },
    NonFiniteResult {
        line: u64,
        field: &'static str,
    },
    InvalidHomogeneousResult {
        line: u64,
    },
}

impl fmt::Display for LineTransformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteChartTime => {
                formatter.write_str("Line transform query time must be finite")
            }
            Self::WrongLineNamespace { id } => {
                write!(formatter, "stable ID {id} is not a Line ID")
            }
            Self::UnknownLine { id } => write!(formatter, "unknown Line stable ID {id}"),
            Self::MissingParentState { line, parent } => {
                write!(
                    formatter,
                    "Line {line} has no evaluated parent state for {parent}"
                )
            }
            Self::Track {
                line,
                target,
                source,
            } => write!(
                formatter,
                "Line {line} {target:?} Track evaluation failed: {source}"
            ),
            Self::ComponentTypeMismatch { line } => {
                write!(
                    formatter,
                    "Line {line} Track result has the wrong component type"
                )
            }
            Self::NonFiniteResult { line, field } => {
                write!(formatter, "Line {line} produced a non-finite {field}")
            }
            Self::InvalidHomogeneousResult { line } => {
                write!(
                    formatter,
                    "Line {line} produced a non-affine homogeneous point"
                )
            }
        }
    }
}

impl std::error::Error for LineTransformError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Track { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use fcs_model::{
        CanonicalLineBase, CanonicalLineInherit, CanonicalScrollTempo, CanonicalTextualId,
        CanonicalTime, CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill,
        CanonicalTrackPiece, CanonicalTrackPoint, StableIdRegistry,
    };

    use super::*;

    fn ids(names: &[(&str, EntityKind)]) -> Vec<StableId> {
        let mut registry = StableIdRegistry::new();
        names
            .iter()
            .map(|(name, kind)| {
                registry
                    .insert(*kind, CanonicalTextualId::explicit(*name).unwrap())
                    .unwrap()
            })
            .collect()
    }

    fn vec2(x: f64, y: f64) -> CanonicalVec2 {
        CanonicalVec2::new(x, y).unwrap()
    }

    fn base(
        position: (f64, f64),
        rotation: f64,
        scale: (f64, f64),
        alpha: f64,
        origin: (f64, f64),
    ) -> CanonicalLineBase {
        CanonicalLineBase::new(
            vec2(position.0, position.1),
            rotation,
            vec2(scale.0, scale.1),
            alpha,
            vec2(origin.0, origin.1),
            vec2(0.5, 0.5),
            120.0,
            0.0,
            0.0,
            false,
            0,
        )
        .unwrap()
    }

    fn line(
        id: StableId,
        parent: Option<StableId>,
        order: u64,
        base: CanonicalLineBase,
        inherit: CanonicalLineInherit,
    ) -> CanonicalLine {
        CanonicalLine::new(
            id,
            parent,
            order,
            base,
            inherit,
            CanonicalScrollTempo::Global,
        )
        .unwrap()
    }

    fn point_track(
        owner: StableId,
        name: &str,
        target: CanonicalTrackTarget,
        value: CanonicalTrackValue,
    ) -> CanonicalTrack {
        CanonicalTrack::new(
            owner,
            name,
            target,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::Base,
            vec![CanonicalTrackPiece::Point(
                CanonicalTrackPoint::new(
                    CanonicalTime::from_chart_time_seconds(0.0).unwrap(),
                    value,
                    0,
                )
                .unwrap(),
            )],
        )
        .unwrap()
    }

    fn empty_tracks() -> CanonicalTrackSet {
        CanonicalTrackSet::new(Vec::new()).unwrap()
    }

    #[test]
    fn pivoted_local_matrix_and_point_application_follow_column_vector_order() {
        let id = ids(&[("main", EntityKind::Line)]).remove(0);
        let graph = CanonicalLineGraph::new([line(
            id.clone(),
            None,
            0,
            base((3.0, 4.0), 0.0, (2.0, 3.0), 0.5, (1.0, 2.0)),
            CanonicalLineInherit::default(),
        )])
        .unwrap();

        let result = evaluate_line_transform(&graph, &empty_tracks(), &id, 0.0).unwrap();

        assert_eq!(
            result.local_matrix().rows(),
            [[2.0, 0.0, 2.0], [0.0, 3.0, 0.0], [0.0, 0.0, 1.0]]
        );
        assert_eq!(result.world_matrix(), result.local_matrix());
        assert_eq!(result.world().position(), vec2(2.0, 0.0));
        assert_eq!(
            result.transform_world_point(vec2(1.0, 2.0)).unwrap(),
            vec2(4.0, 6.0)
        );
        assert_eq!(result.world().alpha(), 0.5);
    }

    #[test]
    fn positive_rotation_and_track_components_have_exact_small_angle_orientation() {
        let id = ids(&[("tracked", EntityKind::Line)]).remove(0);
        let graph = CanonicalLineGraph::new([line(
            id.clone(),
            None,
            0,
            CanonicalLineBase::identity(),
            CanonicalLineInherit::default(),
        )])
        .unwrap();
        let angle = f64::from_bits(1);
        let tracks = CanonicalTrackSet::new(vec![
            point_track(
                id.clone(),
                "position",
                CanonicalTrackTarget::Position,
                CanonicalTrackValue::Vec2Length(vec2(10.0, 20.0)),
            ),
            point_track(
                id.clone(),
                "rotation",
                CanonicalTrackTarget::Rotation,
                CanonicalTrackValue::Angle(angle),
            ),
            point_track(
                id.clone(),
                "scale",
                CanonicalTrackTarget::Scale,
                CanonicalTrackValue::Vec2Float(vec2(2.0, 3.0)),
            ),
            point_track(
                id.clone(),
                "alpha",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackValue::Float(0.25),
            ),
        ])
        .unwrap();

        let result = evaluate_line_transform(&graph, &tracks, &id, 1.0).unwrap();

        assert_eq!(result.local().position(), vec2(10.0, 20.0));
        assert_eq!(result.local().rotation().to_bits(), angle.to_bits());
        assert_eq!(result.local().scale(), vec2(2.0, 3.0));
        assert_eq!(result.local().alpha(), 0.25);
        let rows = result.local_matrix().rows();
        assert_eq!(rows[0][0], 2.0);
        assert_eq!(rows[0][1].to_bits(), (-3.0 * angle).to_bits());
        assert_eq!(rows[1][0].to_bits(), (2.0 * angle).to_bits());
        assert_eq!(rows[1][1], 3.0);
    }

    #[test]
    fn parent_components_inherit_independently_without_matrix_decomposition() {
        let mut ids = ids(&[("parent", EntityKind::Line), ("child", EntityKind::Line)]);
        let parent_id = ids.remove(0);
        let child_id = ids.remove(0);
        let parent = line(
            parent_id.clone(),
            None,
            0,
            base((10.0, 20.0), 0.0, (2.0, 3.0), 0.5, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let child = line(
            child_id.clone(),
            Some(parent_id),
            1,
            base((1.0, 2.0), 0.0, (4.0, 5.0), 0.25, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let graph = CanonicalLineGraph::new([child, parent]).unwrap();

        let result = evaluate_line_transform(&graph, &empty_tracks(), &child_id, 0.0).unwrap();

        assert_eq!(result.world().position(), vec2(12.0, 26.0));
        assert_eq!(result.world().rotation(), 0.0);
        assert_eq!(result.world().scale(), vec2(8.0, 15.0));
        assert_eq!(result.world().alpha(), 0.125);
        assert_eq!(
            result.world_matrix().rows(),
            [[8.0, 0.0, 12.0], [0.0, 15.0, 26.0], [0.0, 0.0, 1.0]]
        );
    }

    #[test]
    fn disabled_inherit_flags_start_from_world_identity() {
        let mut ids = ids(&[("parent", EntityKind::Line), ("child", EntityKind::Line)]);
        let parent_id = ids.remove(0);
        let child_id = ids.remove(0);
        let parent = line(
            parent_id.clone(),
            None,
            0,
            base((10.0, 20.0), f64::from_bits(1), (2.0, 3.0), 0.5, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let child = line(
            child_id.clone(),
            Some(parent_id),
            1,
            base((1.0, 2.0), 0.0, (4.0, 5.0), 0.25, (0.0, 0.0)),
            CanonicalLineInherit::new(false, false, false, false, false),
        );
        let graph = CanonicalLineGraph::new([parent, child]).unwrap();

        let result = evaluate_line_transform(&graph, &empty_tracks(), &child_id, 0.0).unwrap();

        assert_eq!(result.world(), result.local());
        assert_eq!(result.world_matrix(), result.local_matrix());
    }

    #[test]
    fn non_uniform_parent_scale_and_child_rotation_keep_declared_component_state() {
        let mut ids = ids(&[("parent", EntityKind::Line), ("child", EntityKind::Line)]);
        let parent_id = ids.remove(0);
        let child_id = ids.remove(0);
        let angle = f64::from_bits(1);
        let parent = line(
            parent_id.clone(),
            None,
            0,
            base((0.0, 0.0), 0.0, (2.0, 3.0), 1.0, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let child = line(
            child_id.clone(),
            Some(parent_id),
            1,
            base((0.0, 0.0), angle, (1.0, 1.0), 1.0, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let graph = CanonicalLineGraph::new([parent, child]).unwrap();

        let result = evaluate_line_transform(&graph, &empty_tracks(), &child_id, 0.0).unwrap();
        let rows = result.world_matrix().rows();

        assert_eq!(rows[0][0], 2.0);
        assert_eq!(rows[0][1].to_bits(), (-2.0 * angle).to_bits());
        assert_eq!(rows[1][0].to_bits(), (3.0 * angle).to_bits());
        assert_eq!(rows[1][1], 3.0);
        assert_eq!(result.world().rotation().to_bits(), angle.to_bits());
        assert_eq!(result.world().scale(), vec2(2.0, 3.0));
    }

    #[test]
    fn stable_topology_makes_declaration_order_irrelevant() {
        let mut ids = ids(&[("parent", EntityKind::Line), ("child", EntityKind::Line)]);
        let parent_id = ids.remove(0);
        let child_id = ids.remove(0);
        let parent = line(
            parent_id.clone(),
            None,
            9,
            base((2.0, 3.0), 0.0, (4.0, 5.0), 0.5, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let child = line(
            child_id.clone(),
            Some(parent_id),
            1,
            base((7.0, 11.0), 0.0, (1.0, 1.0), 0.25, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let first = CanonicalLineGraph::new([parent.clone(), child.clone()]).unwrap();
        let second = CanonicalLineGraph::new([child, parent]).unwrap();

        assert_eq!(
            evaluate_line_transform(&first, &empty_tracks(), &child_id, 0.0).unwrap(),
            evaluate_line_transform(&second, &empty_tracks(), &child_id, 0.0).unwrap()
        );
    }

    #[test]
    fn query_track_and_non_finite_errors_are_stable() {
        let mut ids = ids(&[
            ("line", EntityKind::Line),
            ("unknown", EntityKind::Line),
            ("note", EntityKind::Note),
            ("child", EntityKind::Line),
        ]);
        let line_id = ids.remove(0);
        let unknown_id = ids.remove(0);
        let note_id = ids.remove(0);
        let child_id = ids.remove(0);
        let line_value = line(
            line_id.clone(),
            None,
            0,
            base((0.0, 0.0), 0.0, (f64::MAX, f64::MAX), 1.0, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let child = line(
            child_id.clone(),
            Some(line_id.clone()),
            1,
            base((0.0, 0.0), 0.0, (2.0, 2.0), 1.0, (0.0, 0.0)),
            CanonicalLineInherit::default(),
        );
        let graph = CanonicalLineGraph::new([line_value, child]).unwrap();

        assert_eq!(
            evaluate_line_transform(&graph, &empty_tracks(), &line_id, f64::NAN),
            Err(LineTransformError::NonFiniteChartTime)
        );
        assert_eq!(
            evaluate_line_transform(&graph, &empty_tracks(), &note_id, 0.0),
            Err(LineTransformError::WrongLineNamespace {
                id: note_id.value()
            })
        );
        assert_eq!(
            evaluate_line_transform(&graph, &empty_tracks(), &unknown_id, 0.0),
            Err(LineTransformError::UnknownLine {
                id: unknown_id.value()
            })
        );
        assert!(matches!(
            evaluate_line_transform(&graph, &empty_tracks(), &child_id, 0.0),
            Err(LineTransformError::NonFiniteResult { .. })
        ));

        let gap_track = CanonicalTrack::new(
            line_id.clone(),
            "position-gap",
            CanonicalTrackTarget::Position,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::Error,
            CanonicalTrackFill::Error,
            CanonicalTrackFill::Error,
            vec![CanonicalTrackPiece::Point(
                CanonicalTrackPoint::new(
                    CanonicalTime::from_chart_time_seconds(1.0).unwrap(),
                    CanonicalTrackValue::Vec2Length(vec2(1.0, 1.0)),
                    0,
                )
                .unwrap(),
            )],
        )
        .unwrap();
        let tracks = CanonicalTrackSet::new(vec![gap_track]).unwrap();
        assert!(matches!(
            evaluate_line_transform(&graph, &tracks, &line_id, 0.0),
            Err(LineTransformError::Track {
                target: CanonicalTrackTarget::Position,
                source: TrackEvaluationError::Gap { .. },
                ..
            })
        ));
    }
}
