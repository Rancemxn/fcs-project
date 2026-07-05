//! Coordinate system conversions: Phigros → FCS 1920×1080 logical space.
//!
//! From phispler-ext const.py and phicore.py:
//! - PGR_UW = 0.05625: 1 X unit = 0.05625 * screen_width = 108px @1920w
//! - PGR_UH = 0.6:     1 Y unit = 0.6    * screen_height = 648px @1080h
//! - Actual game: floorPosition * h (full screen height) = 1080 @1080p
//!
//! The Y conversion from code (floorPosition * h) matches PGR_UH=0.6 × 1080 = 648px.
//! But phicore.py uses floorPosition * h directly = floorPosition * 1080.
//! We use the documented value: 1Y = 648px.

const X_UNIT_TO_PX: f64 = 108.0;
const Y_UNIT_TO_PX: f64 = 648.0; // 1Y = 0.6 * 1080 = 648px

pub fn x_to_fcs_px(x: f64) -> f64 {
    x * X_UNIT_TO_PX
}
pub fn y_to_fcs_px(y: f64) -> f64 {
    y * Y_UNIT_TO_PX
}
pub fn px_to_vw(px: f64) -> f64 {
    px / 19.2
}
pub fn px_to_vh(px: f64) -> f64 {
    px / 10.8
}
pub fn x_to_vw(x: f64) -> f64 {
    px_to_vw(x_to_fcs_px(x))
}
pub fn y_to_vh(y: f64) -> f64 {
    px_to_vh(y_to_fcs_px(y))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_x() {
        assert!((x_to_fcs_px(1.0) - 108.0).abs() < 1e-10);
    }
    #[test]
    fn test_y() {
        assert!((y_to_fcs_px(1.0) - 648.0).abs() < 1e-10);
    }
}
