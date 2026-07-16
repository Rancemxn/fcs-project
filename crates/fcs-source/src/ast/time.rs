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
        let left = self.numerator as i128 * other.denominator as i128;
        let right = other.numerator as i128 * self.denominator as i128;
        let numerator = left + right;
        let denominator = self.denominator as i128 * other.denominator as i128;
        Self::from_i128(numerator, denominator)
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
