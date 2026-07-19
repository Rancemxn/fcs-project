use std::fmt;

/// The stable Core easing IDs shared by FCS and Execution ABI 1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
pub enum EasingId {
    Linear = 0,
    EaseInSine = 1,
    EaseOutSine = 2,
    EaseInOutSine = 3,
    EaseInQuad = 4,
    EaseOutQuad = 5,
    EaseInOutQuad = 6,
    EaseInCubic = 7,
    EaseOutCubic = 8,
    EaseInOutCubic = 9,
    EaseInQuart = 10,
    EaseOutQuart = 11,
    EaseInOutQuart = 12,
    EaseInQuint = 13,
    EaseOutQuint = 14,
    EaseInOutQuint = 15,
    EaseInExpo = 16,
    EaseOutExpo = 17,
    EaseInOutExpo = 18,
    EaseInCirc = 19,
    EaseOutCirc = 20,
    EaseInOutCirc = 21,
    EaseInBack = 22,
    EaseOutBack = 23,
    EaseInOutBack = 24,
    EaseInElastic = 25,
    EaseOutElastic = 26,
    EaseInOutElastic = 27,
    EaseInBounce = 28,
    EaseOutBounce = 29,
    EaseInOutBounce = 30,
}

impl EasingId {
    pub const ALL: [Self; 31] = [
        Self::Linear,
        Self::EaseInSine,
        Self::EaseOutSine,
        Self::EaseInOutSine,
        Self::EaseInQuad,
        Self::EaseOutQuad,
        Self::EaseInOutQuad,
        Self::EaseInCubic,
        Self::EaseOutCubic,
        Self::EaseInOutCubic,
        Self::EaseInQuart,
        Self::EaseOutQuart,
        Self::EaseInOutQuart,
        Self::EaseInQuint,
        Self::EaseOutQuint,
        Self::EaseInOutQuint,
        Self::EaseInExpo,
        Self::EaseOutExpo,
        Self::EaseInOutExpo,
        Self::EaseInCirc,
        Self::EaseOutCirc,
        Self::EaseInOutCirc,
        Self::EaseInBack,
        Self::EaseOutBack,
        Self::EaseInOutBack,
        Self::EaseInElastic,
        Self::EaseOutElastic,
        Self::EaseInOutElastic,
        Self::EaseInBounce,
        Self::EaseOutBounce,
        Self::EaseInOutBounce,
    ];

