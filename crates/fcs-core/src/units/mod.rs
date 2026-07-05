//! Strict unit system for FCS.
//!
//! Every physical/geometric value must carry an explicit unit.
//! Implicit conversions are forbidden.

mod color;

pub use color::Color;

use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Unit enums (§3.1–§3.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeUnit {
    Millisecond,
    Second,
    Beat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LengthUnit {
    Pixel,
    ViewportWidth,
    ViewportHeight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AngleUnit {
    Degree,
    Radian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Time(TimeUnit),
    Length(LengthUnit),
    Angle(AngleUnit),
    Dimensionless,
}

// ---------------------------------------------------------------------------
// TypedValue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypedValue {
    pub value: f64,
    pub unit: Unit,
}

impl TypedValue {
    pub const fn new(value: f64, unit: Unit) -> Self {
        Self { value, unit }
    }

    pub const fn dim(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Dimensionless,
        }
    }

    pub const fn seconds(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Time(TimeUnit::Second),
        }
    }

    pub const fn millis(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Time(TimeUnit::Millisecond),
        }
    }

    pub const fn beats(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Time(TimeUnit::Beat),
        }
    }

    pub const fn px(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Length(LengthUnit::Pixel),
        }
    }

    pub const fn deg(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Angle(AngleUnit::Degree),
        }
    }

    pub const fn rad(value: f64) -> Self {
        Self {
            value,
            unit: Unit::Angle(AngleUnit::Radian),
        }
    }

    pub const fn is_dimensionless(&self) -> bool {
        matches!(self.unit, Unit::Dimensionless)
    }

    pub const fn is_time(&self) -> bool {
        matches!(self.unit, Unit::Time(_))
    }

    pub const fn is_length(&self) -> bool {
        matches!(self.unit, Unit::Length(_))
    }

    pub const fn is_angle(&self) -> bool {
        matches!(self.unit, Unit::Angle(_))
    }

    /// Convert time to seconds. Beat values require a resolver function.
    pub fn to_seconds(&self, beat_to_sec: impl FnOnce(f64) -> f64) -> f64 {
        match self.unit {
            Unit::Time(TimeUnit::Second) => self.value,
            Unit::Time(TimeUnit::Millisecond) => self.value / 1000.0,
            Unit::Time(TimeUnit::Beat) => beat_to_sec(self.value),
            _ => panic!("to_seconds called on non-time value"),
        }
    }

    /// Convert time to milliseconds.
    pub fn to_millis(&self, beat_to_sec: impl FnOnce(f64) -> f64) -> f64 {
        self.to_seconds(beat_to_sec) * 1000.0
    }

    /// Convert length to logical pixels (1920×1080 coordinate system).
    pub fn to_px(&self) -> f64 {
        match self.unit {
            Unit::Length(LengthUnit::Pixel) => self.value,
            Unit::Length(LengthUnit::ViewportWidth) => self.value * 19.2,
            Unit::Length(LengthUnit::ViewportHeight) => self.value * 10.8,
            _ => panic!("to_px called on non-length value"),
        }
    }

    /// Convert angle to radians.
    pub fn to_radians(&self) -> f64 {
        match self.unit {
            Unit::Angle(AngleUnit::Radian) => self.value,
            Unit::Angle(AngleUnit::Degree) => self.value.to_radians(),
            _ => panic!("to_radians called on non-angle value"),
        }
    }

    /// Convert angle to degrees.
    pub fn to_degrees(&self) -> f64 {
        match self.unit {
            Unit::Angle(AngleUnit::Degree) => self.value,
            Unit::Angle(AngleUnit::Radian) => self.value.to_degrees(),
            _ => panic!("to_degrees called on non-angle value"),
        }
    }
}

impl fmt::Display for TypedValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)?;
        match self.unit {
            Unit::Time(TimeUnit::Millisecond) => write!(f, "ms"),
            Unit::Time(TimeUnit::Second) => write!(f, "s"),
            Unit::Time(TimeUnit::Beat) => write!(f, "b"),
            Unit::Length(LengthUnit::Pixel) => write!(f, "px"),
            Unit::Length(LengthUnit::ViewportWidth) => write!(f, "vw"),
            Unit::Length(LengthUnit::ViewportHeight) => write!(f, "vh"),
            Unit::Angle(AngleUnit::Degree) => write!(f, "deg"),
            Unit::Angle(AngleUnit::Radian) => write!(f, "rad"),
            Unit::Dimensionless => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------
// Unit string parsing
// ---------------------------------------------------------------------------

impl FromStr for TimeUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ms" => Ok(Self::Millisecond),
            "s" => Ok(Self::Second),
            "b" => Ok(Self::Beat),
            _ => Err(format!("unknown time unit: '{}'", s)),
        }
    }
}

impl FromStr for LengthUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "px" => Ok(Self::Pixel),
            "vw" => Ok(Self::ViewportWidth),
            "vh" => Ok(Self::ViewportHeight),
            _ => Err(format!("unknown length unit: '{}'", s)),
        }
    }
}

impl FromStr for AngleUnit {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "deg" => Ok(Self::Degree),
            "rad" => Ok(Self::Radian),
            _ => Err(format!("unknown angle unit: '{}'", s)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_to_seconds() {
        assert!((TypedValue::millis(1500.0).to_seconds(|_| 0.0) - 1.5).abs() < 1e-10);
        assert!((TypedValue::seconds(2.0).to_seconds(|_| 0.0) - 2.0).abs() < 1e-10);
        assert!((TypedValue::beats(4.0).to_seconds(|b| b / 2.0) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_length_to_px() {
        assert!((TypedValue::px(100.0).to_px() - 100.0).abs() < 1e-10);
        let vw = TypedValue::new(50.0, Unit::Length(LengthUnit::ViewportWidth));
        assert!((vw.to_px() - 960.0).abs() < 1e-10);
        let vh = TypedValue::new(50.0, Unit::Length(LengthUnit::ViewportHeight));
        assert!((vh.to_px() - 540.0).abs() < 1e-10);
    }

    #[test]
    fn test_angle_conversion() {
        assert!((TypedValue::deg(180.0).to_radians() - std::f64::consts::PI).abs() < 1e-10);
        assert!((TypedValue::rad(std::f64::consts::PI).to_degrees() - 180.0).abs() < 1e-10);
    }

    #[test]
    fn test_display() {
        assert_eq!(TypedValue::millis(120.0).to_string(), "120ms");
        assert_eq!(TypedValue::px(200.0).to_string(), "200px");
        assert_eq!(TypedValue::deg(45.0).to_string(), "45deg");
        assert_eq!(TypedValue::dim(1.5).to_string(), "1.5");
        assert_eq!(TypedValue::beats(4.0).to_string(), "4b");
    }

    #[test]
    fn test_time_unit_parse() {
        assert_eq!("ms".parse::<TimeUnit>().unwrap(), TimeUnit::Millisecond);
        assert_eq!("s".parse::<TimeUnit>().unwrap(), TimeUnit::Second);
        assert_eq!("b".parse::<TimeUnit>().unwrap(), TimeUnit::Beat);
    }
}
