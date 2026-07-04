//! Color type — HEX sRGB with optional alpha (§3.5).

use std::fmt;
use std::str::FromStr;

/// An sRGB color with optional alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// sRGB → linear (for blending in linear color space, §3.5).
    #[inline]
    fn srgb_to_linear(c: u8) -> f64 {
        let cf = c as f64 / 255.0;
        if cf <= 0.04045 { cf / 12.92 } else { ((cf + 0.055) / 1.055).powf(2.4) }
    }

    pub fn to_linear(&self) -> [f64; 4] {
        [
            Self::srgb_to_linear(self.r),
            Self::srgb_to_linear(self.g),
            Self::srgb_to_linear(self.b),
            self.a as f64 / 255.0,
        ]
    }
}

impl Default for Color {
    fn default() -> Self { Self::WHITE }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.a == 255 {
            write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            write!(f, "#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('#') {
            return Err(format!("color must start with '#': '{}'", s));
        }
        let hex = &s[1..];
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| format!("bad red: '{}'", &hex[0..2]))?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| format!("bad green: '{}'", &hex[2..4]))?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| format!("bad blue: '{}'", &hex[4..6]))?;
                Ok(Self::rgb(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).map_err(|_| format!("bad red: '{}'", &hex[0..2]))?;
                let g = u8::from_str_radix(&hex[2..4], 16).map_err(|_| format!("bad green: '{}'", &hex[2..4]))?;
                let b = u8::from_str_radix(&hex[4..6], 16).map_err(|_| format!("bad blue: '{}'", &hex[4..6]))?;
                let a = u8::from_str_radix(&hex[6..8], 16).map_err(|_| format!("bad alpha: '{}'", &hex[6..8]))?;
                Ok(Self::rgba(r, g, b, a))
            }
            _ => Err(format!("color must be #RRGGBB or #RRGGBBAA, got '{}'", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rgb() {
        let c: Color = "#FF0000".parse().unwrap();
        assert_eq!(c, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_parse_rgba() {
        let c: Color = "#FF000080".parse().unwrap();
        assert_eq!(c, Color::rgba(255, 0, 0, 128));
    }

    #[test]
    fn test_display() {
        assert_eq!(Color::rgb(255, 0, 0).to_string(), "#FF0000");
        assert_eq!(Color::WHITE.to_string(), "#FFFFFF");
    }

    #[test]
    fn test_linear_white() {
        let linear = Color::WHITE.to_linear();
        for &c in &linear[0..3] { assert!((c - 1.0).abs() < 0.01); }
    }
}