    pub const fn abi_id(self) -> u16 {
        self as u16
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Linear => "linear",
            Self::EaseInSine => "easeInSine",
            Self::EaseOutSine => "easeOutSine",
            Self::EaseInOutSine => "easeInOutSine",
            Self::EaseInQuad => "easeInQuad",
            Self::EaseOutQuad => "easeOutQuad",
            Self::EaseInOutQuad => "easeInOutQuad",
            Self::EaseInCubic => "easeInCubic",
            Self::EaseOutCubic => "easeOutCubic",
            Self::EaseInOutCubic => "easeInOutCubic",
            Self::EaseInQuart => "easeInQuart",
            Self::EaseOutQuart => "easeOutQuart",
            Self::EaseInOutQuart => "easeInOutQuart",
            Self::EaseInQuint => "easeInQuint",
            Self::EaseOutQuint => "easeOutQuint",
            Self::EaseInOutQuint => "easeInOutQuint",
            Self::EaseInExpo => "easeInExpo",
            Self::EaseOutExpo => "easeOutExpo",
            Self::EaseInOutExpo => "easeInOutExpo",
            Self::EaseInCirc => "easeInCirc",
            Self::EaseOutCirc => "easeOutCirc",
            Self::EaseInOutCirc => "easeInOutCirc",
            Self::EaseInBack => "easeInBack",
            Self::EaseOutBack => "easeOutBack",
            Self::EaseInOutBack => "easeInOutBack",
            Self::EaseInElastic => "easeInElastic",
            Self::EaseOutElastic => "easeOutElastic",
            Self::EaseInOutElastic => "easeInOutElastic",
            Self::EaseInBounce => "easeInBounce",
            Self::EaseOutBounce => "easeOutBounce",
            Self::EaseInOutBounce => "easeInOutBounce",
        }
    }

    pub fn evaluate(self, input: f64) -> Result<f64, EasingError> {
        if !input.is_finite() {
            return Err(EasingError::NonFiniteInput);
        }
        if !(0.0..=1.0).contains(&input) {
            return Err(EasingError::InputOutOfRange);
        }
        if input == 0.0 {
            return Ok(0.0);
        }
        if input == 1.0 {
            return Ok(1.0);
        }

        let output = match self {
            Self::Linear => input,
            Self::EaseInSine => ease_in(Family::Sine, input),
            Self::EaseOutSine => ease_out(Family::Sine, input),
            Self::EaseInOutSine => ease_in_out(Family::Sine, input),
            Self::EaseInQuad => ease_in(Family::Quad, input),
            Self::EaseOutQuad => ease_out(Family::Quad, input),
            Self::EaseInOutQuad => ease_in_out(Family::Quad, input),
            Self::EaseInCubic => ease_in(Family::Cubic, input),
            Self::EaseOutCubic => ease_out(Family::Cubic, input),
            Self::EaseInOutCubic => ease_in_out(Family::Cubic, input),
            Self::EaseInQuart => ease_in(Family::Quart, input),
            Self::EaseOutQuart => ease_out(Family::Quart, input),
            Self::EaseInOutQuart => ease_in_out(Family::Quart, input),
            Self::EaseInQuint => ease_in(Family::Quint, input),
            Self::EaseOutQuint => ease_out(Family::Quint, input),
            Self::EaseInOutQuint => ease_in_out(Family::Quint, input),
            Self::EaseInExpo => ease_in(Family::Expo, input),
            Self::EaseOutExpo => ease_out(Family::Expo, input),
            Self::EaseInOutExpo => ease_in_out(Family::Expo, input),
            Self::EaseInCirc => ease_in(Family::Circ, input),
            Self::EaseOutCirc => ease_out(Family::Circ, input),
            Self::EaseInOutCirc => ease_in_out(Family::Circ, input),
            Self::EaseInBack => ease_in(Family::Back, input),
            Self::EaseOutBack => ease_out(Family::Back, input),
            Self::EaseInOutBack => ease_in_out(Family::Back, input),
            Self::EaseInElastic => ease_in(Family::Elastic, input),
            Self::EaseOutElastic => ease_out(Family::Elastic, input),
            Self::EaseInOutElastic => ease_in_out(Family::Elastic, input),
            Self::EaseInBounce => ease_in(Family::Bounce, input),
            Self::EaseOutBounce => ease_out(Family::Bounce, input),
            Self::EaseInOutBounce => ease_in_out(Family::Bounce, input),
        };

        output
            .is_finite()
            .then_some(output)
            .ok_or(EasingError::NonFiniteResult)
    }
}

impl TryFrom<u16> for EasingId {
    type Error = EasingError;

    fn try_from(id: u16) -> Result<Self, Self::Error> {
        Self::ALL
            .get(usize::from(id))
            .copied()
            .ok_or(EasingError::UnknownId { id })
    }
}

/// Evaluates a stable FCS/ABI easing ID at a finite progress value in `[0, 1]`.
pub fn evaluate_easing(id: u16, input: f64) -> Result<f64, EasingError> {
    EasingId::try_from(id)?.evaluate(input)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingError {
    UnknownId { id: u16 },
    NonFiniteInput,
    InputOutOfRange,
    NonFiniteResult,
}

impl fmt::Display for EasingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownId { id } => write!(formatter, "unknown Core easing ID {id}"),
            Self::NonFiniteInput => formatter.write_str("easing input must be finite"),
            Self::InputOutOfRange => formatter.write_str("easing input must be in [0, 1]"),
            Self::NonFiniteResult => formatter.write_str("easing result must be finite"),
        }
    }
}

impl std::error::Error for EasingError {}

#[derive(Debug, Clone, Copy)]
enum Family {
    Sine,
    Quad,
    Cubic,
    Quart,
    Quint,
    Expo,
    Circ,
    Back,
    Elastic,
    Bounce,
}

