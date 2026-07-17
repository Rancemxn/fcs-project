#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Beat {
    numerator: i64,
    denominator: i64,
}

impl PartialOrd for Beat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Beat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = self.numerator as i128 * other.denominator as i128;
        let right = other.numerator as i128 * self.denominator as i128;
        left.cmp(&right)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatError {
    ZeroDenominator,
    Overflow,
}

impl Beat {
    pub fn new(numerator: i64, denominator: i64) -> Result<Self, BeatError> {
        Self::from_i128(numerator as i128, denominator as i128)
    }

    pub const fn numerator(self) -> i64 {
        self.numerator
    }

    pub const fn denominator(self) -> i64 {
        self.denominator
    }

    pub fn checked_add(self, other: Self) -> Result<Self, BeatError> {
        let left = (self.numerator as i128)
            .checked_mul(other.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        let right = (other.numerator as i128)
            .checked_mul(self.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        let numerator = left.checked_add(right).ok_or(BeatError::Overflow)?;
        let denominator = (self.denominator as i128)
            .checked_mul(other.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        Self::from_i128(numerator, denominator)
    }

    /// Returns the exact difference of two beats when it fits the source rational shape.
    pub fn checked_sub(self, other: Self) -> Result<Self, BeatError> {
        let left = (self.numerator as i128)
            .checked_mul(other.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        let right = (other.numerator as i128)
            .checked_mul(self.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        let numerator = left.checked_sub(right).ok_or(BeatError::Overflow)?;
        let denominator = (self.denominator as i128)
            .checked_mul(other.denominator as i128)
            .ok_or(BeatError::Overflow)?;
        Self::from_i128(numerator, denominator)
    }

    /// Returns this beat multiplied by a signed integer without floating-point conversion.
    pub fn checked_mul_i64(self, factor: i64) -> Result<Self, BeatError> {
        if factor == 0 {
            return Ok(Self {
                numerator: 0,
                denominator: 1,
            });
        }
        let factor = RationalParts::from_i64(factor).ok_or(BeatError::ZeroDenominator)?;
        self.apply_rational(factor, false)
    }

    /// Returns this beat divided by a signed integer without floating-point conversion.
    pub fn checked_div_i64(self, divisor: i64) -> Result<Self, BeatError> {
        let divisor = RationalParts::from_i64(divisor).ok_or(BeatError::ZeroDenominator)?;
        self.apply_rational(divisor, true)
    }

    /// Returns this beat multiplied by an exact binary64 rational value.
    ///
    /// The binary64 bit pattern is interpreted as an exact rational number; it is
    /// never rounded through an intermediate `f64` multiplication.
    pub fn checked_mul_float(self, factor: f64) -> Result<Self, BeatError> {
        if !factor.is_finite() {
            return Err(BeatError::Overflow);
        }
        if factor == 0.0 {
            return Ok(Self {
                numerator: 0,
                denominator: 1,
            });
        }
        let factor = RationalParts::from_f64(factor).ok_or(BeatError::Overflow)?;
        self.apply_rational(factor, false)
    }

    /// Returns this beat divided by an exact binary64 rational value.
    ///
    /// The binary64 bit pattern is interpreted as an exact rational number; it is
    /// never rounded through an intermediate `f64` division.
    pub fn checked_div_float(self, divisor: f64) -> Result<Self, BeatError> {
        if !divisor.is_finite() {
            return Err(BeatError::Overflow);
        }
        if divisor == 0.0 {
            return Err(BeatError::ZeroDenominator);
        }
        let divisor = RationalParts::from_f64(divisor).ok_or(BeatError::Overflow)?;
        self.apply_rational(divisor, true)
    }

    /// Returns the exact additive inverse of this beat.
    pub fn checked_neg(self) -> Result<Self, BeatError> {
        let numerator = (self.numerator as i128)
            .checked_neg()
            .ok_or(BeatError::Overflow)?;
        Self::from_i128(numerator, self.denominator as i128)
    }

    /// Returns whether this beat is exactly zero.
    pub const fn is_zero(self) -> bool {
        self.numerator == 0
    }

    fn apply_rational(self, factor: RationalParts, divide: bool) -> Result<Self, BeatError> {
        if self.is_zero() {
            return Ok(Self {
                numerator: 0,
                denominator: 1,
            });
        }
        if factor.odd_numerator == 0 {
            return Err(BeatError::ZeroDenominator);
        }

        let base = RationalParts::from_beat(self);
        let (mut numerator, mut denominator, exponent) = if divide {
            (
                base.odd_numerator
                    .checked_mul(factor.odd_denominator)
                    .ok_or(BeatError::Overflow)?,
                base.odd_denominator
                    .checked_mul(factor.odd_numerator)
                    .ok_or(BeatError::Overflow)?,
                base.exponent
                    .checked_sub(factor.exponent)
                    .ok_or(BeatError::Overflow)?,
            )
        } else {
            (
                base.odd_numerator
                    .checked_mul(factor.odd_numerator)
                    .ok_or(BeatError::Overflow)?,
                base.odd_denominator
                    .checked_mul(factor.odd_denominator)
                    .ok_or(BeatError::Overflow)?,
                base.exponent
                    .checked_add(factor.exponent)
                    .ok_or(BeatError::Overflow)?,
            )
        };

        let common = gcd(numerator, denominator);
        numerator /= common;
        denominator /= common;

        let (numerator, denominator) = if exponent >= 0 {
            (
                numerator
                    .checked_shl(exponent as u32)
                    .ok_or(BeatError::Overflow)?,
                denominator,
            )
        } else {
            (
                numerator,
                denominator
                    .checked_shl(exponent.unsigned_abs())
                    .ok_or(BeatError::Overflow)?,
            )
        };

        let numerator = signed_magnitude(numerator, base.negative ^ factor.negative)
            .ok_or(BeatError::Overflow)?;
        let denominator = i64::try_from(denominator).map_err(|_| BeatError::Overflow)?;
        Self::new(numerator, denominator)
    }

    fn from_i128(mut numerator: i128, mut denominator: i128) -> Result<Self, BeatError> {
        if denominator == 0 {
            return Err(BeatError::ZeroDenominator);
        }
        if denominator < 0 {
            numerator = -numerator;
            denominator = -denominator;
        }
        let divisor = gcd(numerator.unsigned_abs(), denominator as u128) as i128;
        let numerator = i64::try_from(numerator / divisor).map_err(|_| BeatError::Overflow)?;
        let denominator = i64::try_from(denominator / divisor).map_err(|_| BeatError::Overflow)?;
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct RationalParts {
    negative: bool,
    odd_numerator: u128,
    odd_denominator: u128,
    exponent: i32,
}

impl RationalParts {
    fn from_i64(value: i64) -> Option<Self> {
        if value == 0 {
            return None;
        }
        let negative = value.is_negative();
        let magnitude = value.unsigned_abs() as u128;
        let twos = magnitude.trailing_zeros();
        Some(Self {
            negative,
            odd_numerator: magnitude >> twos,
            odd_denominator: 1,
            exponent: twos as i32,
        })
    }

    fn from_beat(value: Beat) -> Self {
        let numerator = value.numerator.unsigned_abs() as u128;
        let denominator = value.denominator as u128;
        let numerator_twos = numerator.trailing_zeros();
        let denominator_twos = denominator.trailing_zeros();
        Self {
            negative: value.numerator.is_negative(),
            odd_numerator: numerator >> numerator_twos,
            odd_denominator: denominator >> denominator_twos,
            exponent: numerator_twos as i32 - denominator_twos as i32,
        }
    }

    fn from_f64(value: f64) -> Option<Self> {
        if !value.is_finite() || value == 0.0 {
            return None;
        }
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let fraction = bits & ((1_u64 << 52) - 1);
        let exponent_bits = ((bits >> 52) & 0x7ff) as i32;
        let (significand, exponent) = if exponent_bits == 0 {
            (fraction, -1074)
        } else {
            ((1_u64 << 52) | fraction, exponent_bits - 1023 - 52)
        };
        if significand == 0 {
            return None;
        }
        let twos = significand.trailing_zeros();
        Some(Self {
            negative,
            odd_numerator: (significand >> twos) as u128,
            odd_denominator: 1,
            exponent: exponent + twos as i32,
        })
    }
}

fn signed_magnitude(magnitude: u128, negative: bool) -> Option<i64> {
    if negative {
        if magnitude > (1_u128 << 63) {
            return None;
        }
        if magnitude == 1_u128 << 63 {
            Some(i64::MIN)
        } else {
            Some(-(magnitude as i64))
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}

const fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    if a == 0 { 1 } else { a }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bpm(f64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidBpm;

impl Bpm {
    pub fn new(value: f64) -> Result<Self, InvalidBpm> {
        if value.is_finite() && value > 0.0 {
            Ok(Self(value))
        } else {
            Err(InvalidBpm)
        }
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceBpm(f64);

impl SourceBpm {
    pub const fn from_value(value: f64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> f64 {
        self.0
    }
}
