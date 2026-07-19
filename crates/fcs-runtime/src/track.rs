//! Exact evaluation of immutable canonical Track values.

use std::cmp::Ordering;
use std::fmt;

use fcs_model::{
    CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill, CanonicalTrackInterpolation,
    CanonicalTrackPiece, CanonicalTrackSet, CanonicalTrackTarget, CanonicalTrackValue,
    CanonicalVec2, StableId,
};

use crate::{EasingError, EasingId};

const MAX_BEZIER_REFINEMENTS: usize = 192;

/// Evaluates every canonical Track for one Line property at a finite chart time.
///
/// `base` is the already-resolved static schema value. A Track using `base` fill
/// is inactive at that query rather than contributing a duplicate base value.
pub fn evaluate_track_set(
    tracks: &CanonicalTrackSet,
    owner: &StableId,
    target: CanonicalTrackTarget,
    chart_time: f64,
    base: CanonicalTrackValue,
) -> Result<CanonicalTrackValue, TrackEvaluationError> {
    if !chart_time.is_finite() {
        return Err(TrackEvaluationError::NonFiniteChartTime);
    }
    if !value_matches_target(base, target) {
        return Err(TrackEvaluationError::BaseTypeMismatch);
    }
    if !value_is_finite(base) {
        return Err(TrackEvaluationError::NonFiniteBase);
    }

    let mut selected_replace = None;
    let mut add = Vec::new();
    let mut multiply = Vec::new();

    for track in tracks
        .tracks()
        .iter()
        .filter(|track| track.owner() == owner && track.target() == target)
    {
        let Some(value) = evaluate_track(track, chart_time)? else {
            continue;
        };
        match track.blend() {
            CanonicalTrackBlend::Replace => match selected_replace {
                Some((priority, _)) if priority > track.priority() => {}
                Some((priority, _)) if priority == track.priority() => {
                    return Err(TrackEvaluationError::ReplaceConflict { priority });
                }
                _ => selected_replace = Some((track.priority(), value)),
            },
            CanonicalTrackBlend::Add => add.push((track.priority(), track.name(), value)),
            CanonicalTrackBlend::Multiply => {
                multiply.push((track.priority(), track.name(), value));
            }
        }
    }

    add.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
    multiply.sort_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));

    let mut value = selected_replace.map_or(base, |(_, value)| value);
    for (_, _, operand) in add {
        value = combine(value, operand, BlendOperation::Add)?;
    }
    for (_, _, operand) in multiply {
        value = combine(value, operand, BlendOperation::Multiply)?;
    }
    value_is_finite(value)
        .then_some(value)
        .ok_or(TrackEvaluationError::NonFiniteResult)
}

fn evaluate_track(
    track: &CanonicalTrack,
    chart_time: f64,
) -> Result<Option<CanonicalTrackValue>, TrackEvaluationError> {
    let pieces = track.pieces();
    for (index, piece) in pieces.iter().enumerate() {
        match piece {
            CanonicalTrackPiece::Segment(segment)
                if chart_time >= segment.start().chart_time_seconds()
                    && chart_time < segment.end().chart_time_seconds() =>
            {
                return evaluate_segment(segment, chart_time).map(Some);
            }
            CanonicalTrackPiece::Point(point) => {
                let shadowed = pieces.iter().any(|other| {
                    matches!(
                        other,
                        CanonicalTrackPiece::Segment(segment)
                            if segment.start().chart_time_seconds()
                                == point.time().chart_time_seconds()
                    )
                });
                let next_start = pieces.get(index + 1).map(piece_start);
                if !shadowed
                    && chart_time >= point.time().chart_time_seconds()
                    && next_start.is_none_or(|next| chart_time < next)
                {
                    return Ok(Some(point.value()));
                }
            }
            _ => {}
        }
    }

    let effective = pieces
        .iter()
        .filter(|piece| !point_is_shadowed(piece, pieces))
        .collect::<Vec<_>>();
    let first = effective
        .first()
        .expect("canonical Tracks contain an effective piece");
    let last = effective
        .last()
        .expect("canonical Tracks contain an effective piece");

    let fill = if chart_time < piece_start(first) {
        track.extrapolate_before()
    } else if matches!(last, CanonicalTrackPiece::Segment(segment) if chart_time >= segment.end().chart_time_seconds())
    {
        track.extrapolate_after()
    } else {
        track.fill()
    };
    evaluate_fill(track, chart_time, fill, &effective)
}

fn point_is_shadowed(piece: &CanonicalTrackPiece, pieces: &[CanonicalTrackPiece]) -> bool {
    let CanonicalTrackPiece::Point(point) = piece else {
        return false;
    };
    pieces.iter().any(|other| {
        matches!(
            other,
            CanonicalTrackPiece::Segment(segment)
                if segment.start().chart_time_seconds() == point.time().chart_time_seconds()
        )
    })
}