fn ease_in(family: Family, input: f64) -> f64 {
    if input == 0.0 {
        return 0.0;
    }
    if input == 1.0 {
        return 1.0;
    }

    match family {
        Family::Sine => 1.0 - math::cos((std::f64::consts::PI * input) / 2.0),
        Family::Quad => input * input,
        Family::Cubic => (input * input) * input,
        Family::Quart => ((input * input) * input) * input,
        Family::Quint => (((input * input) * input) * input) * input,
        Family::Expo => {
            if input == 0.0 {
                0.0
            } else {
                math::pow(2.0, (10.0 * input) - 10.0)
            }
        }
        Family::Circ => 1.0 - math::sqrt(1.0 - (input * input)),
        Family::Back => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            (c3 * ((input * input) * input)) - (c1 * (input * input))
        }
        Family::Elastic => {
            if input == 0.0 {
                0.0
            } else if input == 1.0 {
                1.0
            } else {
                let exponent = (10.0 * input) - 10.0;
                let angle = ((((10.0 * input) - 10.75) * 2.0) * std::f64::consts::PI) / 3.0;
                -math::pow(2.0, exponent) * math::sin(angle)
            }
        }
        Family::Bounce => 1.0 - out_bounce(1.0 - input),
    }
}

fn ease_out(family: Family, input: f64) -> f64 {
    1.0 - ease_in(family, 1.0 - input)
}

fn ease_in_out(family: Family, input: f64) -> f64 {
    if input < 0.5 {
        ease_in(family, 2.0 * input) / 2.0
    } else {
        1.0 - (ease_in(family, 2.0 - (2.0 * input)) / 2.0)
    }
}

