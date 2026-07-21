use std::fmt;

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive, Zero};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecimalLimits {
    pub max_digits: usize,
    pub max_abs_exponent: usize,
}

impl Default for DecimalLimits {
    fn default() -> Self {
        Self {
            max_digits: 4096,
            max_abs_exponent: 4096,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactDecimal {
    raw: String,
    exact: ExactRational,
    negative_zero: bool,
}

impl ExactDecimal {
    pub fn parse(raw: &str, limits: DecimalLimits) -> Result<Self, ExactNumberError> {
        let (mantissa, exponent) = split_exponent(raw, limits.max_abs_exponent)?;
        let negative = mantissa.starts_with('-');
        let unsigned = mantissa.strip_prefix('-').unwrap_or(mantissa);
        let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
        if integer.is_empty()
            || !integer.bytes().all(|byte| byte.is_ascii_digit())
            || (unsigned.contains('.')
                && (fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit())))
        {
            return Err(ExactNumberError::InvalidDecimal);
        }
        let digit_count = integer.len().saturating_add(fraction.len());
        if digit_count == 0 || digit_count > limits.max_digits {
            return Err(ExactNumberError::LimitExceeded {
                kind: "max_decimal_digits",
                limit: limits.max_digits,
                observed: digit_count,
            });
        }

        let mut digits = String::with_capacity(digit_count);
        digits.push_str(integer);
        digits.push_str(fraction);
        let mut numerator =
            BigInt::parse_bytes(digits.as_bytes(), 10).ok_or(ExactNumberError::InvalidDecimal)?;
        if negative {
            numerator = -numerator;
        }

        let scale = i64::try_from(fraction.len())
            .map_err(|_| ExactNumberError::InvalidDecimal)?
            .checked_sub(exponent)
            .ok_or(ExactNumberError::InvalidDecimal)?;
        let exact = if scale >= 0 {
            ExactRational(BigRational::new(
                numerator,
                power_of_ten(scale.unsigned_abs())?,
            ))
        } else {
            ExactRational::from_integer(numerator * power_of_ten(scale.unsigned_abs())?)
        };
        Ok(Self {
            raw: raw.to_owned(),
            negative_zero: negative && exact.is_zero(),
            exact,
        })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn exact(&self) -> &ExactRational {
        &self.exact
    }

    pub fn to_f64(&self) -> Result<f64, ExactNumberError> {
        if self.negative_zero {
            Ok(-0.0)
        } else {
            self.exact.to_f64()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactRational(pub(crate) BigRational);

impl ExactRational {
    pub(crate) fn from_integer(value: BigInt) -> Self {
        Self(BigRational::from_integer(value))
    }

    pub(crate) fn value(&self) -> &BigRational {
        &self.0
    }

    pub fn numerator(&self) -> String {
        self.0.numer().to_string()
    }

    pub fn denominator(&self) -> String {
        self.0.denom().to_string()
    }

    pub fn is_integer(&self) -> bool {
        self.0.is_integer()
    }

    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }

    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }

    pub fn is_nonnegative(&self) -> bool {
        !self.0.is_negative()
    }

    pub fn to_i64(&self) -> Option<i64> {
        if !self.0.is_integer() {
            return None;
        }
        self.0.to_integer().to_i64()
    }

    pub fn to_f64(&self) -> Result<f64, ExactNumberError> {
        self.0
            .to_f64()
            .filter(|value| value.is_finite())
            .ok_or(ExactNumberError::FloatOutOfRange)
    }
}

impl fmt::Display for ExactRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactNumberError {
    InvalidDecimal,
    LimitExceeded {
        kind: &'static str,
        limit: usize,
        observed: usize,
    },
    FloatOutOfRange,
}

impl fmt::Display for ExactNumberError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDecimal => formatter.write_str("invalid exact decimal"),
            Self::LimitExceeded {
                kind,
                limit,
                observed,
            } => write!(
                formatter,
                "exact number limit {kind} exceeded: limit {limit}, observed {observed}"
            ),
            Self::FloatOutOfRange => formatter.write_str("exact number is outside finite Float64"),
        }
    }
}

impl std::error::Error for ExactNumberError {}

fn split_exponent(raw: &str, max_abs_exponent: usize) -> Result<(&str, i64), ExactNumberError> {
    let Some(index) = raw.find(['e', 'E']) else {
        return Ok((raw, 0));
    };
    let mantissa = &raw[..index];
    let exponent = &raw[index + 1..];
    let (negative, digits) = match exponent.as_bytes().first() {
        Some(b'+') => (false, &exponent[1..]),
        Some(b'-') => (true, &exponent[1..]),
        _ => (false, exponent),
    };
    let observed = parse_bounded_usize(digits, max_abs_exponent)?;
    let exponent = i64::try_from(observed).map_err(|_| ExactNumberError::InvalidDecimal)?;
    Ok((mantissa, if negative { -exponent } else { exponent }))
}

fn parse_bounded_usize(value: &str, limit: usize) -> Result<usize, ExactNumberError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ExactNumberError::InvalidDecimal);
    }
    let mut output = 0usize;
    for byte in value.bytes() {
        output = output
            .checked_mul(10)
            .and_then(|value| value.checked_add(usize::from(byte - b'0')))
            .ok_or(ExactNumberError::LimitExceeded {
                kind: "max_abs_decimal_exponent",
                limit,
                observed: usize::MAX,
            })?;
        if output > limit {
            return Err(ExactNumberError::LimitExceeded {
                kind: "max_abs_decimal_exponent",
                limit,
                observed: output,
            });
        }
    }
    Ok(output)
}

fn power_of_ten(exponent: u64) -> Result<BigInt, ExactNumberError> {
    let exponent = u32::try_from(exponent).map_err(|_| ExactNumberError::InvalidDecimal)?;
    Ok(BigInt::from(10u8).pow(exponent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_parser_preserves_exact_value_exponents_and_signed_zero() {
        let limits = DecimalLimits::default();
        let decimal = ExactDecimal::parse("-12.3400e-2", limits).unwrap();
        assert_eq!(decimal.raw(), "-12.3400e-2");
        assert_eq!(decimal.exact().numerator(), "-617");
        assert_eq!(decimal.exact().denominator(), "5000");
        assert_eq!(decimal.to_f64().unwrap(), -0.1234);
        assert_eq!(
            ExactDecimal::parse("-0e999", limits)
                .unwrap()
                .to_f64()
                .unwrap()
                .to_bits(),
            (-0.0f64).to_bits()
        );
    }

    #[test]
    fn decimal_limits_reject_large_digits_and_exponents_before_power_allocation() {
        let limits = DecimalLimits {
            max_digits: 3,
            max_abs_exponent: 4,
        };
        assert!(matches!(
            ExactDecimal::parse("1234", limits),
            Err(ExactNumberError::LimitExceeded {
                kind: "max_decimal_digits",
                ..
            })
        ));
        assert!(matches!(
            ExactDecimal::parse("1e999999999999999999999999", limits),
            Err(ExactNumberError::LimitExceeded {
                kind: "max_abs_decimal_exponent",
                ..
            })
        ));
        for invalid in ["", ".1", "1.", "+1", "1e", "1e+", "1e2e3"] {
            assert_eq!(
                ExactDecimal::parse(invalid, DecimalLimits::default()),
                Err(ExactNumberError::InvalidDecimal)
            );
        }
    }
}
