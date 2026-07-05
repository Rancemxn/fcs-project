//! RPE Controls LUT baking — sample FCS `d`-dependent expressions into
//! discrete RPE Control keyframe tables.
//!
//! FCS expressions like `alpha: easeInSine(d, 0px, 400px, 0.0, 1.0, ...)`
//! are continuous functions. RPE Controls are indexed by Y-distance `x`.
//! We sample at regular intervals and produce keyframe arrays.

use crate::from_fcs::coord;
use crate::from_fcs::evaluator::{EvalEnv, eval_expr};
use fcs_core::ast::{Expression, NoteInstance};
use serde::Serialize;

const D_MAX: f64 = 1000.0;
const D_STEP: f64 = 50.0;
const TERMINAL: f64 = 9999999.0;

#[derive(Debug, Clone, Serialize)]
pub struct AlphaPoint {
    pub x: f64,
    pub easing: i32,
    pub alpha: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct PosPoint {
    pub x: f64,
    pub easing: i32,
    pub pos: f64,
}
#[derive(Debug, Clone, Serialize)]
pub struct SizePoint {
    pub x: f64,
    pub easing: i32,
    pub size: f64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RpeNoteControls {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alpha_control: Vec<AlphaPoint>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pos_control: Vec<PosPoint>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub size_control: Vec<SizePoint>,
}

/// Sample RPE Controls for a note. Returns None if no `d`-dependent properties exist.
pub fn sample_note_controls(note: &NoteInstance) -> Option<RpeNoteControls> {
    let mut ctrl = RpeNoteControls::default();
    let mut any = false;

    if let Some(e) = get_expr(note, "alpha").filter(|e| has_var(e, "d")) {
        ctrl.alpha_control = sample_alpha(e);
        any = true;
    }
    if let Some(e) = get_expr(note, "positionX").filter(|e| has_var(e, "d")) {
        ctrl.pos_control = sample_pos(e);
        any = true;
    }
    if let Some(e) = get_expr(note, "size").filter(|e| has_var(e, "d")) {
        ctrl.size_control = sample_size(e);
        any = true;
    }
    if any { Some(ctrl) } else { None }
}

fn get_expr<'a>(note: &'a NoteInstance, key: &str) -> Option<&'a Expression> {
    use fcs_core::ast::NotePropertyValue;
    note.properties
        .iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| {
            if let NotePropertyValue::Expr(e) = v {
                Some(e)
            } else {
                None
            }
        })
}

fn has_var(expr: &Expression, var: &str) -> bool {
    match expr {
        Expression::Variable(n) => n == var,
        Expression::BinaryOp { left, right, .. } => has_var(left, var) || has_var(right, var),
        Expression::UnaryOp { operand, .. } => has_var(operand, var),
        Expression::Call { args, .. } => args.iter().any(|a| has_var(a, var)),
        Expression::Ternary {
            cond,
            if_true,
            if_false,
        } => has_var(cond, var) || has_var(if_true, var) || has_var(if_false, var),
        Expression::ChainCompare { left, ops } => {
            has_var(left, var) || ops.iter().any(|(_, e)| has_var(e, var))
        }
        Expression::Literal(_) => false,
    }
}

fn d_to_x(d: f64) -> f64 {
    coord::fcs_px_to_rpe_y(d).abs()
}

fn sample_alpha(expr: &Expression) -> Vec<AlphaPoint> {
    let n = (D_MAX / D_STEP).ceil() as usize;
    let mut pts: Vec<AlphaPoint> = (0..=n)
        .filter_map(|i| {
            let d = i as f64 * D_STEP;
            if d > D_MAX {
                return None;
            }
            let e = EvalEnv { pixel_distance: d, ..Default::default() };
            Some(AlphaPoint {
                x: d_to_x(d),
                easing: 1,
                alpha: eval_expr(expr, &e).clamp(0.0, 1.0),
            })
        })
        .collect();
    let last = pts.last().map(|p| p.alpha).unwrap_or(1.0);
    pts.push(AlphaPoint {
        x: TERMINAL,
        easing: 1,
        alpha: last,
    });
    pts
}

fn sample_pos(expr: &Expression) -> Vec<PosPoint> {
    let e0 = EvalEnv { pixel_distance: 0.0, ..Default::default() };
    let base = eval_expr(expr, &e0);
    if base.abs() < 1e-10 {
        return vec![];
    }
    let n = (D_MAX / D_STEP).ceil() as usize;
    let mut pts: Vec<PosPoint> = (0..=n)
        .filter_map(|i| {
            let d = i as f64 * D_STEP;
            if d > D_MAX {
                return None;
            }
            let e = EvalEnv { pixel_distance: d, ..Default::default() };
            let v = eval_expr(expr, &e);
            Some(PosPoint {
                x: d_to_x(d),
                easing: 1,
                pos: v / base,
            })
        })
        .collect();
    let last = pts.last().map(|p| p.pos).unwrap_or(1.0);
    pts.push(PosPoint {
        x: TERMINAL,
        easing: 1,
        pos: last,
    });
    pts
}

fn sample_size(expr: &Expression) -> Vec<SizePoint> {
    let n = (D_MAX / D_STEP).ceil() as usize;
    let mut pts: Vec<SizePoint> = (0..=n)
        .filter_map(|i| {
            let d = i as f64 * D_STEP;
            if d > D_MAX {
                return None;
            }
            let e = EvalEnv { pixel_distance: d, ..Default::default() };
            Some(SizePoint {
                x: d_to_x(d),
                easing: 1,
                size: eval_expr(expr, &e).max(0.0),
            })
        })
        .collect();
    let last = pts.last().map(|p| p.size).unwrap_or(1.0);
    pts.push(SizePoint {
        x: TERMINAL,
        easing: 1,
        size: last,
    });
    pts
}

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;

    #[test]
    fn test_has_var_d() {
        let (_, e) =
            parser::parse_expression("easeInSine(d, 0px, 400px, 0.0, 1.0, 0.0, 1.0)").unwrap();
        assert!(has_var(&e, "d"));
    }

    #[test]
    fn test_no_var() {
        let (_, e) = parser::parse_expression("sin(b * pi) * 200px").unwrap();
        assert!(!has_var(&e, "d"));
    }

    #[test]
    fn test_sample_alpha() {
        let (_, e) =
            parser::parse_expression("easeInSine(d, 0px, 400px, 0.0, 1.0, 0.0, 1.0)").unwrap();
        let pts = sample_alpha(&e);
        assert!(pts.len() > 2);
        assert!((pts[0].alpha).abs() < 1e-10);
        assert_eq!(pts.last().unwrap().x, TERMINAL);
    }
}