fn evaluate_fill(
    track: &CanonicalTrack,
    chart_time: f64,
    fill: CanonicalTrackFill,
    pieces: &[&CanonicalTrackPiece],
) -> Result<Option<CanonicalTrackValue>, TrackEvaluationError> {
    match fill {
        CanonicalTrackFill::Base => Ok(None),
        CanonicalTrackFill::Zero => Ok(Some(identity(track.target(), false)?)),
        CanonicalTrackFill::One => Ok(Some(identity(track.target(), true)?)),
        CanonicalTrackFill::HoldBefore => pieces
            .iter()
            .copied()
            .find_map(|piece| match piece {
                CanonicalTrackPiece::Segment(segment)
                    if segment.start().chart_time_seconds() > chart_time =>
                {
                    Some(segment.start_value())
                }
                _ => None,
            })
            .map(Some)
            .ok_or_else(|| TrackEvaluationError::Gap {
                track: track.name().to_owned(),
            }),
        CanonicalTrackFill::HoldAfter => pieces
            .iter()
            .copied()
            .rev()
            .find_map(|piece| match piece {
                CanonicalTrackPiece::Segment(segment)
                    if segment.end().chart_time_seconds() <= chart_time =>
                {
                    Some(segment.end_value())
                }
                _ => None,
            })
            .map(Some)
            .ok_or_else(|| TrackEvaluationError::Gap {
                track: track.name().to_owned(),
            }),
        CanonicalTrackFill::Error => Err(TrackEvaluationError::Gap {
            track: track.name().to_owned(),
        }),
    }
}

fn piece_start(piece: &CanonicalTrackPiece) -> f64 {
    match piece {
        CanonicalTrackPiece::Segment(segment) => segment.start().chart_time_seconds(),
        CanonicalTrackPiece::Point(point) => point.time().chart_time_seconds(),
    }
}

fn evaluate_segment(
    segment: &fcs_model::CanonicalTrackSegment,
    chart_time: f64,
) -> Result<CanonicalTrackValue, TrackEvaluationError> {
    if matches!(segment.interpolation(), CanonicalTrackInterpolation::Step) {
        return Ok(segment.start_value());
    }

    let start = segment.start().chart_time_seconds();
    let end = segment.end().chart_time_seconds();
    let numerator = chart_time - start;
    let denominator = end - start;
    if !numerator.is_finite() || !denominator.is_finite() {
        return Err(TrackEvaluationError::NonFiniteProgress);
    }
    let progress = numerator / denominator;
    if !progress.is_finite() {
        return Err(TrackEvaluationError::NonFiniteProgress);
    }
    let progress = clamp_progress(progress);
    let progress = match segment.interpolation() {
        CanonicalTrackInterpolation::Step => unreachable!(),
        CanonicalTrackInterpolation::Linear => progress,
        CanonicalTrackInterpolation::Easing(name) => {
            let easing = EasingId::ALL
                .into_iter()
                .find(|easing| easing.name() == name)
                .ok_or_else(|| TrackEvaluationError::InvalidEasing { name: name.clone() })?;
            easing
                .evaluate(progress)
                .map_err(TrackEvaluationError::Easing)?
        }
        CanonicalTrackInterpolation::CubicBezier(control) => {
            cubic_bezier_progress(*control, progress)?
        }
    };
    interpolate(segment.start_value(), segment.end_value(), progress)
}

fn interpolate(
    start: CanonicalTrackValue,
    end: CanonicalTrackValue,
    progress: f64,
) -> Result<CanonicalTrackValue, TrackEvaluationError> {
    match (start, end) {
        (CanonicalTrackValue::Float(start), CanonicalTrackValue::Float(end)) => {
            finite_scalar(start + ((end - start) * progress)).map(CanonicalTrackValue::Float)
        }
        (CanonicalTrackValue::Angle(start), CanonicalTrackValue::Angle(end)) => {
            finite_scalar(start + ((end - start) * progress)).map(CanonicalTrackValue::Angle)
        }
        (CanonicalTrackValue::Vec2Float(start), CanonicalTrackValue::Vec2Float(end)) => {
            interpolate_vec(start, end, progress).map(CanonicalTrackValue::Vec2Float)
        }
        (CanonicalTrackValue::Vec2Length(start), CanonicalTrackValue::Vec2Length(end)) => {
            interpolate_vec(start, end, progress).map(CanonicalTrackValue::Vec2Length)
        }
        _ => Err(TrackEvaluationError::ValueTypeMismatch),
    }
}

fn interpolate_vec(
    start: CanonicalVec2,
    end: CanonicalVec2,
    progress: f64,
) -> Result<CanonicalVec2, TrackEvaluationError> {
    let x = finite_scalar(start.x() + ((end.x() - start.x()) * progress))?;
    let y = finite_scalar(start.y() + ((end.y() - start.y()) * progress))?;
    CanonicalVec2::new(x, y).map_err(|_| TrackEvaluationError::NonFiniteResult)
}

#[derive(Clone, Copy)]
enum BlendOperation {
    Add,
    Multiply,
}

fn combine(
    left: CanonicalTrackValue,
    right: CanonicalTrackValue,
    operation: BlendOperation,
) -> Result<CanonicalTrackValue, TrackEvaluationError> {
    let scalar = |left, right| match operation {
        BlendOperation::Add => left + right,
        BlendOperation::Multiply => left * right,
    };
    match (left, right) {
        (CanonicalTrackValue::Float(left), CanonicalTrackValue::Float(right)) => {
            finite_scalar(scalar(left, right)).map(CanonicalTrackValue::Float)
        }
        (CanonicalTrackValue::Angle(left), CanonicalTrackValue::Angle(right)) => {
            finite_scalar(scalar(left, right)).map(CanonicalTrackValue::Angle)
        }
        (CanonicalTrackValue::Vec2Float(left), CanonicalTrackValue::Vec2Float(right)) => {
            combine_vec(left, right, operation).map(CanonicalTrackValue::Vec2Float)
        }
        (CanonicalTrackValue::Vec2Length(left), CanonicalTrackValue::Vec2Length(right)) => {
            combine_vec(left, right, operation).map(CanonicalTrackValue::Vec2Length)
        }
        _ => Err(TrackEvaluationError::ValueTypeMismatch),
    }
}

