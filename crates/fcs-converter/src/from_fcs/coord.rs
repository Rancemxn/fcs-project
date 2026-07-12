//! Coordinate conversions: FCS 1920×1080 logical space → PGR / RPE / PEC.
//!
//! FCS logical coordinate system (§3.2):
//!   - Width: 1920 units, Height: 1080 units
//!   - Center: (0, 0), Y-up, X-right
//!   - 1px = 1 logical unit, 1vw = 19.2 units, 1vh = 10.8 units
//!
//! PGR:
//!   - 1 X unit = 108px (@1920w), so px → X = px / 108.0
//!   - 1 Y unit = 648px (@1080h), so px → Y = px / 648.0
//!   - V1: positionX encoded as int(1000 * x + y)
//!   - V3: positionX as float, floorPosition as float
//!
//! RPE (Re:PhiEdit):
//!   - Canvas: 1200 × 900 (logical)
//!   - positionX: float, relative offset from center (0 = center)
//!
//! PEC (PhiEditer):
//!   - positionX encoded as int: (x / RPE_W + 0.5) * 2048

/// PGR coordinate constants.
const PGR_X_UNIT: f64 = 108.0;
const PGR_Y_UNIT: f64 = 648.0;

/// RPE canvas dimensions.
/// RPE canvas: 1350×900 (X: [-675, 675], Y: [-450, 450]).
/// Ref: phira-docs, phispler-ext const.py RPE_WIDTH/RPE_HEIGHT.
const RPE_W: f64 = 1350.0;
const RPE_H: f64 = 900.0;

/// PEC encoding constant.
const PEC_ENC: f64 = 2048.0;

// ---------------------------------------------------------------------------
// FCS → PGR
// ---------------------------------------------------------------------------

/// Convert FCS px to PGR X units.
pub fn fcs_px_to_pgr_x(px: f64) -> f64 {
    px / PGR_X_UNIT
}

/// Convert FCS px to PGR Y units (floorPosition).
pub fn fcs_px_to_pgr_y(px: f64) -> f64 {
    px / PGR_Y_UNIT
}

/// Encode PGR V1 positionX: `int(1000 * x + y)`.
pub fn encode_pgr_v1_position(x: f64, y: f64) -> i32 {
    (1000.0 * x + y) as i32
}

/// Decode PGR V1 positionX x component.
pub fn decode_pgr_v1_x(pos: i32) -> f64 {
    (pos - pos % 1000) as f64 / 1000.0
}

/// Decode PGR V1 positionX y component.
pub fn decode_pgr_v1_y(pos: i32) -> f64 {
    (pos % 1000) as f64
}

// ---------------------------------------------------------------------------
// FCS → RPE
// ---------------------------------------------------------------------------

/// Convert FCS px to RPE positionX (float on 1200-wide canvas, centered).
/// FCS x=0 → RPE x=0.
pub fn fcs_px_to_rpe_x(px: f64) -> f64 {
    px * (RPE_W / 1920.0)
}

/// Convert FCS px to RPE y offset.
pub fn fcs_px_to_rpe_y(px: f64) -> f64 {
    px * (RPE_H / 1080.0)
}

// ---------------------------------------------------------------------------
// FCS → PEC
// ---------------------------------------------------------------------------

/// Convert FCS px to PEC encoded x coordinate.
/// PEC: x_pec = (rpe_x / RPE_W + 0.5) * 2048
pub fn fcs_px_to_pec_x(px: f64) -> i32 {
    let rpe_x = fcs_px_to_rpe_x(px);
    ((rpe_x / RPE_W + 0.5) * PEC_ENC).round() as i32
}

/// Convert FCS px to PEC encoded y coordinate.
pub fn fcs_px_to_pec_y(px: f64) -> i32 {
    let rpe_y = fcs_px_to_rpe_y(px);
    ((rpe_y / RPE_H + 0.5) * PEC_ENC).round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fcs_center_to_pgr() {
        assert!((fcs_px_to_pgr_x(0.0)).abs() < 1e-10);
        assert!((fcs_px_to_pgr_y(0.0)).abs() < 1e-10);
    }

    #[test]
    fn test_fcs_108px_to_pgr_x() {
        assert!((fcs_px_to_pgr_x(108.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_fcs_648px_to_pgr_y() {
        assert!((fcs_px_to_pgr_y(648.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pgr_v1_encode_decode() {
        let pos = encode_pgr_v1_position(3.0, 456.0);
        assert!((decode_pgr_v1_x(pos) - 3.0).abs() < 1e-10);
        assert!((decode_pgr_v1_y(pos) - 456.0).abs() < 1e-10);
    }

    #[test]
    fn test_fcs_center_to_rpe() {
        assert!((fcs_px_to_rpe_x(0.0)).abs() < 1e-10);
    }

    #[test]
    fn test_fcs_edge_to_rpe() {
        // RPE X range: [-675, 675] at 1350-wide canvas
        assert!((fcs_px_to_rpe_x(960.0) - 675.0).abs() < 1e-10);
        assert!((fcs_px_to_rpe_x(-960.0) + 675.0).abs() < 1e-10);
    }

    #[test]
    fn test_fcs_center_to_pec_x() {
        assert_eq!(fcs_px_to_pec_x(0.0), 1024);
    }
}
