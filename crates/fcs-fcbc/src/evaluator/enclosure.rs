//! Outward bounds and cubic Taylor models for direct Distance integration.
//!
//! The polynomial variable ranges over [-1, 1]. Every discarded coefficient
//! and every binary64 node rounding is retained in the remainder. Integrating
//! the polynomial and remainder therefore encloses the whole interval, including
//! behavior between sample points; it is not a quadrature error estimate.

use astro_float::{BigFloat, Consts, Radix, RoundingMode};
use std::ops::{Add, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Bounds {
    pub lo: f64,
    pub hi: f64,
}

impl Bounds {
    pub const ZERO: Self = Self::point(0.0);
    pub const UNIT: Self = Self { lo: -1.0, hi: 1.0 };
    pub const POSITIVE_UNIT: Self = Self { lo: 0.0, hi: 1.0 };

    pub const fn point(value: f64) -> Self {
        Self {
            lo: value,
            hi: value,
        }
    }

    pub fn finite(self) -> Option<Self> {
        (self.lo.is_finite() && self.hi.is_finite() && self.lo <= self.hi).then_some(self)
    }

    pub fn hull(self, other: Self) -> Self {
        Self {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }

    pub fn intersect(self, other: Self) -> Self {
        Self {
            lo: self.lo.max(other.lo),
            hi: self.hi.min(other.hi),
        }
    }

    pub fn magnitude(self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }

    pub fn midpoint(self) -> f64 {
        self.lo.midpoint(self.hi)
    }

    pub fn contains(self, value: f64) -> bool {
        self.lo <= value && value <= self.hi
    }

    pub fn rounding_error(self) -> Self {
        let maximum = self.magnitude();
        // One ULP also covers subnormal roundings. Infinite/NaN bounds are
        // rejected by the caller instead of being mistaken for certification.
        let error = maximum.next_up() - maximum;
        Self {
            lo: -error,
            hi: error,
        }
    }

    pub fn square(self) -> Self {
        let lo = if self.contains(0.0) {
            0.0
        } else {
            (self.lo * self.lo).min(self.hi * self.hi).next_down()
        };
        Self {
            lo,
            hi: (self.lo * self.lo).max(self.hi * self.hi).next_up(),
        }
    }

    fn float_binary(self, other: Self, apply: impl Fn(f64, f64) -> f64) -> Self {
        let mut values = [
            apply(self.lo, other.lo),
            apply(self.lo, other.hi),
            apply(self.hi, other.lo),
            apply(self.hi, other.hi),
        ];
        if values.iter().any(|value| value.is_nan()) {
            return Self {
                lo: f64::NEG_INFINITY,
                hi: f64::INFINITY,
            };
        }
        values.sort_by(f64::total_cmp);
        Self {
            lo: values[0],
            hi: values[3],
        }
    }
}

impl Add for Bounds {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        if self == Self::ZERO {
            return other;
        }
        if other == Self::ZERO {
            return self;
        }
        Self {
            lo: (self.lo + other.lo).next_down(),
            hi: (self.hi + other.hi).next_up(),
        }
    }
}

impl Neg for Bounds {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
}

impl Sub for Bounds {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

impl Mul for Bounds {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        if self == Self::ZERO || other == Self::ZERO {
            return Self::ZERO;
        }
        let products = [
            self.lo * other.lo,
            self.lo * other.hi,
            self.hi * other.lo,
            self.hi * other.hi,
        ];
        if products.iter().any(|value| value.is_nan()) {
            return Self {
                lo: f64::NEG_INFINITY,
                hi: f64::INFINITY,
            };
        }
        Self {
            lo: products
                .into_iter()
                .fold(f64::INFINITY, f64::min)
                .next_down(),
            hi: products
                .into_iter()
                .fold(f64::NEG_INFINITY, f64::max)
                .next_up(),
        }
    }
}

impl Div for Bounds {
    type Output = Self;
    fn div(self, other: Self) -> Self {
        if other.contains(0.0) {
            return Self {
                lo: f64::NEG_INFINITY,
                hi: f64::INFINITY,
            };
        }
        self * Self {
            lo: (1.0 / other.hi).next_down(),
            hi: (1.0 / other.lo).next_up(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Taylor {
    coefficients: [Bounds; 4],
    remainder: Bounds,
    bound: Bounds,
}

impl Taylor {
    pub fn constant(value: f64) -> Self {
        Self::enclosed(Bounds::point(value))
    }

    pub fn enclosed(value: Bounds) -> Self {
        Self {
            coefficients: [value, Bounds::ZERO, Bounds::ZERO, Bounds::ZERO],
            remainder: Bounds::ZERO,
            bound: value,
        }
    }

    pub fn variable(start: f64, end: f64) -> Self {
        let half = Bounds::point(0.5);
        Self {
            coefficients: [
                (Bounds::point(start) + Bounds::point(end)) * half,
                (Bounds::point(end) - Bounds::point(start)) * half,
                Bounds::ZERO,
                Bounds::ZERO,
            ],
            remainder: Bounds::ZERO,
            bound: Bounds { lo: start, hi: end },
        }
    }

    pub fn exact_constant(self) -> Option<f64> {
        (self.bound.lo.to_bits() == self.bound.hi.to_bits()).then_some(self.bound.lo)
    }

    fn polynomial_range(self) -> Bounds {
        self.coefficients
            .iter()
            .enumerate()
            .fold(Bounds::ZERO, |sum, (degree, value)| {
                sum + *value
                    * if degree == 0 {
                        Bounds::point(1.0)
                    } else if degree % 2 == 0 {
                        Bounds::POSITIVE_UNIT
                    } else {
                        Bounds::UNIT
                    }
            })
    }

    pub fn range(self) -> Bounds {
        (self.polynomial_range() + self.remainder).intersect(self.bound)
    }

    pub fn with_bound(mut self, bound: Bounds) -> Self {
        self.bound = self.bound.intersect(bound);
        self
    }

    pub fn with_remainder(mut self, remainder: Bounds) -> Self {
        self.remainder = self.remainder + remainder;
        self.bound = self.bound + remainder;
        self
    }

    pub fn real_add(self, other: Self) -> Self {
        Self {
            coefficients: std::array::from_fn(|i| self.coefficients[i] + other.coefficients[i]),
            remainder: self.remainder + other.remainder,
            bound: self.range() + other.range(),
        }
    }

    pub fn real_mul(self, other: Self) -> Self {
        let mut product = [Bounds::ZERO; 7];
        for (i, left) in self.coefficients.iter().enumerate() {
            for (j, right) in other.coefficients.iter().enumerate() {
                product[i + j] = product[i + j] + *left * *right;
            }
        }
        let mut remainder = self.remainder * other.polynomial_range()
            + other.remainder * self.polynomial_range()
            + self.remainder * other.remainder;
        for (degree, value) in product.iter().enumerate().skip(4) {
            remainder = remainder
                + *value
                    * if degree % 2 == 0 {
                        Bounds::POSITIVE_UNIT
                    } else {
                        Bounds::UNIT
                    };
        }
        Self {
            coefficients: [product[0], product[1], product[2], product[3]],
            remainder,
            bound: self.range() * other.range(),
        }
    }

    pub fn scale(self, value: Bounds) -> Self {
        self.real_mul(Self::enclosed(value))
    }

    pub fn rounded(self) -> Self {
        self.with_remainder(self.range().rounding_error())
    }

    /// Encloses f(self) using derivatives at center and a fourth-derivative
    /// bound over the complete argument range (Taylor's theorem).
    pub fn compose(self, center: f64, coefficients: [Bounds; 4], fourth: Bounds) -> Self {
        let delta = self.real_add(Self::constant(-center));
        let polynomial = coefficients
            .into_iter()
            .rev()
            .fold(Self::constant(0.0), |result, coefficient| {
                result.real_mul(delta).real_add(Self::enclosed(coefficient))
            });
        polynomial.with_remainder(fourth * delta.range().square().square() / Bounds::point(24.0))
    }

    pub fn reciprocal(self) -> Option<Self> {
        let range = self.range().finite()?;
        if range.contains(0.0) {
            return None;
        }
        let center = range.midpoint();
        let inverse = Bounds::point(1.0) / Bounds::point(center);
        let square = inverse * inverse;
        let fourth = square * square;
        let whole_inverse = Bounds::point(1.0) / range;
        Some(self.compose(
            center,
            [inverse, -square, square * inverse, -fourth],
            Bounds::point(24.0)
                * whole_inverse
                * whole_inverse
                * whole_inverse
                * whole_inverse
                * whole_inverse,
        ))
    }

    pub fn divide(self, other: Self) -> Option<Self> {
        if let (Some(left), Some(right)) = (self.exact_constant(), other.exact_constant()) {
            return (right != 0.0).then(|| Self::constant(left / right));
        }
        let bound = self
            .range()
            .float_binary(other.range(), |left, right| left / right);
        Some(
            self.real_mul(other.reciprocal()?)
                .rounded()
                .with_bound(bound),
        )
    }

    pub fn integral(self, start: f64, end: f64) -> Bounds {
        (self.coefficients[0] + self.coefficients[2] / Bounds::point(3.0) + self.remainder)
            * (Bounds::point(end) - Bounds::point(start))
    }

    /// Integral from the binary64 midpoint to each time in this panel.
    pub fn primitive(self, start: f64, end: f64) -> Self {
        let half = Bounds::point(0.5);
        let width = (Bounds::point(end) - Bounds::point(start)) * half;
        let center = Bounds::point(start.midpoint(end));
        let center_error = (Bounds::point(start) + Bounds::point(end)) * half - center;
        let remainder = width * self.coefficients[3] / Bounds::point(4.0) * Bounds::POSITIVE_UNIT
            + width * self.remainder * Bounds::UNIT
            + center_error * self.range();
        Self {
            coefficients: [
                Bounds::ZERO,
                width * self.coefficients[0],
                width * self.coefficients[1] / Bounds::point(2.0),
                width * self.coefficients[2] / Bounds::point(3.0),
            ],
            remainder,
            bound: (Bounds { lo: start, hi: end } - center) * self.range(),
        }
    }
}

impl Neg for Taylor {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            coefficients: self.coefficients.map(|value| -value),
            remainder: -self.remainder,
            bound: -self.bound,
        }
    }
}

impl Add for Taylor {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        if let (Some(left), Some(right)) = (self.exact_constant(), other.exact_constant()) {
            return Self::constant(left + right);
        }
        let bound = self
            .range()
            .float_binary(other.range(), |left, right| left + right);
        self.real_add(other).rounded().with_bound(bound)
    }
}

impl Sub for Taylor {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        self + -other
    }
}

impl Mul for Taylor {
    type Output = Self;
    fn mul(self, other: Self) -> Self {
        if let (Some(left), Some(right)) = (self.exact_constant(), other.exact_constant()) {
            return Self::constant(left * right);
        }
        let bound = self
            .range()
            .float_binary(other.range(), |left, right| left * right);
        self.real_mul(other).rounded().with_bound(bound)
    }
}

pub(super) struct Math {
    pub constants: Consts,
}

impl Math {
    pub fn new() -> Option<Self> {
        Some(Self {
            constants: Consts::new().ok()?,
        })
    }

    pub fn float_round(&mut self, value: &BigFloat) -> Option<f64> {
        let rounded = value
            .format(Radix::Dec, RoundingMode::None, &mut self.constants)
            .ok()?
            .parse::<f64>()
            .ok()?;
        rounded.is_finite().then_some(rounded)
    }

    pub fn float_bound(&mut self, value: &BigFloat, lower: bool) -> Option<f64> {
        let rounded = self.float_round(value)?;
        let ordering = BigFloat::from_f64(rounded, 64).cmp(value)?;
        Some(if lower && ordering > 0 {
            rounded.next_down()
        } else if !lower && ordering < 0 {
            rounded.next_up()
        } else {
            rounded
        })
    }

    fn point(&mut self, opcode: u16, input: f64) -> Option<Bounds> {
        let mut values = [0.0; 2];
        for (index, rounding) in [RoundingMode::Down, RoundingMode::Up]
            .into_iter()
            .enumerate()
        {
            let input = BigFloat::from_f64(input, 192);
            let value = match opcode {
                47 => input.sqrt(192, rounding),
                48 => input.exp(192, rounding, &mut self.constants),
                49 => input.ln(192, rounding, &mut self.constants),
                50 => input.sin(192, rounding, &mut self.constants),
                51 => input.cos(192, rounding, &mut self.constants),
                52 => input.tan(192, rounding, &mut self.constants),
                53 => input.asin(192, rounding, &mut self.constants),
                54 => input.acos(192, rounding, &mut self.constants),
                55 => input.atan(192, rounding, &mut self.constants),
                _ => return None,
            };
            values[index] = self.float_bound(&value, index == 0)?;
        }
        Bounds {
            lo: values[0],
            hi: values[1],
        }
        .finite()
    }

    fn unary_range(&mut self, opcode: u16, input: Bounds) -> Option<Bounds> {
        if matches!(opcode, 50 | 51) {
            return Some(Bounds::UNIT);
        }
        if opcode == 52 {
            return None;
        }
        if (opcode == 47 && input.lo < 0.0)
            || (opcode == 49 && input.lo <= 0.0)
            || (matches!(opcode, 53 | 54) && (input.lo < -1.0 || input.hi > 1.0))
        {
            return None;
        }
        let left = self.point(opcode, input.lo)?;
        let right = self.point(opcode, input.hi)?;
        Some(left.hull(right))
    }

    fn derivatives(&mut self, opcode: u16, x: Bounds) -> Option<[Bounds; 4]> {
        let one = Bounds::point(1.0);
        let square = x.square();
        let result = match opcode {
            47 => {
                if x.lo <= 0.0 {
                    return None;
                }
                let root = self.unary_range(47, x)?;
                [
                    one / (Bounds::point(2.0) * root),
                    -one / (Bounds::point(4.0) * x * root),
                    Bounds::point(3.0) / (Bounds::point(8.0) * square * root),
                    Bounds::point(-15.0) / (Bounds::point(16.0) * square * x * root),
                ]
            }
            48 => [self.unary_range(48, x)?; 4],
            49 => {
                if x.lo <= 0.0 {
                    return None;
                }
                [
                    one / x,
                    -one / square,
                    Bounds::point(2.0) / (square * x),
                    Bounds::point(-6.0) / square.square(),
                ]
            }
            50 | 51 => {
                let (sin, cos) = if x.lo == x.hi {
                    (self.point(50, x.lo)?, self.point(51, x.lo)?)
                } else {
                    (Bounds::UNIT, Bounds::UNIT)
                };
                if opcode == 50 {
                    [cos, -sin, -cos, sin]
                } else {
                    [-sin, -cos, sin, cos]
                }
            }
            53 | 54 => {
                let complement = one - square;
                if complement.lo <= 0.0 {
                    return None;
                }
                let root = self.unary_range(47, complement)?;
                let a = one / root;
                let b = x / (complement * root);
                let c = (one + Bounds::point(2.0) * square) / (complement.square() * root);
                let d = Bounds::point(3.0) * x * (Bounds::point(3.0) + Bounds::point(2.0) * square)
                    / (complement.square() * complement * root);
                if opcode == 53 {
                    [a, b, c, d]
                } else {
                    [-a, -b, -c, -d]
                }
            }
            55 => {
                let denominator = one + square;
                [
                    one / denominator,
                    Bounds::point(-2.0) * x / denominator.square(),
                    (Bounds::point(6.0) * square - Bounds::point(2.0))
                        / (denominator.square() * denominator),
                    Bounds::point(24.0) * x * (one - square) / denominator.square().square(),
                ]
            }
            _ => return None,
        };
        result
            .iter()
            .all(|bound| bound.finite().is_some())
            .then_some(result)
    }

    pub fn unary(&mut self, opcode: u16, value: Taylor) -> Option<Taylor> {
        let range = value.range().finite()?;
        if let Some(input) = value.exact_constant() {
            let output = super::unary_float(
                super::RuntimeValue::Scalar {
                    ty: super::ValueType::Float,
                    value: input,
                },
                opcode,
            )
            .ok()?;
            return Some(Taylor::constant(super::scalar_payload(&output).ok()?));
        }
        if matches!(opcode, 44..=46) {
            let apply = |value: f64| match opcode {
                44 => value.floor(),
                45 => value.ceil(),
                _ => value.round_ties_even(),
            };
            return Some(Taylor::enclosed(Bounds {
                lo: apply(range.lo),
                hi: apply(range.hi),
            }));
        }
        if opcode == 52 {
            let sin = self.unary(50, value)?;
            let cos = self.unary(51, value)?;
            return Some(sin.real_mul(cos.reciprocal()?).rounded());
        }
        let bound = self.unary_range(opcode, range)?;
        let center = range.midpoint();
        let Some(derivatives) = self.derivatives(opcode, range) else {
            return Some(Taylor::enclosed(bound).rounded());
        };
        let at_center = self.derivatives(opcode, Bounds::point(center))?;
        Some(
            value
                .compose(
                    center,
                    [
                        self.point(opcode, center)?,
                        at_center[0],
                        at_center[1] / Bounds::point(2.0),
                        at_center[2] / Bounds::point(6.0),
                    ],
                    derivatives[3],
                )
                .with_bound(bound)
                .rounded(),
        )
    }

    pub fn power(&mut self, base: Taylor, exponent: Taylor) -> Option<Taylor> {
        if let Some(exponent) = exponent.exact_constant()
            && exponent.fract() == 0.0
            && exponent.abs() <= 64.0
        {
            let mut power = exponent.abs() as u32;
            let mut factor = base;
            let mut result = Taylor::constant(1.0);
            while power != 0 {
                if power & 1 != 0 {
                    result = result.real_mul(factor);
                }
                power >>= 1;
                if power != 0 {
                    factor = factor.real_mul(factor);
                }
            }
            if exponent < 0.0 {
                result = result.reciprocal()?;
            }
            return Some(result.rounded());
        }
        let logarithm = if let Some(value) = base.exact_constant() {
            Taylor::enclosed(self.point(49, value)?)
        } else {
            self.unary(49, base)?
        };
        self.unary(48, exponent.real_mul(logarithm))
    }

    pub fn atan2(&mut self, y: Taylor, x: Taylor) -> Option<Taylor> {
        if let (Some(y), Some(x)) = (y.exact_constant(), x.exact_constant()) {
            return Some(Taylor::constant(y.atan2(x)));
        }
        let xr = x.range().finite()?;
        let yr = y.range().finite()?;
        let pi = self.point(54, -1.0)?;
        if xr.lo > 0.0 {
            return self.unary(55, y.real_mul(x.reciprocal()?));
        }
        if yr.lo > 0.0 || yr.hi < 0.0 {
            let angle = self.unary(55, x.real_mul(y.reciprocal()?))?;
            let half_pi = pi / Bounds::point(if yr.lo > 0.0 { 2.0 } else { -2.0 });
            return Some(Taylor::enclosed(half_pi).real_add(-angle).rounded());
        }
        let above_cut = yr.lo > 0.0 || (yr.lo == 0.0 && !yr.lo.is_sign_negative());
        let below_cut = yr.hi < 0.0 || (yr.hi == 0.0 && yr.hi.is_sign_negative());
        if xr.hi < 0.0 && (above_cut || below_cut) {
            let angle = self.unary(55, y.real_mul(x.reciprocal()?))?;
            return Some(
                angle
                    .real_add(Taylor::enclosed(if above_cut { pi } else { -pi }))
                    .rounded(),
            );
        }
        Some(
            Taylor::enclosed(Bounds {
                lo: -pi.hi,
                hi: pi.hi,
            })
            .rounded(),
        )
    }

    pub fn bezier(&mut self, controls: [f64; 4], progress: Taylor) -> Option<Taylor> {
        let range = progress.range().finite()?;
        if range.lo < 0.0 || range.hi > 1.0 {
            return None;
        }
        if let Some(value) = progress.exact_constant() {
            return Some(Taylor::constant(
                fcs_runtime::evaluate_cubic_bezier_progress(controls, value).ok()?,
            ));
        }
        if controls[0] == controls[1] && controls[2] == controls[3] {
            return Some(progress);
        }
        let parameter = |value| {
            let [lo, hi] =
                fcs_runtime::cubic_bezier_parameter_bounds([controls[0], controls[2]], value)
                    .ok()?;
            Some(Bounds { lo, hi })
        };
        let parameters = parameter(range.lo)?.hull(parameter(range.hi)?);
        let polynomial = |first, second| {
            let first = Bounds::point(first);
            let second = Bounds::point(second);
            [
                Bounds::point(3.0) * first,
                Bounds::point(3.0) * second - Bounds::point(6.0) * first,
                Bounds::point(3.0) * first - Bounds::point(3.0) * second + Bounds::point(1.0),
            ]
        };
        let x = polynomial(controls[0], controls[2]);
        let y = polynomial(controls[1], controls[3]);
        let value = |t: Bounds| ((y[2] * t + y[1]) * t + y[0]) * t;
        let derivatives = |t: Bounds| {
            let derivative = |c: [Bounds; 3]| {
                [
                    (Bounds::point(3.0) * c[2] * t + Bounds::point(2.0) * c[1]) * t + c[0],
                    Bounds::point(6.0) * c[2] * t + Bounds::point(2.0) * c[1],
                    Bounds::point(6.0) * c[2],
                ]
            };
            let [a, b, c] = derivative(x);
            let [d, e, f] = derivative(y);
            if a.contains(0.0) {
                return None;
            }
            let a2 = a.square();
            let a3 = a2 * a;
            let a4 = a2.square();
            let a5 = a4 * a;
            let a6 = a3.square();
            let a7 = a6 * a;
            Some([
                d / a,
                (e * a - d * b) / a3,
                f / a3 - Bounds::point(3.0) * e * b / a4 - d * c / a4
                    + Bounds::point(3.0) * d * b.square() / a5,
                Bounds::point(-6.0) * f * b / a5 - Bounds::point(4.0) * e * c / a5
                    + Bounds::point(15.0) * e * b.square() / a6
                    + Bounds::point(10.0) * d * b * c / a6
                    - Bounds::point(15.0) * d * b.square() * b / a7,
            ])
        };
        let bound = value(parameters);
        let center = range.midpoint();
        let center_parameter = parameter(center)?;
        let Some(whole) = derivatives(parameters) else {
            return Some(Taylor::enclosed(bound).rounded());
        };
        let at_center = derivatives(center_parameter)?;
        Some(
            progress
                .compose(
                    center,
                    [
                        value(center_parameter),
                        at_center[0],
                        at_center[1] / Bounds::point(2.0),
                        at_center[2] / Bounds::point(6.0),
                    ],
                    whole[3],
                )
                .with_bound(bound)
                .rounded(),
        )
    }

    pub fn easing(&mut self, id: u16, value: Taylor) -> Option<Taylor> {
        let range = value.range().finite()?;
        if range.lo < 0.0 || range.hi > 1.0 || id > 30 {
            return None;
        }
        if let Some(value) = value.exact_constant() {
            return Some(Taylor::constant(
                fcs_runtime::evaluate_easing(id, value).ok()?,
            ));
        }
        if id == 0 {
            return Some(value);
        }
        let family = (id - 1) / 3;
        let direction = (id - 1) % 3;
        let one = Taylor::constant(1.0);
        let half = Taylor::constant(0.5);
        let result = match direction {
            0 => self.ease_in(family, value)?,
            1 if family == 9 => self.bounce(value)?,
            1 => one - self.ease_in(family, one - value)?,
            _ => {
                let left = || value.with_bound(Bounds { lo: 0.0, hi: 0.5 });
                let right = || value.with_bound(Bounds { lo: 0.5, hi: 1.0 });
                if range.hi <= 0.5 {
                    self.ease_in(family, Taylor::constant(2.0) * left())? * half
                } else if range.lo >= 0.5 {
                    one - self.ease_in(
                        family,
                        Taylor::constant(2.0) - Taylor::constant(2.0) * right(),
                    )? * half
                } else {
                    let a = self.ease_in(family, Taylor::constant(2.0) * left())? * half;
                    let b = one
                        - self.ease_in(
                            family,
                            Taylor::constant(2.0) - Taylor::constant(2.0) * right(),
                        )? * half;
                    Taylor::enclosed(a.range().hull(b.range()))
                }
            }
        };
        // Expo/Elastic pin endpoints instead of using their open-interval
        // formula. A straddling interval must enclose those values too.
        if matches!(family, 5 | 8) && (range.contains(0.0) || range.contains(1.0)) {
            let mut bound = result.range();
            if range.contains(0.0) {
                bound = bound.hull(Bounds::point(0.0));
            }
            if range.contains(1.0) {
                bound = bound.hull(Bounds::point(1.0));
            }
            return Some(Taylor::enclosed(bound));
        }
        Some(result)
    }

    fn ease_in(&mut self, family: u16, value: Taylor) -> Option<Taylor> {
        let c = Taylor::constant;
        let result = match family {
            0 => c(1.0) - self.unary(51, c(std::f64::consts::PI) * value * c(0.5))?,
            1..=4 => {
                let mut result = value;
                for _ in 0..family {
                    result = result * value;
                }
                result
            }
            5 => self.power(c(2.0), c(10.0) * value - c(10.0))?,
            6 => c(1.0) - self.unary(47, c(1.0) - value * value)?,
            7 => c(1.70158 + 1.0) * ((value * value) * value) - c(1.70158) * (value * value),
            8 => {
                let power = self.power(c(2.0), c(10.0) * value - c(10.0))?;
                let angle = (((c(10.0) * value - c(10.75)) * c(2.0)) * c(std::f64::consts::PI))
                    .divide(c(3.0))?;
                -power * self.unary(50, angle)?
            }
            9 => c(1.0) - self.bounce(c(1.0) - value)?,
            _ => return None,
        };
        Some(result)
    }

    fn bounce(&mut self, value: Taylor) -> Option<Taylor> {
        let range = value.range().finite()?;
        let mut result: Option<Bounds> = None;
        for (lower, upper, offset, base) in [
            (0.0, 1.0 / 2.75, 0.0, 0.0),
            (1.0 / 2.75, 2.0 / 2.75, 1.5 / 2.75, 0.75),
            (2.0 / 2.75, 2.5 / 2.75, 2.25 / 2.75, 0.9375),
            (2.5 / 2.75, 1.0, 2.625 / 2.75, 0.984375),
        ] {
            if range.hi < lower || range.lo > upper {
                continue;
            }
            let shifted = value.with_bound(Bounds {
                lo: lower,
                hi: upper,
            }) - Taylor::constant(offset);
            let piece = Taylor::constant(7.5625) * (shifted * shifted) + Taylor::constant(base);
            if range.lo >= lower && range.hi <= upper {
                return Some(piece);
            }
            result = Some(result.map_or(piece.range(), |bound| bound.hull(piece.range())));
        }
        result.map(Taylor::enclosed)
    }
}

#[cfg(test)]
mod tests {
    use super::{Bounds, Math, Taylor};

    #[test]
    fn polynomial_integrals_and_discarded_terms_are_enclosed() {
        let time = Taylor::variable(0.0, 2.0);
        let affine = Taylor::constant(1.5) + Taylor::constant(0.25) * time;
        let area = affine.integral(0.0, 2.0);
        assert!(area.contains(3.5));
        assert!(area.hi - area.lo < 1e-12);
        let quadratic = Taylor::constant(1.0) + Taylor::constant(0.25) * time * time;
        let area = quadratic.integral(0.0, 2.0);
        assert!(area.contains(8.0 / 3.0));
        assert!(area.hi - area.lo < 1e-12);
        let quartic = time * time * time * time;
        assert!(quartic.integral(0.0, 2.0).contains(32.0 / 5.0));
        let reciprocal = (Taylor::constant(2.0) + Taylor::variable(-0.01, 0.01))
            .reciprocal()
            .unwrap();
        for t in [-0.01, 0.0, 0.01] {
            assert!(reciprocal.range().contains(1.0 / (2.0 + t)));
        }
        assert!((Bounds { lo: -1.0, hi: 1.0 }).square().contains(0.0));
        assert!(Taylor::variable(-1.0, 1.0).reciprocal().is_none());
    }

    #[test]
    fn elementary_easing_and_bezier_models_enclose_point_queries() {
        let mut math = Math::new().unwrap();
        let signed_zero = Taylor::variable(-1.0, 1.0) * Taylor::constant(0.0);
        let angle = math
            .atan2(signed_zero, Taylor::constant(-1.0))
            .unwrap()
            .range();
        assert!(angle.contains(-std::f64::consts::PI));
        assert!(angle.contains(std::f64::consts::PI));
        let subnormal = f64::from_bits(1);
        assert_eq!(Bounds::point(subnormal).midpoint(), subnormal);
        for (start, end) in [(0.1, 0.2), (0.45, 0.55), (0.8, 0.9)] {
            let input = Taylor::variable(start, end);
            let mut models = Vec::new();
            for opcode in 44..=55 {
                models.push((math.unary(opcode, input).unwrap(), opcode, None));
            }
            for easing in 0..=30 {
                models.push((math.easing(easing, input).unwrap(), 60, Some(easing)));
            }
            for (model, opcode, easing) in models {
                for x in [-1.0, -0.5, 0.0, 0.5, 1.0] {
                    let time =
                        (start * 0.5 + end * 0.5 + (end - start) * 0.5 * x).clamp(start, end);
                    let normalized = (Bounds::point(time)
                        - (Bounds::point(start) + Bounds::point(end)) * Bounds::point(0.5))
                        / ((Bounds::point(end) - Bounds::point(start)) * Bounds::point(0.5));
                    let expected = if let Some(easing) = easing {
                        fcs_runtime::evaluate_easing(easing, time).unwrap()
                    } else {
                        let value = super::super::unary_float(
                            super::super::RuntimeValue::Scalar {
                                ty: super::super::ValueType::Float,
                                value: time,
                            },
                            opcode,
                        )
                        .unwrap();
                        super::super::scalar_payload(&value).unwrap()
                    };
                    let bound = model
                        .coefficients
                        .into_iter()
                        .rev()
                        .fold(Bounds::ZERO, |value, coefficient| {
                            value * normalized + coefficient
                        })
                        + model.remainder;
                    assert!(
                        bound.contains(expected),
                        "opcode={opcode} easing={easing:?} [{start}, {end}] x={x}: {bound:?} excludes {expected}"
                    );
                }
            }
            for controls in [
                [0.2, 0.1, 0.8, 0.9],
                [0.0, 1.0, 1.0, 0.0],
                [0.3, -0.2, 0.7, 1.2],
            ] {
                let model = math.bezier(controls, input).unwrap();
                for x in [-0.5, 0.0, 0.5] {
                    let time = start * 0.5 + end * 0.5 + (end - start) * 0.5 * x;
                    let normalized = (Bounds::point(time)
                        - (Bounds::point(start) + Bounds::point(end)) * Bounds::point(0.5))
                        / ((Bounds::point(end) - Bounds::point(start)) * Bounds::point(0.5));
                    let expected =
                        fcs_runtime::evaluate_cubic_bezier_progress(controls, time).unwrap();
                    let bound = model
                        .coefficients
                        .into_iter()
                        .rev()
                        .fold(Bounds::ZERO, |value, coefficient| {
                            value * normalized + coefficient
                        })
                        + model.remainder;
                    assert!(
                        bound.contains(expected),
                        "Bezier {controls:?}: {bound:?} excludes {expected}"
                    );
                }
            }
        }
        let bracket = fcs_runtime::cubic_bezier_parameter_bounds([0.0, 0.0], 0.125).unwrap();
        assert!(bracket[0] <= 0.5 && 0.5 <= bracket[1]);
        assert!(bracket[1] - bracket[0] <= 4.0 * f64::EPSILON);
        assert!(fcs_runtime::cubic_bezier_parameter_bounds([0.0, 1.0], -0.1).is_err());
    }
}