fn combine_vec(
    left: CanonicalVec2,
    right: CanonicalVec2,
    operation: BlendOperation,
) -> Result<CanonicalVec2, TrackEvaluationError> {
    let (x, y) = match operation {
        BlendOperation::Add => (left.x() + right.x(), left.y() + right.y()),
        BlendOperation::Multiply => (left.x() * right.x(), left.y() * right.y()),
    };
    let x = finite_scalar(x)?;
    let y = finite_scalar(y)?;
    CanonicalVec2::new(x, y).map_err(|_| TrackEvaluationError::NonFiniteResult)
}

fn identity(
    target: CanonicalTrackTarget,
    one: bool,
) -> Result<CanonicalTrackValue, TrackEvaluationError> {
    let value = if one { 1.0 } else { 0.0 };
    match target {
        CanonicalTrackTarget::Position => CanonicalVec2::new(value, value)
            .map(CanonicalTrackValue::Vec2Length)
            .map_err(|_| TrackEvaluationError::NonFiniteResult),
        CanonicalTrackTarget::Rotation => Ok(CanonicalTrackValue::Angle(value)),
        CanonicalTrackTarget::Scale => CanonicalVec2::new(value, value)
            .map(CanonicalTrackValue::Vec2Float)
            .map_err(|_| TrackEvaluationError::NonFiniteResult),
        CanonicalTrackTarget::Alpha => Ok(CanonicalTrackValue::Float(value)),
    }
}

fn finite_scalar(value: f64) -> Result<f64, TrackEvaluationError> {
    value
        .is_finite()
        .then_some(value)
        .ok_or(TrackEvaluationError::NonFiniteResult)
}

#[allow(clippy::manual_clamp)]
fn clamp_progress(progress: f64) -> f64 {
    if progress < 0.0 {
        0.0
    } else if progress > 1.0 {
        1.0
    } else {
        progress
    }
}

fn value_is_finite(value: CanonicalTrackValue) -> bool {
    match value {
        CanonicalTrackValue::Float(value) | CanonicalTrackValue::Angle(value) => value.is_finite(),
        CanonicalTrackValue::Vec2Float(value) | CanonicalTrackValue::Vec2Length(value) => {
            value.x().is_finite() && value.y().is_finite()
        }
    }
}

fn value_matches_target(value: CanonicalTrackValue, target: CanonicalTrackTarget) -> bool {
    matches!(
        (target, value),
        (
            CanonicalTrackTarget::Position,
            CanonicalTrackValue::Vec2Length(_)
        ) | (
            CanonicalTrackTarget::Rotation,
            CanonicalTrackValue::Angle(_)
        ) | (
            CanonicalTrackTarget::Scale,
            CanonicalTrackValue::Vec2Float(_)
        ) | (CanonicalTrackTarget::Alpha, CanonicalTrackValue::Float(_))
    )
}

fn cubic_bezier_progress(
    [x1, y1, x2, y2]: [f64; 4],
    progress: f64,
) -> Result<f64, TrackEvaluationError> {
    if progress == 0.0 || progress == 1.0 {
        return Ok(progress);
    }
    if [x1, y1, x2, y2, progress]
        .into_iter()
        .any(|value| !value.is_finite())
        || !(0.0..=1.0).contains(&x1)
        || !(0.0..=1.0).contains(&x2)
    {
        return Err(TrackEvaluationError::InvalidBezier);
    }
    if x1 == 0.0 && y1 == 0.0 && x2 == 1.0 && y2 == 1.0 {
        return Ok(progress);
    }
    if [x1, y1, x2, y2, progress]
        .into_iter()
        .any(|value| value != 0.0 && value.abs() < f64::MIN_POSITIVE)
        || y1.abs() > f64::MAX / 32.0
        || y2.abs() > f64::MAX / 32.0
    {
        return Err(TrackEvaluationError::BezierEnclosureUnavailable);
    }

    exact_bezier_progress([x1, y1, x2, y2], progress)
        .ok()
        .flatten()
        .ok_or(TrackEvaluationError::BezierEnclosureUnavailable)
}