fn out_bounce(input: f64) -> f64 {
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

// I4.8 replaces or cross-checks this isolated platform math bridge with the
// correctly rounded independent reference path required by FCS section 14.1.
mod math {
    pub(super) fn sin(value: f64) -> f64 {
        value.sin()
    }

    pub(super) fn cos(value: f64) -> f64 {
        value.cos()
    }

    pub(super) fn sqrt(value: f64) -> f64 {
        value.sqrt()
    }

    pub(super) fn pow(base: f64, exponent: f64) -> f64 {
        base.powf(exponent)
    }
}

#[cfg(test)]
mod tests {
    use super::{EasingError, EasingId, evaluate_easing};
    use std::collections::BTreeSet;

    #[test]
    fn all_ids_round_trip_with_unique_canonical_names() {
        let expected_names = [
            "linear",
            "easeInSine",
            "easeOutSine",
            "easeInOutSine",
            "easeInQuad",
            "easeOutQuad",
            "easeInOutQuad",
            "easeInCubic",
            "easeOutCubic",
            "easeInOutCubic",
            "easeInQuart",
            "easeOutQuart",
            "easeInOutQuart",
            "easeInQuint",
            "easeOutQuint",
            "easeInOutQuint",
            "easeInExpo",
            "easeOutExpo",
            "easeInOutExpo",
            "easeInCirc",
            "easeOutCirc",
            "easeInOutCirc",
            "easeInBack",
            "easeOutBack",
            "easeInOutBack",
            "easeInElastic",
            "easeOutElastic",
            "easeInOutElastic",
            "easeInBounce",
            "easeOutBounce",
            "easeInOutBounce",
        ];
        let names = EasingId::ALL
            .iter()
            .map(|easing| easing.name())
            .collect::<BTreeSet<_>>();
        assert_eq!(names.len(), 31);

        for (expected_id, easing) in EasingId::ALL.iter().copied().enumerate() {
            assert_eq!(usize::from(easing.abi_id()), expected_id);
            assert_eq!(easing.name(), expected_names[expected_id]);
            assert_eq!(EasingId::try_from(easing.abi_id()), Ok(easing));
        }
        assert_eq!(
            EasingId::try_from(31),
            Err(EasingError::UnknownId { id: 31 })
        );
    }

    #[test]
    fn every_easing_pins_binary64_endpoints() {
        for easing in EasingId::ALL {
            for input in [0.0, -0.0] {
                assert_eq!(easing.evaluate(input).unwrap().to_bits(), 0.0_f64.to_bits());
            }
            assert_eq!(easing.evaluate(1.0).unwrap().to_bits(), 1.0_f64.to_bits());
        }
    }

    #[test]
    fn algebraic_midpoint_vectors_are_bit_exact() {
        let vectors = [
            (EasingId::Linear, 0.5_f64),
            (EasingId::EaseInQuad, 0.25),
            (EasingId::EaseOutQuad, 0.75),
            (EasingId::EaseInOutQuad, 0.5),
            (EasingId::EaseInCubic, 0.125),
            (EasingId::EaseOutCubic, 0.875),
            (EasingId::EaseInOutCubic, 0.5),
            (EasingId::EaseInQuart, 0.0625),
            (EasingId::EaseOutQuart, 0.9375),
            (EasingId::EaseInOutQuart, 0.5),
            (EasingId::EaseInQuint, 0.03125),
            (EasingId::EaseOutQuint, 0.96875),
            (EasingId::EaseInOutQuint, 0.5),
            (EasingId::EaseInExpo, 0.03125),
            (EasingId::EaseOutExpo, 0.96875),
            (EasingId::EaseInOutExpo, 0.5),
        ];
        for (easing, expected) in vectors {
            assert_eq!(
                easing.evaluate(0.5).unwrap().to_bits(),
                expected.to_bits(),
                "{}",
                easing.name()
            );
        }
    }

    #[test]
    fn every_family_obeys_the_normative_out_and_in_out_transforms() {
        for family_start in (1..=28).step_by(3) {
            let ease_in = EasingId::try_from(family_start).unwrap();
            let ease_out = EasingId::try_from(family_start + 1).unwrap();
            let ease_in_out = EasingId::try_from(family_start + 2).unwrap();
            for input in [0.125, 0.25, 0.625, 0.875] {
                let reflected = 1.0 - ease_in.evaluate(1.0 - input).unwrap();
                assert_eq!(
                    ease_out.evaluate(input).unwrap().to_bits(),
                    reflected.to_bits()
                );

                let transformed = if input < 0.5 {
                    ease_in.evaluate(2.0 * input).unwrap() / 2.0
                } else {
                    1.0 - (ease_in.evaluate(2.0 - (2.0 * input)).unwrap() / 2.0)
                };
                assert_eq!(
                    ease_in_out.evaluate(input).unwrap().to_bits(),
                    transformed.to_bits()
                );
            }
        }
    }

    #[test]
    fn transformed_progress_reapplies_endpoint_pinning_after_rounding() {
        let smallest_positive = f64::from_bits(1);
        assert_eq!(1.0 - smallest_positive, 1.0);

        for family_start in (1..=28).step_by(3) {
            let ease_out = EasingId::try_from(family_start + 1).unwrap();
            assert_eq!(
                ease_out.evaluate(smallest_positive).unwrap().to_bits(),
                0.0_f64.to_bits(),
                "{}",
                ease_out.name()
            );
        }
    }

    #[test]
    fn overshoot_and_bounce_behavior_remains_visible() {
        assert!(EasingId::EaseInBack.evaluate(0.5).unwrap() < 0.0);
        assert!(EasingId::EaseOutBack.evaluate(0.5).unwrap() > 1.0);
        assert!(EasingId::EaseInElastic.evaluate(0.5).unwrap() < 0.0);
        assert!(EasingId::EaseOutElastic.evaluate(0.5).unwrap() > 1.0);

        let before_rebound = EasingId::EaseOutBounce.evaluate(0.75).unwrap();
        let rebound = EasingId::EaseOutBounce.evaluate(0.8).unwrap();
        assert!(before_rebound > rebound);
        for easing in [
            EasingId::EaseInBounce,
            EasingId::EaseOutBounce,
            EasingId::EaseInOutBounce,
        ] {
            for step in 0..=64 {
                let output = easing.evaluate(f64::from(step) / 64.0).unwrap();
                assert!((0.0..=1.0).contains(&output));
            }
        }
    }

    #[test]
    fn every_supported_id_has_a_finite_midpoint_and_dense_sample() {
        for easing in EasingId::ALL {
            assert!(easing.evaluate(0.5).unwrap().is_finite());
            for step in 0..=64 {
                let output = easing.evaluate(f64::from(step) / 64.0).unwrap();
                assert!(output.is_finite(), "{} at step {step}", easing.name());
            }
        }
    }

    #[test]
    fn invalid_id_domain_and_non_finite_inputs_are_rejected() {
        assert_eq!(
            evaluate_easing(31, 0.5),
            Err(EasingError::UnknownId { id: 31 })
        );
        for input in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                EasingId::Linear.evaluate(input),
                Err(EasingError::NonFiniteInput)
            );
        }
        for input in [-f64::MIN_POSITIVE, 1.0 + f64::EPSILON] {
            assert_eq!(
                EasingId::Linear.evaluate(input),
                Err(EasingError::InputOutOfRange)
            );
        }
    }
}
