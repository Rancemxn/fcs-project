use std::collections::BTreeSet;

use astro_float::{BigFloat, Consts, Radix, RoundingMode};
use fcs_model::{
    CanonicalChartScrollTempoPoint, CanonicalExpressionDag, CanonicalExpressionNode,
    CanonicalExpressionOpcode, CanonicalExpressionType, CanonicalExpressionValue, CanonicalLine,
    CanonicalLineBase, CanonicalLineGraph, CanonicalLineInherit, CanonicalScrollCoordinate,
    CanonicalScrollLine, CanonicalScrollSet, CanonicalScrollTempo, CanonicalTextualId,
    CanonicalTime, CanonicalTrack, CanonicalTrackBlend, CanonicalTrackFill,
    CanonicalTrackInterpolation, CanonicalTrackPiece, CanonicalTrackSegment, CanonicalTrackSet,
    CanonicalTrackTarget, CanonicalTrackValue, CanonicalVec2, EntityKind, StableId,
    StableIdRegistry,
};
use fcs_runtime::{
    EasingId, ExpressionEnvironment, evaluate_easing, evaluate_expression, evaluate_line_scroll,
    evaluate_line_transform, evaluate_track_set,
};
use serde::Deserialize;

const INITIAL_REFERENCE_PRECISION: usize = 256;
const MAX_REFERENCE_PRECISION: usize = 4096;
const NUMERIC_VECTORS: &str =
    include_str!("../../../docs/conformance/fcs5/expected/numeric-vectors.toml");

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NumericVectors {
    schema_version: u32,
    rounding: String,
    #[serde(default)]
    constant: Vec<ConstantVector>,
    #[serde(default)]
    operation: Vec<OperationVector>,
    #[serde(default)]
    difficult_operation: Vec<DifficultOperationVector>,
    #[serde(default)]
    easing: Vec<EasingVector>,
    #[serde(default)]
    tempo: Vec<TempoVector>,
    #[serde(default)]
    scroll: Vec<ScrollVector>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstantVector {
    name: String,
    hex_bits: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationVector {
    expression: String,
    hex_bits: Option<String>,
    #[serde(rename = "type")]
    value_type: Option<String>,
    value: Option<bool>,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DifficultOperationVector {
    opcode: String,
    input_hex_bits: String,
    right_hex_bits: Option<String>,
    output_hex_bits: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EasingVector {
    name: String,
    x_hex_bits: String,
    y_hex_bits: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TempoVector {
    bpm: f64,
    beat: String,
    chart_time_seconds_hex_bits: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScrollVector {
    scroll_bpm: f64,
    chart_time_seconds: f64,
    q_hex_bits: String,
    distance_at_speed_one_hex_bits: String,
}

fn reference_value(operation: impl Fn(usize, RoundingMode, &mut Consts) -> BigFloat) -> f64 {
    let mut precision = INITIAL_REFERENCE_PRECISION;
    while precision <= MAX_REFERENCE_PRECISION {
        let mut constants = Consts::new().expect("Astro-float constants cache must initialize");
        let lower = operation(precision, RoundingMode::Down, &mut constants);
        let upper = operation(precision, RoundingMode::Up, &mut constants);
        let lower = big_float_to_f64(&lower, &mut constants);
        let upper = big_float_to_f64(&upper, &mut constants);
        if lower.to_bits() == upper.to_bits() {
            return lower;
        }
        precision *= 2;
    }
    panic!("Astro-float enclosure did not establish one binary64 result");
}

fn reference_value_at(
    precision: usize,
    operation: impl Fn(usize, RoundingMode, &mut Consts) -> BigFloat,
) -> f64 {
    let mut constants = Consts::new().expect("Astro-float constants cache must initialize");
    let lower = operation(precision, RoundingMode::Down, &mut constants);
    let upper = operation(precision, RoundingMode::Up, &mut constants);
    let lower = big_float_to_f64(&lower, &mut constants);
    let upper = big_float_to_f64(&upper, &mut constants);
    assert_eq!(
        lower.to_bits(),
        upper.to_bits(),
        "fixed Astro-float easing precision did not enclose one binary64 result"
    );
    lower
}

fn big_float_to_f64(value: &BigFloat, constants: &mut Consts) -> f64 {
    assert!(
        !value.is_nan(),
        "Astro-float operation unexpectedly returned NaN"
    );
    value
        .format(Radix::Dec, RoundingMode::None, constants)
        .expect("Astro-float decimal formatting must succeed")
        .parse()
        .expect("an Astro-float decimal must parse as binary64")
}

fn reference_unary(opcode: CanonicalExpressionOpcode, input: f64) -> f64 {
    reference_value(|precision, rounding, constants| {
        let input = BigFloat::from_f64(input, precision);
        match opcode {
            CanonicalExpressionOpcode::Sqrt => input.sqrt(precision, rounding),
            CanonicalExpressionOpcode::Exp => input.exp(precision, rounding, constants),
            CanonicalExpressionOpcode::Ln => input.ln(precision, rounding, constants),
            CanonicalExpressionOpcode::Sin => input.sin(precision, rounding, constants),
            CanonicalExpressionOpcode::Cos => input.cos(precision, rounding, constants),
            CanonicalExpressionOpcode::Tan => input.tan(precision, rounding, constants),
            CanonicalExpressionOpcode::Asin => input.asin(precision, rounding, constants),
            CanonicalExpressionOpcode::Acos => input.acos(precision, rounding, constants),
            CanonicalExpressionOpcode::Atan => input.atan(precision, rounding, constants),
            _ => panic!("unsupported unary reference opcode {opcode:?}"),
        }
    })
}

fn reference_binary(opcode: CanonicalExpressionOpcode, left: f64, right: f64) -> f64 {
    match opcode {
        CanonicalExpressionOpcode::Atan2 => reference_atan2(left, right),
        CanonicalExpressionOpcode::Pow => reference_value(|precision, rounding, constants| {
            BigFloat::from_f64(left, precision).pow(
                &BigFloat::from_f64(right, precision),
                precision,
                rounding,
                constants,
            )
        }),
        _ => panic!("unsupported binary reference opcode {opcode:?}"),
    }
}

fn reference_easing(easing: EasingId, input: f64) -> f64 {
    if input == 0.0 {
        return 0.0;
    }
    if input == 1.0 {
        return 1.0;
    }
    let ease_in = |family: u16, value: f64| -> f64 {
        match family {
            0 => {
                1.0 - reference_easing_unary(
                    CanonicalExpressionOpcode::Cos,
                    (std::f64::consts::PI * value) / 2.0,
                )
            }
            1 => value * value,
            2 => (value * value) * value,
            3 => ((value * value) * value) * value,
            4 => (((value * value) * value) * value) * value,
            5 => {
                reference_easing_binary(CanonicalExpressionOpcode::Pow, 2.0, (10.0 * value) - 10.0)
            }
            6 => {
                1.0 - reference_easing_unary(CanonicalExpressionOpcode::Sqrt, 1.0 - (value * value))
            }
            7 => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                (c3 * ((value * value) * value)) - (c1 * (value * value))
            }
            8 => {
                let exponent = (10.0 * value) - 10.0;
                let angle = ((((10.0 * value) - 10.75) * 2.0) * std::f64::consts::PI) / 3.0;
                -reference_easing_binary(CanonicalExpressionOpcode::Pow, 2.0, exponent)
                    * reference_easing_unary(CanonicalExpressionOpcode::Sin, angle)
            }
            9 => 1.0 - reference_bounce(1.0 - value),
            _ => unreachable!(),
        }
    };
    let ease_out = |family: u16, value: f64| 1.0 - ease_in(family, 1.0 - value);
    let ease_in_out = |family: u16, value: f64| {
        if value < 0.5 {
            ease_in(family, 2.0 * value) / 2.0
        } else {
            1.0 - (ease_in(family, 2.0 - (2.0 * value)) / 2.0)
        }
    };
    let bounce = |value: f64| reference_bounce(value);
    match easing {
        EasingId::Linear => input,
        EasingId::EaseInSine => ease_in(0, input),
        EasingId::EaseOutSine => ease_out(0, input),
        EasingId::EaseInOutSine => ease_in_out(0, input),
        EasingId::EaseInQuad => ease_in(1, input),
        EasingId::EaseOutQuad => ease_out(1, input),
        EasingId::EaseInOutQuad => ease_in_out(1, input),
        EasingId::EaseInCubic => ease_in(2, input),
        EasingId::EaseOutCubic => ease_out(2, input),
        EasingId::EaseInOutCubic => ease_in_out(2, input),
        EasingId::EaseInQuart => ease_in(3, input),
        EasingId::EaseOutQuart => ease_out(3, input),
        EasingId::EaseInOutQuart => ease_in_out(3, input),
        EasingId::EaseInQuint => ease_in(4, input),
        EasingId::EaseOutQuint => ease_out(4, input),
        EasingId::EaseInOutQuint => ease_in_out(4, input),
        EasingId::EaseInExpo => ease_in(5, input),
        EasingId::EaseOutExpo => ease_out(5, input),
        EasingId::EaseInOutExpo => ease_in_out(5, input),
        EasingId::EaseInCirc => ease_in(6, input),
        EasingId::EaseOutCirc => ease_out(6, input),
        EasingId::EaseInOutCirc => ease_in_out(6, input),
        EasingId::EaseInBack => ease_in(7, input),
        EasingId::EaseOutBack => ease_out(7, input),
        EasingId::EaseInOutBack => ease_in_out(7, input),
        EasingId::EaseInElastic => ease_in(8, input),
        EasingId::EaseOutElastic => ease_out(8, input),
        EasingId::EaseInOutElastic => ease_in_out(8, input),
        EasingId::EaseInBounce => ease_in(9, input),
        EasingId::EaseOutBounce => bounce(input),
        EasingId::EaseInOutBounce => ease_in_out(9, input),
    }
}

fn reference_easing_unary(opcode: CanonicalExpressionOpcode, input: f64) -> f64 {
    reference_value_at(256, |precision, rounding, constants| {
        let input = BigFloat::from_f64(input, precision);
        match opcode {
            CanonicalExpressionOpcode::Sqrt => input.sqrt(precision, rounding),
            CanonicalExpressionOpcode::Sin => input.sin(precision, rounding, constants),
            CanonicalExpressionOpcode::Cos => input.cos(precision, rounding, constants),
            _ => panic!("unsupported easing unary reference opcode {opcode:?}"),
        }
    })
}

fn reference_easing_binary(opcode: CanonicalExpressionOpcode, left: f64, right: f64) -> f64 {
    reference_value_at(256, |precision, rounding, constants| match opcode {
        CanonicalExpressionOpcode::Pow if left == 2.0 => {
            let working_precision = precision + 64;
            let exponent = BigFloat::from_f64(right, working_precision);
            let logarithm = constants.ln_2(working_precision, RoundingMode::None);
            let product = exponent.mul(&logarithm, working_precision, RoundingMode::None);
            product.exp(precision, rounding, constants)
        }
        CanonicalExpressionOpcode::Pow => BigFloat::from_f64(left, precision).pow(
            &BigFloat::from_f64(right, precision),
            precision,
            rounding,
            constants,
        ),
        _ => panic!("unsupported easing binary reference opcode {opcode:?}"),
    })
}

fn reference_bounce(input: f64) -> f64 {
    let n1 = 7.5625;
    let d1 = 2.75;
    if input < 1.0 / d1 {
        n1 * (input * input)
    } else if input < 2.0 / d1 {
        let shifted = input - (1.5 / d1);
        (n1 * (shifted * shifted)) + 0.75
    } else if input < 2.5 / d1 {
        let shifted = input - (2.25 / d1);
        (n1 * (shifted * shifted)) + 0.9375
    } else {
        let shifted = input - (2.625 / d1);
        (n1 * (shifted * shifted)) + 0.984375
    }
}

fn reference_atan2(y: f64, x: f64) -> f64 {
    if y == 0.0 {
        if x.is_sign_positive() {
            return y;
        }
        let magnitude =
            reference_value(|precision, rounding, constants| constants.pi(precision, rounding));
        return magnitude.copysign(y);
    }
    if x == 0.0 {
        let magnitude = reference_value(|precision, rounding, constants| {
            constants.pi(precision, rounding).div(
                &BigFloat::from_f64(2.0, precision),
                precision,
                rounding,
            )
        });
        return magnitude.copysign(y);
    }

    reference_value(|precision, rounding, constants| {
        let ratio = BigFloat::from_f64(y, precision).div(
            &BigFloat::from_f64(x, precision),
            precision,
            rounding,
        );
        let angle = ratio.atan(precision, rounding, constants);
        if x.is_sign_positive() {
            angle
        } else if y.is_sign_positive() {
            angle.add(&constants.pi(precision, rounding), precision, rounding)
        } else {
            let pi_rounding = match rounding {
                RoundingMode::Down => RoundingMode::Up,
                RoundingMode::Up => RoundingMode::Down,
                _ => unreachable!("reference_value uses directed rounding"),
            };
            angle.sub(&constants.pi(precision, pi_rounding), precision, rounding)
        }
    })
}

fn constant(value: CanonicalExpressionValue) -> CanonicalExpressionNode {
    let result_type = value.value_type();
    CanonicalExpressionNode::new(
        CanonicalExpressionOpcode::Constant,
        result_type,
        [None, None, None],
        Some(value),
        0,
    )
}

fn product_unary(opcode: CanonicalExpressionOpcode, input: f64) -> f64 {
    product_expression(
        vec![
            constant(CanonicalExpressionValue::Float(input)),
            CanonicalExpressionNode::new(
                opcode,
                CanonicalExpressionType::Float,
                [Some(0), None, None],
                None,
                0,
            ),
        ],
        1,
    )
}

fn product_binary(opcode: CanonicalExpressionOpcode, left: f64, right: f64) -> f64 {
    product_expression(
        vec![
            constant(CanonicalExpressionValue::Float(left)),
            constant(CanonicalExpressionValue::Float(right)),
            CanonicalExpressionNode::new(
                opcode,
                CanonicalExpressionType::Float,
                [Some(0), Some(1), None],
                None,
                0,
            ),
        ],
        2,
    )
}

fn product_expression(nodes: Vec<CanonicalExpressionNode>, root: usize) -> f64 {
    let expression = CanonicalExpressionDag::new(nodes, root)
        .expect("reference fixture must be a valid expression");
    let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
    match evaluate_expression(&expression, environment).unwrap() {
        CanonicalExpressionValue::Float(value) => value,
        value => panic!("unexpected numeric result {value:?}"),
    }
}

fn parse_bits(value: &str) -> Result<u64, String> {
    if value.len() != 16 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "expected exactly 16 hexadecimal digits, got {value:?}"
        ));
    }
    u64::from_str_radix(value, 16).map_err(|error| error.to_string())
}

fn line_id(name: &str) -> StableId {
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

fn bezier_product(controls: [f64; 4], progress: f64) -> f64 {
    let owner = line_id("bezier-reference");
    let track = CanonicalTrack::new(
        owner.clone(),
        "bezier",
        CanonicalTrackTarget::Alpha,
        CanonicalTrackBlend::Replace,
        0,
        CanonicalTrackFill::Base,
        CanonicalTrackFill::Base,
        CanonicalTrackFill::HoldAfter,
        vec![CanonicalTrackPiece::Segment(
            CanonicalTrackSegment::new(
                time(0.0),
                time(1.0),
                CanonicalTrackValue::Float(0.0),
                CanonicalTrackValue::Float(1.0),
                CanonicalTrackInterpolation::CubicBezier(controls),
                0,
            )
            .unwrap(),
        )],
    )
    .unwrap();
    match evaluate_track_set(
        &CanonicalTrackSet::new(vec![track]).unwrap(),
        &owner,
        CanonicalTrackTarget::Alpha,
        progress,
        CanonicalTrackValue::Float(0.0),
    )
    .unwrap()
    {
        CanonicalTrackValue::Float(value) => value,
        value => panic!("unexpected Bezier Track result {value:?}"),
    }
}

fn scroll_product(scroll_bpm: f64, chart_time: f64) -> (f64, f64) {
    let id = line_id("numeric-corpus-scroll");
    let graph = CanonicalLineGraph::new([CanonicalLine::new(
        id.clone(),
        None,
        0,
        CanonicalLineBase::identity(),
        CanonicalLineInherit::default(),
        CanonicalScrollTempo::Global,
    )
    .unwrap()])
    .unwrap();
    let scroll = CanonicalScrollSet::new(vec![
        CanonicalScrollLine::new(
            id.clone(),
            CanonicalScrollCoordinate::new([
                CanonicalChartScrollTempoPoint::new(0.0, scroll_bpm).unwrap()
            ])
            .unwrap(),
            1.0,
            false,
            1.0,
            0.0,
            0.0,
        )
        .unwrap(),
    ])
    .unwrap();
    let tracks = CanonicalTrackSet::new(Vec::new()).unwrap();
    let evaluated = evaluate_line_scroll(&graph, &scroll, &tracks, &id, chart_time).unwrap();
    (evaluated.local_q(), evaluated.local_floor())
}

fn varying_scroll_product(interpolation: CanonicalTrackInterpolation, chart_time: f64) -> f64 {
    let id = line_id("varying-reference-scroll");
    let graph = CanonicalLineGraph::new([CanonicalLine::new(
        id.clone(),
        None,
        0,
        CanonicalLineBase::identity(),
        CanonicalLineInherit::default(),
        CanonicalScrollTempo::Global,
    )
    .unwrap()])
    .unwrap();
    let scroll = CanonicalScrollSet::new(vec![
        CanonicalScrollLine::new(
            id.clone(),
            CanonicalScrollCoordinate::new([
                CanonicalChartScrollTempoPoint::new(0.0, 120.0).unwrap()
            ])
            .unwrap(),
            1.0,
            false,
            1.0,
            0.0,
            0.0,
        )
        .unwrap(),
    ])
    .unwrap();
    let track = CanonicalTrack::new(
        id.clone(),
        "varying-speed",
        CanonicalTrackTarget::ScrollSpeed,
        CanonicalTrackBlend::Replace,
        0,
        CanonicalTrackFill::HoldAfter,
        CanonicalTrackFill::HoldBefore,
        CanonicalTrackFill::HoldAfter,
        vec![CanonicalTrackPiece::Segment(
            CanonicalTrackSegment::new(
                time(0.0),
                time(2.0),
                CanonicalTrackValue::Float(0.0),
                CanonicalTrackValue::Float(2.0),
                interpolation,
                0,
            )
            .unwrap(),
        )],
    )
    .unwrap();
    let tracks = CanonicalTrackSet::new(vec![track]).unwrap();
    evaluate_line_scroll(&graph, &scroll, &tracks, &id, chart_time)
        .unwrap()
        .local_floor()
}

fn execute_vectors(source: &str) -> Result<(), String> {
    let vectors: NumericVectors = toml::from_str(source).map_err(|error| error.to_string())?;
    if vectors.schema_version != 1 {
        return Err("numeric vector schema_version must be 1".to_owned());
    }
    if vectors.rounding != "binary64-roundTiesToEven" {
        return Err("numeric vector rounding policy is unknown".to_owned());
    }

    let mut constants_seen = BTreeSet::new();
    for vector in vectors.constant {
        if !constants_seen.insert(vector.name.clone()) {
            return Err(format!("duplicate numeric constant {:?}", vector.name));
        }
        let expected = parse_bits(&vector.hex_bits)?;
        let (product, reference) = match vector.name.as_str() {
            "pi" => (
                std::f64::consts::PI,
                reference_value(|precision, rounding, constants| constants.pi(precision, rounding)),
            ),
            "tau" => (
                std::f64::consts::TAU,
                reference_value(|precision, rounding, constants| {
                    constants.pi(precision, rounding).mul(
                        &BigFloat::from_f64(2.0, precision),
                        precision,
                        rounding,
                    )
                }),
            ),
            name => return Err(format!("unknown numeric constant {name:?}")),
        };
        if product.to_bits() != expected || reference.to_bits() != expected {
            return Err(format!(
                "constant {} did not match {:016x}",
                vector.name, expected
            ));
        }
    }

    let mut operations_seen = BTreeSet::new();
    for vector in vectors.operation {
        if !operations_seen.insert(vector.expression.clone()) {
            return Err(format!(
                "duplicate numeric operation {:?}",
                vector.expression
            ));
        }
        match vector.expression.as_str() {
            "sqrt(4.0)" | "sin(0.0)" | "cos(0.0)" | "exp(0.0)" | "ln(1.0)" => {
                if vector.value_type.is_some() || vector.value.is_some() || vector.note.is_some() {
                    return Err(format!(
                        "invalid numeric operation shape for {}",
                        vector.expression
                    ));
                }
                let expected = parse_bits(vector.hex_bits.as_deref().ok_or_else(|| {
                    format!("numeric operation {} lacks hex_bits", vector.expression)
                })?)?;
                let (opcode, input) = match vector.expression.as_str() {
                    "sqrt(4.0)" => (CanonicalExpressionOpcode::Sqrt, 4.0),
                    "sin(0.0)" => (CanonicalExpressionOpcode::Sin, 0.0),
                    "cos(0.0)" => (CanonicalExpressionOpcode::Cos, 0.0),
                    "exp(0.0)" => (CanonicalExpressionOpcode::Exp, 0.0),
                    "ln(1.0)" => (CanonicalExpressionOpcode::Ln, 1.0),
                    _ => unreachable!(),
                };
                if product_unary(opcode, input).to_bits() != expected
                    || reference_unary(opcode, input).to_bits() != expected
                {
                    return Err(format!(
                        "operation {} did not match {:016x}",
                        vector.expression, expected
                    ));
                }
            }
            "false && (1.0 / 0.0 > 0.0)" | "true || (1.0 / 0.0 > 0.0)" => {
                if vector.hex_bits.is_some()
                    || vector.value_type.as_deref() != Some("bool")
                    || vector.note.as_deref() != Some("rhs is not evaluated")
                {
                    return Err(format!(
                        "invalid lazy operation shape for {}",
                        vector.expression
                    ));
                }
                let expected = vector
                    .value
                    .ok_or_else(|| format!("lazy operation {} lacks value", vector.expression))?;
                if execute_lazy_boolean(vector.expression.starts_with("true")) != expected {
                    return Err(format!(
                        "lazy operation {} did not match",
                        vector.expression
                    ));
                }
            }
            expression => return Err(format!("unknown numeric operation {expression:?}")),
        }
    }

    let mut difficult_operations_seen = BTreeSet::new();
    for vector in vectors.difficult_operation {
        let identity = (
            vector.opcode.clone(),
            vector.input_hex_bits.clone(),
            vector.right_hex_bits.clone(),
        );
        if !difficult_operations_seen.insert(identity) {
            return Err(format!("duplicate difficult operation {:?}", vector.opcode));
        }
        let input = f64::from_bits(parse_bits(&vector.input_hex_bits)?);
        let expected = parse_bits(&vector.output_hex_bits)?;
        let opcode = match vector.opcode.as_str() {
            "sqrt" => CanonicalExpressionOpcode::Sqrt,
            "exp" => CanonicalExpressionOpcode::Exp,
            "ln" => CanonicalExpressionOpcode::Ln,
            "sin" => CanonicalExpressionOpcode::Sin,
            "cos" => CanonicalExpressionOpcode::Cos,
            "tan" => CanonicalExpressionOpcode::Tan,
            "asin" => CanonicalExpressionOpcode::Asin,
            "acos" => CanonicalExpressionOpcode::Acos,
            "atan" => CanonicalExpressionOpcode::Atan,
            "atan2" => CanonicalExpressionOpcode::Atan2,
            "pow" => CanonicalExpressionOpcode::Pow,
            opcode => return Err(format!("unknown difficult operation {opcode:?}")),
        };
        let (product, reference) = if matches!(
            opcode,
            CanonicalExpressionOpcode::Atan2 | CanonicalExpressionOpcode::Pow
        ) {
            let right = f64::from_bits(parse_bits(vector.right_hex_bits.as_deref().ok_or_else(
                || format!("binary operation {} lacks right_hex_bits", vector.opcode),
            )?)?);
            (
                product_binary(opcode, input, right),
                reference_binary(opcode, input, right),
            )
        } else {
            if vector.right_hex_bits.is_some() {
                return Err(format!(
                    "unary operation {} must not have right_hex_bits",
                    vector.opcode
                ));
            }
            (product_unary(opcode, input), reference_unary(opcode, input))
        };
        if product.to_bits() != expected || reference.to_bits() != expected {
            return Err(format!(
                "difficult operation {} did not match {:016x}",
                vector.opcode, expected
            ));
        }
    }

    let mut easings_seen = BTreeSet::new();
    for vector in vectors.easing {
        if !easings_seen.insert((vector.name.clone(), vector.x_hex_bits.clone())) {
            return Err(format!("duplicate easing {:?}", vector.name));
        }
        let input = f64::from_bits(parse_bits(&vector.x_hex_bits)?);
        let expected = parse_bits(&vector.y_hex_bits)?;
        let easing = EasingId::ALL
            .into_iter()
            .find(|easing| easing.name() == vector.name)
            .ok_or_else(|| format!("unknown easing {:?}", vector.name))?;
        if evaluate_easing(easing.abi_id(), input)
            .map_err(|error| error.to_string())?
            .to_bits()
            != expected
            || reference_easing(easing, input).to_bits() != expected
        {
            return Err(format!(
                "easing {} did not match {:016x}",
                vector.name, expected
            ));
        }
    }

    let mut tempos_seen = BTreeSet::new();
    for vector in vectors.tempo {
        if !tempos_seen.insert((vector.bpm.to_bits(), vector.beat.clone())) {
            return Err("duplicate tempo vector".to_owned());
        }
        if vector.beat != "2/1" || !vector.bpm.is_finite() || vector.bpm <= 0.0 {
            return Err("unknown or invalid tempo vector".to_owned());
        }
        let expected = parse_bits(&vector.chart_time_seconds_hex_bits)?;
        let chart_time = 2.0 * 60.0 / vector.bpm;
        if chart_time.to_bits() != expected {
            return Err("tempo vector did not match expected chart time".to_owned());
        }
    }

    let mut scroll_seen = BTreeSet::new();
    for vector in vectors.scroll {
        if !scroll_seen.insert((
            vector.scroll_bpm.to_bits(),
            vector.chart_time_seconds.to_bits(),
        )) {
            return Err("duplicate scroll vector".to_owned());
        }
        let expected_q = parse_bits(&vector.q_hex_bits)?;
        let expected_distance = parse_bits(&vector.distance_at_speed_one_hex_bits)?;
        let (q, distance) = scroll_product(vector.scroll_bpm, vector.chart_time_seconds);
        if q.to_bits() != expected_q || distance.to_bits() != expected_distance {
            return Err("scroll vector did not match expected values".to_owned());
        }
    }
    Ok(())
}

fn execute_lazy_boolean(is_or: bool) -> bool {
    let nodes = vec![
        constant(CanonicalExpressionValue::Bool(is_or)),
        constant(CanonicalExpressionValue::Float(1.0)),
        constant(CanonicalExpressionValue::Float(0.0)),
        CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Div,
            CanonicalExpressionType::Float,
            [Some(1), Some(2), None],
            None,
            0,
        ),
        CanonicalExpressionNode::new(
            CanonicalExpressionOpcode::Gt,
            CanonicalExpressionType::Bool,
            [Some(3), Some(2), None],
            None,
            0,
        ),
        CanonicalExpressionNode::new(
            if is_or {
                CanonicalExpressionOpcode::Or
            } else {
                CanonicalExpressionOpcode::And
            },
            CanonicalExpressionType::Bool,
            [Some(0), Some(4), None],
            None,
            0,
        ),
    ];
    let expression = CanonicalExpressionDag::new(nodes, 5).unwrap();
    let environment = ExpressionEnvironment::new(0.0, 0.0, 0.0, 0.0).unwrap();
    match evaluate_expression(&expression, environment).unwrap() {
        CanonicalExpressionValue::Bool(value) => value,
        value => panic!("unexpected lazy Boolean result {value:?}"),
    }
}

#[test]
fn all_core_transcendentals_match_the_astro_float_reference_bits() {
    let unary = [
        (
            CanonicalExpressionOpcode::Sqrt,
            f64::from_bits(0x4000_0000_0000_0001),
        ),
        (
            CanonicalExpressionOpcode::Exp,
            f64::from_bits(0x3fd5_5555_5555_5555),
        ),
        (
            CanonicalExpressionOpcode::Ln,
            f64::from_bits(0x3ff0_0000_0000_0001),
        ),
        (
            CanonicalExpressionOpcode::Sin,
            f64::from_bits(0x4415_af1d_78b5_8c40),
        ),
        (
            CanonicalExpressionOpcode::Cos,
            f64::from_bits(0x4415_af1d_78b5_8c40),
        ),
        (
            CanonicalExpressionOpcode::Tan,
            f64::from_bits(0x4415_af1d_78b5_8c40),
        ),
        (
            CanonicalExpressionOpcode::Asin,
            f64::from_bits(0x3fef_ffff_ffff_ffff),
        ),
        (
            CanonicalExpressionOpcode::Acos,
            f64::from_bits(0x3fef_ffff_ffff_ffff),
        ),
        (CanonicalExpressionOpcode::Atan, f64::MAX),
    ];
    for (opcode, input) in unary {
        let product = product_unary(opcode, input);
        let reference = reference_unary(opcode, input);
        assert_eq!(
            product.to_bits(),
            reference.to_bits(),
            "{opcode:?}({input:?}): product={:016x}, reference={:016x}",
            product.to_bits(),
            reference.to_bits()
        );
    }

    let binary = [
        (
            CanonicalExpressionOpcode::Atan2,
            f64::from_bits(0x7e37_e43c_8800_759c),
            -f64::from_bits(0x01a5_6e1f_c2f8_f359),
        ),
        (
            CanonicalExpressionOpcode::Pow,
            f64::from_bits(0x3ff0_0000_0000_0001),
            f64::from_bits(0x4330_0000_0000_0000),
        ),
    ];
    for (opcode, left, right) in binary {
        let product = product_binary(opcode, left, right);
        let reference = reference_binary(opcode, left, right);
        assert_eq!(
            product.to_bits(),
            reference.to_bits(),
            "{opcode:?}({left:?}, {right:?}): product={:016x}, reference={:016x}",
            product.to_bits(),
            reference.to_bits()
        );
    }

    for (y, x) in [(0.0, -1.0), (-0.0, -1.0), (1.0, 0.0), (-1.0, 0.0)] {
        assert_eq!(
            product_binary(CanonicalExpressionOpcode::Atan2, y, x).to_bits(),
            reference_atan2(y, x).to_bits()
        );
    }
}

#[test]
fn independent_easing_formulas_match_every_core_family() {
    let low = f64::from_bits(0x3fb9_9999_9999_999a);
    let midpoint_left = f64::from_bits(0x3fdf_ffff_ffff_ffff);
    let high = f64::from_bits(0x3fe6_6666_6666_6666);
    let cases = [
        (EasingId::Linear, midpoint_left),
        (EasingId::EaseInSine, low),
        (EasingId::EaseOutSine, high),
        (EasingId::EaseInOutSine, midpoint_left),
        (EasingId::EaseInQuad, high),
        (EasingId::EaseInCubic, high),
        (EasingId::EaseInQuart, high),
        (EasingId::EaseInQuint, high),
        (EasingId::EaseInExpo, low),
        (EasingId::EaseOutExpo, high),
        (EasingId::EaseInOutExpo, midpoint_left),
        (EasingId::EaseInCirc, high),
        (EasingId::EaseOutCirc, low),
        (EasingId::EaseInOutCirc, midpoint_left),
        (EasingId::EaseInBack, high),
        (EasingId::EaseInElastic, low),
        (EasingId::EaseOutElastic, high),
        (EasingId::EaseInOutElastic, midpoint_left),
        (EasingId::EaseInBounce, low),
        (EasingId::EaseOutBounce, high),
        (EasingId::EaseInOutBounce, midpoint_left),
    ];
    for (easing, input) in cases {
        let product = evaluate_easing(easing.abi_id(), input).unwrap();
        let reference = reference_easing(easing, input);
        assert_eq!(
            product.to_bits(),
            reference.to_bits(),
            "{}({input:?}): product={:016x}, reference={:016x}",
            easing.name(),
            product.to_bits(),
            reference.to_bits()
        );
    }
}

#[test]
fn independent_scroll_integrals_match_linear_easing_and_bezier_tracks() {
    let cases: [(CanonicalTrackInterpolation, f64); 3] = [
        (CanonicalTrackInterpolation::Linear, 4.0),
        (
            CanonicalTrackInterpolation::Easing("easeInQuad".to_owned()),
            8.0 / 3.0,
        ),
        (
            CanonicalTrackInterpolation::CubicBezier([1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0]),
            4.0,
        ),
    ];
    for (interpolation, expected) in cases {
        let product = varying_scroll_product(interpolation.clone(), 2.0);
        assert_eq!(
            product.to_bits(),
            expected.to_bits(),
            "{interpolation:?}: product={product:?}, reference={expected:?}"
        );
    }
}

#[test]
fn independent_bezier_vectors_cover_flat_x_and_overshooting_y() {
    let vectors: [([f64; 4], f64, f64); 3] = [
        ([0.0, 2.0, 0.0, -1.0], 0.125, 0.5),
        ([0.5, 2.0, 0.5, 2.0], 0.5, 1.625),
        ([0.0, 0.0, 1.0, 1.0], 0.25, 0.25),
    ];
    for (controls, progress, reference) in vectors {
        assert_eq!(
            bezier_product(controls, progress).to_bits(),
            reference.to_bits(),
            "controls={controls:?}, progress={progress}"
        );
    }
}

#[test]
fn numeric_vector_toml_is_strict_and_executable() {
    execute_vectors(NUMERIC_VECTORS).unwrap();

    assert!(
        execute_vectors(&NUMERIC_VECTORS.replace(
            "schema_version = 1",
            "schema_version = 1\nschema_version = 1"
        ))
        .is_err()
    );
    assert!(
        execute_vectors(&NUMERIC_VECTORS.replace(
            "rounding = \"binary64-roundTiesToEven\"",
            "rounding = \"binary64-roundTiesToEven\"\nunknown = true"
        ))
        .is_err()
    );
    assert!(execute_vectors(&NUMERIC_VECTORS.replace("sqrt(4.0)", "sqrt(9.0)")).is_err());
    assert!(
        execute_vectors(
            &NUMERIC_VECTORS.replace("hex_bits = \"4000000000000000\"", "hex_bits = \"4000\"")
        )
        .is_err()
    );
    let duplicate_easing = format!(
        "{NUMERIC_VECTORS}\n[[easing]]\nname = \"linear\"\nx_hex_bits = \"3fe0000000000000\"\ny_hex_bits = \"3fe0000000000000\"\n"
    );
    assert!(execute_vectors(&duplicate_easing).is_err());
}

#[test]
fn plain_matrix_reference_matches_non_uniform_inherited_transform() {
    let mut registry = StableIdRegistry::new();
    let parent_id = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("matrix-parent").unwrap(),
        )
        .unwrap();
    let child_id = registry
        .insert(
            EntityKind::Line,
            CanonicalTextualId::explicit("matrix-child").unwrap(),
        )
        .unwrap();
    let vec2 = |x, y| CanonicalVec2::new(x, y).unwrap();
    let base = |position, rotation, scale| {
        CanonicalLineBase::new(
            position,
            rotation,
            scale,
            1.0,
            vec2(0.0, 0.0),
            vec2(0.5, 0.5),
            120.0,
            0.0,
            0.0,
            false,
            0,
        )
        .unwrap()
    };
    let parent_rotation = 0.375;
    let child_rotation = -0.625;
    let graph = CanonicalLineGraph::new([
        CanonicalLine::new(
            parent_id.clone(),
            None,
            0,
            base(vec2(3.0, -5.0), parent_rotation, vec2(2.0, 0.5)),
            CanonicalLineInherit::default(),
            CanonicalScrollTempo::Global,
        )
        .unwrap(),
        CanonicalLine::new(
            child_id.clone(),
            Some(parent_id),
            1,
            base(vec2(-7.0, 11.0), child_rotation, vec2(0.25, 4.0)),
            CanonicalLineInherit::new(true, true, true, true, true),
            CanonicalScrollTempo::Global,
        )
        .unwrap(),
    ])
    .unwrap();
    let product = evaluate_line_transform(
        &graph,
        &CanonicalTrackSet::new(Vec::new()).unwrap(),
        &child_id,
        0.0,
    )
    .unwrap();

    let matrix = |position: CanonicalVec2, rotation: f64, scale: CanonicalVec2| {
        let sin = reference_unary(CanonicalExpressionOpcode::Sin, rotation);
        let cos = reference_unary(CanonicalExpressionOpcode::Cos, rotation);
        [
            [cos * scale.x(), -sin * scale.y(), position.x()],
            [sin * scale.x(), cos * scale.y(), position.y()],
            [0.0, 0.0, 1.0],
        ]
    };
    let multiply = |left: [[f64; 3]; 3], right: [[f64; 3]; 3]| {
        let mut result = [[0.0; 3]; 3];
        for row in 0..3 {
            for column in 0..3 {
                result[row][column] = (left[row][0] * right[0][column]
                    + left[row][1] * right[1][column])
                    + left[row][2] * right[2][column];
            }
        }
        result
    };
    let reference = multiply(
        matrix(vec2(3.0, -5.0), parent_rotation, vec2(2.0, 0.5)),
        matrix(vec2(-7.0, 11.0), child_rotation, vec2(0.25, 4.0)),
    );
    let product = product.world_matrix().rows();
    for row in 0..3 {
        for column in 0..3 {
            assert_eq!(
                product[row][column].to_bits(),
                reference[row][column].to_bits(),
                "matrix[{row}][{column}]"
            );
        }
    }
}