fn exact_bezier_progress([x1, y1, x2, y2]: [f64; 4], progress: f64) -> Result<Option<f64>, ()> {
    let x_controls = [0.0, x1, x2, 1.0].map(Expansion::from_f64);
    let y_controls = [0.0, y1, y2, 1.0].map(Expansion::from_f64);
    let expected_x = Expansion::from_f64(progress);
    let mut lower = Expansion::from_f64(0.0);
    let mut upper = Expansion::from_f64(1.0);

    for iteration in 0..MAX_BEZIER_REFINEMENTS {
        let midpoint = lower.add(&upper)?.scale(0.5)?;
        match cubic_value(&x_controls, &midpoint)?
            .sub(&expected_x)?
            .sign()
        {
            Ordering::Equal => return cubic_value(&y_controls, &midpoint)?.round().map(Some),
            Ordering::Less => lower = midpoint,
            Ordering::Greater => upper = midpoint,
        }

        if iteration >= 55
            && let Some(value) = certify_bezier_interval(&y_controls, &lower, &upper)?
        {
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn certify_bezier_interval(
    controls: &[Expansion; 4],
    lower: &Expansion,
    upper: &Expansion,
) -> Result<Option<f64>, ()> {
    let Some(direction) = derivative_direction(controls, lower, upper)? else {
        return Ok(None);
    };
    let lower_value = cubic_value(controls, lower)?;
    let upper_value = cubic_value(controls, upper)?;
    let (minimum, maximum) = if direction == Ordering::Less {
        (upper_value, lower_value)
    } else {
        (lower_value, upper_value)
    };
    let minimum = minimum.round()?;
    let maximum = maximum.round()?;
    Ok((minimum.to_bits() == maximum.to_bits()).then_some(minimum))
}

fn derivative_direction(
    controls: &[Expansion; 4],
    lower: &Expansion,
    upper: &Expansion,
) -> Result<Option<Ordering>, ()> {
    let y1 = &controls[1];
    let y2 = &controls[2];
    let a = y1
        .scale(9.0)?
        .sub(&y2.scale(9.0)?)?
        .add(&Expansion::from_f64(3.0))?;
    let b = y2.scale(6.0)?.sub(&y1.scale(12.0)?)?;
    let c = y1.scale(3.0)?;
    let mut signs = vec![
        quadratic_value(&a, &b, &c, lower)?.sign(),
        quadratic_value(&a, &b, &c, upper)?.sign(),
    ];

    if a.sign() != Ordering::Equal {
        let vertex_numerator = b.neg();
        let lower_scaled = a.scale(2.0)?.mul(lower)?;
        let upper_scaled = a.scale(2.0)?.mul(upper)?;
        let (minimum, maximum) = if a.sign() == Ordering::Greater {
            (lower_scaled, upper_scaled)
        } else {
            (upper_scaled, lower_scaled)
        };
        if vertex_numerator.sub(&minimum)?.sign() != Ordering::Less
            && vertex_numerator.sub(&maximum)?.sign() != Ordering::Greater
        {
            let numerator = a.mul(&c)?.scale(4.0)?.sub(&b.mul(&b)?)?;
            let vertex_sign = match (numerator.sign(), a.sign()) {
                (Ordering::Equal, _) => Ordering::Equal,
                (left, right) if left == right => Ordering::Greater,
                _ => Ordering::Less,
            };
            signs.push(vertex_sign);
        }
    }

    if signs.iter().all(|sign| *sign != Ordering::Less) {
        Ok(Some(Ordering::Greater))
    } else if signs.iter().all(|sign| *sign != Ordering::Greater) {
        Ok(Some(Ordering::Less))
    } else {
        Ok(None)
    }
}

fn quadratic_value(
    a: &Expansion,
    b: &Expansion,
    c: &Expansion,
    parameter: &Expansion,
) -> Result<Expansion, ()> {
    a.mul(parameter)?.add(b)?.mul(parameter)?.add(c)
}

fn cubic_value(controls: &[Expansion; 4], parameter: &Expansion) -> Result<Expansion, ()> {
    let first = [
        lerp(&controls[0], &controls[1], parameter)?,
        lerp(&controls[1], &controls[2], parameter)?,
        lerp(&controls[2], &controls[3], parameter)?,
    ];
    let second = [
        lerp(&first[0], &first[1], parameter)?,
        lerp(&first[1], &first[2], parameter)?,
    ];
    lerp(&second[0], &second[1], parameter)
}

fn lerp(left: &Expansion, right: &Expansion, parameter: &Expansion) -> Result<Expansion, ()> {
    let complement = Expansion::from_f64(1.0).sub(parameter)?;
    left.mul(&complement)?.add(&right.mul(parameter)?)
}

#[derive(Clone, Debug)]
struct Expansion(Vec<f64>);

impl Expansion {
    fn from_f64(value: f64) -> Self {
        if value == 0.0 {
            Self(Vec::new())
        } else {
            Self(vec![value])
        }
    }

    fn sign(&self) -> Ordering {
        self.0
            .last()
            .map_or(Ordering::Equal, |value| value.total_cmp(&0.0))
    }

    fn neg(&self) -> Self {
        Self(self.0.iter().map(|value| -*value).collect())
    }

    fn sub(&self, other: &Self) -> Result<Self, ()> {
        self.add(&other.neg())
    }

    fn add(&self, other: &Self) -> Result<Self, ()> {
        let mut result = self.clone();
        for component in &other.0 {
            result = result.grow(*component)?;
        }
        result.compress()
    }

    fn grow(&self, value: f64) -> Result<Self, ()> {
        let mut output = Vec::with_capacity(self.0.len() + 1);
        let mut accumulator = value;
        for component in &self.0 {
            let (sum, error) = two_sum(accumulator, *component)?;
            if error != 0.0 {
                output.push(error);
            }
            accumulator = sum;
        }
        if accumulator != 0.0 || output.is_empty() {
            output.push(accumulator);
        }
        Ok(Self(output))
    }

    fn scale(&self, scalar: f64) -> Result<Self, ()> {
        if self.0.is_empty() || scalar == 0.0 {
            return Ok(Self(Vec::new()));
        }
        let mut result = Self(Vec::new());
        for component in &self.0 {
            let (product, error) = two_product(*component, scalar)?;
            let term = Self(vec![error, product]).compress()?;
            result = result.add(&term)?;
        }
        result.compress()
    }

    fn mul(&self, other: &Self) -> Result<Self, ()> {
        if self.0.is_empty() || other.0.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let mut result = Self(Vec::new());
        for component in &other.0 {
            result = result.add(&self.scale(*component)?)?;
        }
        result.compress()
    }

    fn compress(&self) -> Result<Self, ()> {
        if self.0.is_empty() {
            return Ok(Self(Vec::new()));
        }
        let mut temporary = vec![0.0; self.0.len()];
        let mut accumulator = *self.0.last().ok_or(())?;
        let mut bottom = self.0.len() - 1;
        for component in self.0[..self.0.len() - 1].iter().rev() {
            let (sum, error) = fast_two_sum(accumulator, *component)?;
            if error != 0.0 {
                temporary[bottom] = sum;
                bottom -= 1;
                accumulator = error;
            } else {
                accumulator = sum;
            }
        }

        let mut output = Vec::with_capacity(self.0.len());
        for component in &temporary[bottom + 1..] {
            let (sum, error) = fast_two_sum(*component, accumulator)?;
            if error != 0.0 {
                output.push(error);
            }
            accumulator = sum;
        }
        if accumulator != 0.0 || output.is_empty() {
            output.push(accumulator);
        }
        Ok(Self(output))
    }

    fn compare_f64(&self, value: f64) -> Result<Ordering, ()> {
        Ok(self.sub(&Self::from_f64(value))?.sign())
    }

    fn round(&self) -> Result<f64, ()> {
        match self.sign() {
            Ordering::Equal => Ok(0.0),
            Ordering::Less => self.round_with_bounds(-f64::MAX, -0.0),
            Ordering::Greater => self.round_with_bounds(0.0, f64::MAX),
        }
    }

    fn round_with_bounds(&self, minimum: f64, maximum: f64) -> Result<f64, ()> {
        let mut lower_key = ordered_key(minimum);
        let mut upper_key = ordered_key(maximum);
        while lower_key + 1 < upper_key {
            let midpoint_key = lower_key + ((upper_key - lower_key) / 2);
            let midpoint = f64_from_ordered_key(midpoint_key);
            if self.compare_f64(midpoint)? == Ordering::Less {
                upper_key = midpoint_key;
            } else {
                lower_key = midpoint_key;
            }
        }

        let lower = f64_from_ordered_key(lower_key);
        let upper = f64_from_ordered_key(upper_key);
        if self.compare_f64(lower)? == Ordering::Equal {
            return Ok(lower);
        }
        let midpoint = Self::from_f64(lower)
            .add(&Self::from_f64(upper))?
            .scale(0.5)?;
        match self.sub(&midpoint)?.sign() {
            Ordering::Less => Ok(lower),
            Ordering::Greater => Ok(upper),
            Ordering::Equal if lower.to_bits() & 1 == 0 => Ok(lower),
            Ordering::Equal => Ok(upper),
        }
    }
}

fn two_sum(left: f64, right: f64) -> Result<(f64, f64), ()> {
    let sum = left + right;
    if !sum.is_finite() {
        return Err(());
    }
    let right_virtual = sum - left;
    let left_virtual = sum - right_virtual;
    let right_roundoff = right - right_virtual;
    let left_roundoff = left - left_virtual;
    Ok((sum, left_roundoff + right_roundoff))
}

fn fast_two_sum(left: f64, right: f64) -> Result<(f64, f64), ()> {
    let sum = left + right;
    if !sum.is_finite() {
        return Err(());
    }
    let virtual_right = sum - left;
    Ok((sum, right - virtual_right))
}

fn two_product(left: f64, right: f64) -> Result<(f64, f64), ()> {
    if left.is_subnormal() || right.is_subnormal() {
        return Err(());
    }
    let product = left * right;
    if !product.is_finite()
        || (left != 0.0 && right != 0.0 && (product == 0.0 || product.is_subnormal()))
    {
        return Err(());
    }
    let error = left.mul_add(right, -product);
    if !error.is_finite() || error.is_subnormal() {
        return Err(());
    }
    if error == 0.0
        && left != 0.0
        && right != 0.0
        && unbiased_exponent(left) + unbiased_exponent(right) < -970
        && !is_power_of_two(left)
        && !is_power_of_two(right)
    {
        return Err(());
    }
    Ok((product, error))
}

fn unbiased_exponent(value: f64) -> i32 {
    (((value.to_bits() >> 52) & 0x7ff) as i32) - 1023
}

fn is_power_of_two(value: f64) -> bool {
    value.to_bits() & ((1_u64 << 52) - 1) == 0
}

fn ordered_key(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 == 0 {
        bits | (1 << 63)
    } else {
        !bits
    }
}

fn f64_from_ordered_key(key: u64) -> f64 {
    let bits = if key >> 63 == 0 {
        !key
    } else {
        key & !(1 << 63)
    };
    f64::from_bits(bits)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackEvaluationError {
    NonFiniteChartTime,
    NonFiniteBase,
    BaseTypeMismatch,
    ValueTypeMismatch,
    NonFiniteProgress,
    NonFiniteResult,
    ReplaceConflict { priority: i64 },
    Gap { track: String },
    InvalidEasing { name: String },
    Easing(EasingError),
    InvalidBezier,
    BezierEnclosureUnavailable,
}

impl fmt::Display for TrackEvaluationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFiniteChartTime => formatter.write_str("Track query time must be finite"),
            Self::NonFiniteBase => formatter.write_str("Track base value must be finite"),
            Self::BaseTypeMismatch => {
                formatter.write_str("Track base value does not match the selected target")
            }
            Self::ValueTypeMismatch => formatter.write_str("Track values have incompatible types"),
            Self::NonFiniteProgress => formatter.write_str("Track progress is not finite"),
            Self::NonFiniteResult => {
                formatter.write_str("Track evaluation produced a non-finite value")
            }
            Self::ReplaceConflict { priority } => {
                write!(
                    formatter,
                    "multiple effective replace Tracks have priority {priority}"
                )
            }
            Self::Gap { track } => {
                write!(formatter, "Track {track} has no value at the query time")
            }
            Self::InvalidEasing { name } => write!(formatter, "unknown Core easing {name}"),
            Self::Easing(error) => write!(formatter, "Core easing failed: {error}"),
            Self::InvalidBezier => formatter.write_str("invalid cubic Bezier controls"),
            Self::BezierEnclosureUnavailable => {
                formatter.write_str("could not establish a correctly rounded cubic Bezier result")
            }
        }
    }
}

impl std::error::Error for TrackEvaluationError {}

#[cfg(test)]
mod tests {
    use fcs_model::{
        CanonicalTextualId, CanonicalTime, CanonicalTrackPoint, CanonicalTrackSegment, EntityKind,
        StableIdRegistry,
    };

    use super::*;

    fn owner(name: &str) -> StableId {
        StableIdRegistry::new()
            .insert(
                EntityKind::Line,
                CanonicalTextualId::explicit(name).unwrap(),
            )
            .unwrap()
    }

    fn time(value: f64) -> CanonicalTime {
        CanonicalTime::from_chart_time_seconds(value).unwrap()
    }

    fn point_track(
        owner: StableId,
        name: &str,
        target: CanonicalTrackTarget,
        blend: CanonicalTrackBlend,
        priority: i64,
        value: CanonicalTrackValue,
    ) -> CanonicalTrack {
        CanonicalTrack::new(
            owner,
            name,
            target,
            blend,
            priority,
            match blend {
                CanonicalTrackBlend::Replace => CanonicalTrackFill::Base,
                CanonicalTrackBlend::Add => CanonicalTrackFill::Zero,
                CanonicalTrackBlend::Multiply => CanonicalTrackFill::One,
            },
            CanonicalTrackFill::Base,
            CanonicalTrackFill::Base,
            vec![CanonicalTrackPiece::Point(
                CanonicalTrackPoint::new(time(0.0), value, 0).unwrap(),
            )],
        )
        .unwrap()
    }

    fn segment_track(
        owner: StableId,
        name: &str,
        target: CanonicalTrackTarget,
        start: CanonicalTrackValue,
        end: CanonicalTrackValue,
        interpolation: CanonicalTrackInterpolation,
    ) -> CanonicalTrack {
        CanonicalTrack::new(
            owner,
            name,
            target,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::HoldAfter,
            vec![CanonicalTrackPiece::Segment(
                CanonicalTrackSegment::new(time(0.0), time(2.0), start, end, interpolation, 0)
                    .unwrap(),
            )],
        )
        .unwrap()
    }

    #[test]
    fn segment_point_and_half_open_boundaries_select_exact_values() {
        let owner = owner("main");
        let track = CanonicalTrack::new(
            owner.clone(),
            "fade",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::HoldAfter,
            vec![
                CanonicalTrackPiece::Segment(
                    CanonicalTrackSegment::new(
                        time(0.0),
                        time(1.0),
                        CanonicalTrackValue::Float(0.0),
                        CanonicalTrackValue::Float(1.0),
                        CanonicalTrackInterpolation::Linear,
                        0,
                    )
                    .unwrap(),
                ),
                CanonicalTrackPiece::Point(
                    CanonicalTrackPoint::new(time(1.0), CanonicalTrackValue::Float(1.0), 1)
                        .unwrap(),
                ),
            ],
        )
        .unwrap();
        let tracks = CanonicalTrackSet::new(vec![track]).unwrap();

        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                -1.0,
                CanonicalTrackValue::Float(0.25),
            ),
            Ok(CanonicalTrackValue::Float(0.25))
        );
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.5,
                CanonicalTrackValue::Float(0.25),
            ),
            Ok(CanonicalTrackValue::Float(0.5))
        );
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                1.0,
                CanonicalTrackValue::Float(0.25),
            ),
            Ok(CanonicalTrackValue::Float(1.0))
        );
    }

    #[test]
    fn fill_hold_and_error_paths_are_explicit() {
        let owner = owner("main");
        let hold = CanonicalTrack::new(
            owner.clone(),
            "hold",
            CanonicalTrackTarget::Rotation,
            CanonicalTrackBlend::Replace,
            1,
            CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldAfter,
            vec![
                CanonicalTrackPiece::Segment(
                    CanonicalTrackSegment::new(
                        time(1.0),
                        time(2.0),
                        CanonicalTrackValue::Angle(2.0),
                        CanonicalTrackValue::Angle(3.0),
                        CanonicalTrackInterpolation::Linear,
                        0,
                    )
                    .unwrap(),
                ),
                CanonicalTrackPiece::Segment(
                    CanonicalTrackSegment::new(
                        time(3.0),
                        time(4.0),
                        CanonicalTrackValue::Angle(5.0),
                        CanonicalTrackValue::Angle(6.0),
                        CanonicalTrackInterpolation::Linear,
                        1,
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        let tracks = CanonicalTrackSet::new(vec![hold]).unwrap();
        for (query, expected) in [(0.0, 2.0), (2.5, 3.0), (5.0, 6.0)] {
            assert_eq!(
                evaluate_track_set(
                    &tracks,
                    &owner,
                    CanonicalTrackTarget::Rotation,
                    query,
                    CanonicalTrackValue::Angle(0.0),
                ),
                Ok(CanonicalTrackValue::Angle(expected))
            );
        }

        let error = CanonicalTrack::new(
            owner.clone(),
            "error",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::Error,
            CanonicalTrackFill::Error,
            CanonicalTrackFill::Error,
            vec![CanonicalTrackPiece::Segment(
                CanonicalTrackSegment::new(
                    time(1.0),
                    time(2.0),
                    CanonicalTrackValue::Float(0.0),
                    CanonicalTrackValue::Float(1.0),
                    CanonicalTrackInterpolation::Linear,
                    0,
                )
                .unwrap(),
            )],
        )
        .unwrap();
        let tracks = CanonicalTrackSet::new(vec![error]).unwrap();
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(1.0),
            ),
            Err(TrackEvaluationError::Gap {
                track: "error".to_owned()
            })
        );
    }

    #[test]
    fn hold_fills_use_segment_boundaries_and_not_points() {
        let owner = owner("main");
        let before = CanonicalTrack::new(
            owner.clone(),
            "before",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::HoldBefore,
            CanonicalTrackFill::Base,
            vec![
                CanonicalTrackPiece::Point(
                    CanonicalTrackPoint::new(time(4.0), CanonicalTrackValue::Float(9.0), 0)
                        .unwrap(),
                ),
                CanonicalTrackPiece::Segment(
                    CanonicalTrackSegment::new(
                        time(2.0),
                        time(3.0),
                        CanonicalTrackValue::Float(2.0),
                        CanonicalTrackValue::Float(3.0),
                        CanonicalTrackInterpolation::Linear,
                        1,
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_track(&before, -1.0),
            Ok(Some(CanonicalTrackValue::Float(2.0)))
        );

        let after = CanonicalTrack::new(
            owner,
            "after",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Replace,
            0,
            CanonicalTrackFill::HoldAfter,
            CanonicalTrackFill::Base,
            CanonicalTrackFill::HoldAfter,
            vec![
                CanonicalTrackPiece::Point(
                    CanonicalTrackPoint::new(time(0.0), CanonicalTrackValue::Float(9.0), 0)
                        .unwrap(),
                ),
                CanonicalTrackPiece::Segment(
                    CanonicalTrackSegment::new(
                        time(2.0),
                        time(3.0),
                        CanonicalTrackValue::Float(2.0),
                        CanonicalTrackValue::Float(3.0),
                        CanonicalTrackInterpolation::Linear,
                        1,
                    )
                    .unwrap(),
                ),
            ],
        )
        .unwrap();
        assert_eq!(
            evaluate_track(&after, 3.5),
            Ok(Some(CanonicalTrackValue::Float(3.0)))
        );
    }

    #[test]
    fn step_and_all_core_easing_names_are_dispatchable() {
        let owner = owner("main");
        let step = segment_track(
            owner.clone(),
            "step",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackValue::Float(2.0),
            CanonicalTrackValue::Float(8.0),
            CanonicalTrackInterpolation::Step,
        );
        assert_eq!(
            evaluate_track(&step, 1.5),
            Ok(Some(CanonicalTrackValue::Float(2.0)))
        );

        for easing in EasingId::ALL {
            let track = segment_track(
                owner.clone(),
                easing.name(),
                CanonicalTrackTarget::Alpha,
                CanonicalTrackValue::Float(0.0),
                CanonicalTrackValue::Float(1.0),
                CanonicalTrackInterpolation::Easing(easing.name().to_owned()),
            );
            assert_eq!(
                evaluate_track(&track, 1.0),
                Ok(Some(CanonicalTrackValue::Float(
                    easing.evaluate(0.5).unwrap()
                )))
            );
        }
    }

    #[test]
    fn layered_blend_uses_priority_then_owner_local_track_identity() {
        let owner = owner("main");
        let tracks = CanonicalTrackSet::new(vec![
            point_track(
                owner.clone(),
                "replace-low",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Replace,
                0,
                CanonicalTrackValue::Float(2.0),
            ),
            point_track(
                owner.clone(),
                "replace-high",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Replace,
                1,
                CanonicalTrackValue::Float(1.0e16),
            ),
            point_track(
                owner.clone(),
                "a",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Add,
                0,
                CanonicalTrackValue::Float(-1.0e16),
            ),
            point_track(
                owner.clone(),
                "z",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Add,
                0,
                CanonicalTrackValue::Float(1.0),
            ),
            point_track(
                owner.clone(),
                "scale",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Multiply,
                0,
                CanonicalTrackValue::Float(2.0),
            ),
        ])
        .unwrap();
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(0.0),
            ),
            Ok(CanonicalTrackValue::Float(2.0))
        );

        let reversed = CanonicalTrackSet::new(vec![
            point_track(
                owner.clone(),
                "z",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Add,
                0,
                CanonicalTrackValue::Float(1.0),
            ),
            point_track(
                owner.clone(),
                "a",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Add,
                0,
                CanonicalTrackValue::Float(-1.0e16),
            ),
            point_track(
                owner.clone(),
                "replace-high",
                CanonicalTrackTarget::Alpha,
                CanonicalTrackBlend::Replace,
                1,
                CanonicalTrackValue::Float(1.0e16),
            ),
        ])
        .unwrap();
        assert_eq!(
            evaluate_track_set(
                &reversed,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(0.0),
            ),
            Ok(CanonicalTrackValue::Float(1.0e16 - 1.0e16 + 1.0))
        );
    }

    #[test]
    fn add_and_multiply_gap_fills_are_typed_identities() {
        let owner = owner("main");
        let piece = |value| {
            vec![CanonicalTrackPiece::Segment(
                CanonicalTrackSegment::new(
                    time(1.0),
                    time(2.0),
                    CanonicalTrackValue::Float(value),
                    CanonicalTrackValue::Float(value),
                    CanonicalTrackInterpolation::Step,
                    0,
                )
                .unwrap(),
            )]
        };
        let add = CanonicalTrack::new(
            owner.clone(),
            "add",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Add,
            0,
            CanonicalTrackFill::Zero,
            CanonicalTrackFill::Zero,
            CanonicalTrackFill::Zero,
            piece(3.0),
        )
        .unwrap();
        let multiply = CanonicalTrack::new(
            owner.clone(),
            "multiply",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Multiply,
            0,
            CanonicalTrackFill::One,
            CanonicalTrackFill::One,
            CanonicalTrackFill::One,
            piece(4.0),
        )
        .unwrap();
        let tracks = CanonicalTrackSet::new(vec![multiply, add]).unwrap();

        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(5.0),
            ),
            Ok(CanonicalTrackValue::Float(5.0))
        );
    }

    #[test]
    fn typed_linear_and_core_easing_share_one_progress() {
        let owner = owner("main");
        let position = segment_track(
            owner.clone(),
            "position",
            CanonicalTrackTarget::Position,
            CanonicalTrackValue::Vec2Length(CanonicalVec2::new(0.0, 10.0).unwrap()),
            CanonicalTrackValue::Vec2Length(CanonicalVec2::new(10.0, 30.0).unwrap()),
            CanonicalTrackInterpolation::Linear,
        );
        let alpha = segment_track(
            owner.clone(),
            "alpha",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackValue::Float(0.0),
            CanonicalTrackValue::Float(1.0),
            CanonicalTrackInterpolation::Easing("easeInQuad".to_owned()),
        );
        let tracks = CanonicalTrackSet::new(vec![alpha, position]).unwrap();
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Position,
                1.0,
                CanonicalTrackValue::Vec2Length(CanonicalVec2::new(9.0, 9.0).unwrap()),
            ),
            Ok(CanonicalTrackValue::Vec2Length(
                CanonicalVec2::new(5.0, 20.0).unwrap()
            ))
        );
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                1.0,
                CanonicalTrackValue::Float(1.0),
            ),
            Ok(CanonicalTrackValue::Float(0.25))
        );
    }

    #[test]
    fn cubic_bezier_returns_only_certified_binary64_results() {
        assert_eq!(cubic_bezier_progress([0.25, 2.0, 0.75, -1.0], 0.0), Ok(0.0));
        assert_eq!(cubic_bezier_progress([0.25, 2.0, 0.75, -1.0], 1.0), Ok(1.0));
        assert_eq!(cubic_bezier_progress([0.0, 0.0, 1.0, 1.0], 0.25), Ok(0.25));
        assert_eq!(cubic_bezier_progress([0.5, 2.0, 0.5, 2.0], 0.5), Ok(1.625));
        assert_eq!(
            cubic_bezier_progress([0.25, 0.5, 0.75, f64::from_bits(1)], 0.25),
            Err(TrackEvaluationError::BezierEnclosureUnavailable)
        );
    }

    #[test]
    fn query_and_result_errors_are_stable() {
        let owner = owner("main");
        let tracks = CanonicalTrackSet::new(Vec::new()).unwrap();
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                f64::NAN,
                CanonicalTrackValue::Float(1.0),
            ),
            Err(TrackEvaluationError::NonFiniteChartTime)
        );
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Angle(0.0),
            ),
            Err(TrackEvaluationError::BaseTypeMismatch)
        );
        assert_eq!(
            evaluate_track_set(
                &tracks,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(f64::NAN),
            ),
            Err(TrackEvaluationError::NonFiniteBase)
        );

        let overflowing_add = CanonicalTrackSet::new(vec![point_track(
            owner.clone(),
            "overflow",
            CanonicalTrackTarget::Alpha,
            CanonicalTrackBlend::Add,
            0,
            CanonicalTrackValue::Float(f64::MAX),
        )])
        .unwrap();
        assert_eq!(
            evaluate_track_set(
                &overflowing_add,
                &owner,
                CanonicalTrackTarget::Alpha,
                0.0,
                CanonicalTrackValue::Float(f64::MAX),
            ),
            Err(TrackEvaluationError::NonFiniteResult)
        );
    }
}
