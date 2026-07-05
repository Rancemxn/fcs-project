//! FCS expression evaluator — compile-time evaluation of FCS AST expressions.
//!
//! This is a non-VM, tree-walking interpreter.  It evaluates an `Expression`
//! to `f64` given the current environment variables (`b`, `s`, `d`).
//!
//! Division by zero, NaN, or Inf results return `f64::NAN`.  The caller is
//! responsible for culling entities that produce NaN (per FCS §8.3).

use fcs_core::ast::{BinaryOp, CompareOp, Expression, Literal, UnaryOp};
use std::f64::consts::{E, PI};

/// Environment variables available during expression evaluation.
#[derive(Debug, Clone, Copy)]
pub struct EvalEnv {
    /// Current beat (b).
    pub beat: f64,
    /// Current absolute seconds (s).
    pub seconds: f64,
    /// Pixel distance from note to line centre (d).
    pub pixel_distance: f64,
}

impl Default for EvalEnv {
    fn default() -> Self {
        Self {
            beat: 0.0,
            seconds: 0.0,
            pixel_distance: 0.0,
        }
    }
}

impl EvalEnv {
    pub fn new(beat: f64, seconds: f64, pixel_distance: f64) -> Self {
        Self {
            beat,
            seconds,
            pixel_distance,
        }
    }
}

/// Evaluate an FCS expression to `f64`.
///
/// Returns `f64::NAN` on division by zero, unsupported functions, or
/// other evaluation failures.  The caller should treat NaN as "cull".
pub fn eval_expr(expr: &Expression, env: &EvalEnv) -> f64 {
    match expr {
        Expression::Literal(lit) => eval_literal(lit),
        Expression::Variable(name) => eval_variable(name, env),
        Expression::BinaryOp { op, left, right } => {
            let l = eval_expr(left, env);
            let r = eval_expr(right, env);
            eval_binary(*op, l, r)
        }
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            operand,
        } => -eval_expr(operand, env),
        Expression::Call { name, args } => eval_call(name, args, env),
        Expression::Ternary {
            cond,
            if_true,
            if_false,
        } => {
            let c = eval_expr(cond, env);
            if c != 0.0 {
                eval_expr(if_true, env)
            } else {
                eval_expr(if_false, env)
            }
        }
        Expression::ChainCompare { left, ops } => {
            let mut prev = eval_expr(left, env);
            for (op, rhs) in ops {
                let r = eval_expr(rhs, env);
                if !eval_compare(*op, prev, r) {
                    return 0.0;
                }
                prev = r;
            }
            1.0
        }
    }
}

// ---------------------------------------------------------------------------
// Literals
// ---------------------------------------------------------------------------

fn eval_literal(lit: &Literal) -> f64 {
    match lit {
        Literal::Integer(n) => *n as f64,
        Literal::Float(f) => *f,
        Literal::Quantified { value, .. } => *value,
        Literal::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Literal::Color(_) => f64::NAN,
        Literal::String(_) => f64::NAN,
    }
}

// ---------------------------------------------------------------------------
// Variables
// ---------------------------------------------------------------------------

fn eval_variable(name: &str, env: &EvalEnv) -> f64 {
    match name {
        "b" => env.beat,
        "s" => env.seconds,
        "d" => env.pixel_distance,
        "pi" => PI,
        "e" => E,
        _ => f64::NAN,
    }
}

// ---------------------------------------------------------------------------
// Binary operators
// ---------------------------------------------------------------------------

fn eval_binary(op: BinaryOp, a: f64, b: f64) -> f64 {
    match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div => {
            if b == 0.0 {
                f64::NAN
            } else {
                a / b
            }
        }
        BinaryOp::Mod => {
            if b == 0.0 {
                f64::NAN
            } else {
                a % b
            }
        }
        BinaryOp::Pow => a.powf(b),
    }
}

// ---------------------------------------------------------------------------
// Comparison
// ---------------------------------------------------------------------------

fn eval_compare(op: CompareOp, a: f64, b: f64) -> bool {
    match op {
        CompareOp::Lt => a < b,
        CompareOp::Le => a <= b,
        CompareOp::Gt => a > b,
        CompareOp::Ge => a >= b,
        CompareOp::Eq => (a - b).abs() < 1e-12,
        CompareOp::Ne => (a - b).abs() >= 1e-12,
    }
}

// ---------------------------------------------------------------------------
// Function calls
// ---------------------------------------------------------------------------

fn eval_call(name: &str, args: &[Expression], env: &EvalEnv) -> f64 {
    let evaluated: Vec<f64> = args.iter().map(|a| eval_expr(a, env)).collect();
    let a = |i: usize| evaluated.get(i).copied().unwrap_or(f64::NAN);

    match name {
        // §4.3 Built-in math functions
        "sin" => a(0).sin(),
        "cos" => a(0).cos(),
        "tan" => a(0).tan(),
        "asin" => a(0).asin(),
        "acos" => a(0).acos(),
        "atan" => a(0).atan(),
        "atan2" => a(0).atan2(a(1)),
        "abs" => a(0).abs(),
        "exp" => a(0).exp(),
        "ln" => a(0).ln(),
        "log2" => a(0).log2(),
        "log10" => a(0).log10(),
        "sqrt" => a(0).sqrt(),
        "pow" => a(0).powf(a(1)),
        "floor" => a(0).floor(),
        "ceil" => a(0).ceil(),
        "round" => a(0).round(),
        "min" => a(0).min(a(1)),
        "max" => a(0).max(a(1)),
        "clamp" => clamp(a(0), a(1), a(2)),
        "lerp" => lerp(a(0), a(1), a(2)),

        // §7 Easing functions
        n if n.starts_with("ease") => eval_easing_call(n, &evaluated),

        _ => f64::NAN,
    }
}

fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

// ---------------------------------------------------------------------------
// Easing evaluation
// ---------------------------------------------------------------------------

/// Evaluate an FCS easing call: `easeDirType(t, t0, t1, v0, v1, clampL, clampR)`.
fn eval_easing_call(name: &str, args: &[f64]) -> f64 {
    if args.len() < 7 {
        return f64::NAN;
    }
    let t = args[0];
    let t0 = args[1];
    let t1 = args[2];
    let v0 = args[3];
    let v1 = args[4];
    let clamp_left = args[5];
    let clamp_right = args[6];

    if t <= t0 {
        return v0;
    }
    if t >= t1 {
        return v1;
    }

    let mut p = (t - t0) / (t1 - t0);
    p = clamp_left + p * (clamp_right - clamp_left);
    p = p.clamp(0.0, 1.0);

    let f = if name == "easeBezier" {
        if args.len() >= 11 {
            eval_bezier(p, args[7], args[8], args[9], args[10])
        } else {
            p
        }
    } else {
        eval_easing_by_name(name, p)
    };

    v0 + f * (v1 - v0)
}

/// Evaluate a named easing function at normalised progress `t` (0–1).
fn eval_easing_by_name(name: &str, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match name {
        "easeLinear" => t,
        "easeOutSine" => (t * PI / 2.0).sin(),
        "easeInSine" => 1.0 - (t * PI / 2.0).cos(),
        "easeOutQuad" => 1.0 - (1.0 - t).powi(2),
        "easeInQuad" => t * t,
        "easeInOutSine" => -((PI * t).cos() - 1.0) / 2.0,
        "easeInOutQuad" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        "easeOutCubic" => 1.0 - (1.0 - t).powi(3),
        "easeInCubic" => t.powi(3),
        "easeOutQuart" => 1.0 - (1.0 - t).powi(4),
        "easeInQuart" => t.powi(4),
        "easeInOutCubic" => {
            if t < 0.5 {
                4.0 * t.powi(3)
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        "easeInOutQuart" => {
            if t < 0.5 {
                8.0 * t.powi(4)
            } else {
                1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
            }
        }
        "easeOutQuint" => 1.0 - (1.0 - t).powi(5),
        "easeInQuint" => t.powi(5),
        "easeOutExpo" => {
            if t >= 1.0 {
                1.0
            } else {
                1.0 - 2.0f64.powf(-10.0 * t)
            }
        }
        "easeInExpo" => {
            if t <= 0.0 {
                0.0
            } else {
                2.0f64.powf(10.0 * t - 10.0)
            }
        }
        "easeOutCirc" => (1.0 - (t - 1.0).powi(2)).sqrt(),
        "easeInCirc" => 1.0 - (1.0 - t.powi(2)).sqrt(),
        "easeOutBack" => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
        "easeInBack" => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            c3 * t.powi(3) - c1 * t.powi(2)
        }
        "easeInOutCirc" => {
            if t < 0.5 {
                (1.0 - (1.0 - (2.0 * t).powi(2)).sqrt()) / 2.0
            } else {
                ((1.0 - (-2.0 * t + 2.0).powi(2)).sqrt() + 1.0) / 2.0
            }
        }
        "easeInOutBack" => {
            let c1 = 2.5949095;
            if t < 0.5 {
                ((2.0 * t).powi(2) * ((c1 + 1.0) * 2.0 * t - c1)) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((c1 + 1.0) * (t * 2.0 - 2.0) + c1) + 2.0) / 2.0
            }
        }
        "easeOutElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                2.0f64.powf(-10.0 * t) * ((10.0 * t - 0.75) * (2.0 * PI / 3.0)).sin() + 1.0
            }
        }
        "easeInElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                -2.0f64.powf(10.0 * t - 10.0) * ((10.0 * t - 10.75) * (2.0 * PI / 3.0)).sin()
            }
        }
        "easeOutBounce" => {
            let n1 = 7.5625;
            let d1 = 2.75;
            if t < 1.0 / d1 {
                n1 * t * t
            } else if t < 2.0 / d1 {
                n1 * (t - 1.5 / d1).powi(2) + 0.75
            } else if t < 2.5 / d1 {
                n1 * (t - 2.25 / d1).powi(2) + 0.9375
            } else {
                n1 * (t - 2.625 / d1).powi(2) + 0.984375
            }
        }
        "easeInBounce" => 1.0 - eval_easing_by_name("easeOutBounce", 1.0 - t),
        "easeInOutBounce" => {
            if t < 0.5 {
                (1.0 - eval_easing_by_name("easeOutBounce", 1.0 - 2.0 * t)) / 2.0
            } else {
                (1.0 + eval_easing_by_name("easeOutBounce", 2.0 * t - 1.0)) / 2.0
            }
        }
        "easeInOutElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else if t < 0.5 {
                -2.0f64.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * (2.0 * PI / 4.5)).sin() / 2.0
            } else {
                2.0f64.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * (2.0 * PI / 4.5)).sin() / 2.0
                    + 1.0
            }
        }
        _ => t,
    }
}

/// Cubic bezier easing (§7.3).
fn eval_bezier(t: f64, cx1: f64, cy1: f64, cx2: f64, cy2: f64) -> f64 {
    // Newton-Raphson solve for x
    let mut x = t;
    for _ in 0..8 {
        let dx = 3.0 * (1.0 - x).powi(2) * cx1
            + 6.0 * (1.0 - x) * x * (cx2 - cx1)
            + 3.0 * x.powi(2) * (1.0 - cx2);
        let x_val = (1.0 - x).powi(3) * 0.0
            + 3.0 * (1.0 - x).powi(2) * x * cx1
            + 3.0 * (1.0 - x) * x.powi(2) * cx2
            + x.powi(3) * 1.0;
        if dx.abs() < 1e-10 {
            break;
        }
        x -= (x_val - t) / dx;
        x = x.clamp(0.0, 1.0);
    }
    (1.0 - x).powi(3) * 0.0
        + 3.0 * (1.0 - x).powi(2) * x * cy1
        + 3.0 * (1.0 - x) * x.powi(2) * cy2
        + x.powi(3) * 1.0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use fcs_core::parser;

    fn parse_and_eval(src: &str, env: &EvalEnv) -> f64 {
        let (_, expr) = parser::parse_expression(src).expect("parse failed");
        eval_expr(&expr, env)
    }

    #[test]
    fn test_literal_int() {
        assert!((parse_and_eval("42", &EvalEnv::default()) - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_literal_float() {
        assert!((parse_and_eval("3.14", &EvalEnv::default()) - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_var_b() {
        let env = EvalEnv::new(4.0, 0.0, 0.0);
        assert!((parse_and_eval("b", &env) - 4.0).abs() < 1e-10);
    }

    #[test]
    fn test_var_s() {
        let env = EvalEnv::new(0.0, 2.5, 0.0);
        assert!((parse_and_eval("s", &env) - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_var_d() {
        let env = EvalEnv::new(0.0, 0.0, 200.0);
        assert!((parse_and_eval("d", &env) - 200.0).abs() < 1e-10);
    }

    #[test]
    fn test_const_pi() {
        assert!((parse_and_eval("pi", &EvalEnv::default()) - PI).abs() < 1e-10);
    }

    #[test]
    fn test_const_e() {
        assert!((parse_and_eval("e", &EvalEnv::default()) - E).abs() < 1e-10);
    }

    #[test]
    fn test_add() {
        assert!((parse_and_eval("1 + 2", &EvalEnv::default()) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_mul_precedence() {
        assert!((parse_and_eval("1 + 2 * 3", &EvalEnv::default()) - 7.0).abs() < 1e-10);
    }

    #[test]
    fn test_div_zero() {
        assert!(parse_and_eval("1 / 0", &EvalEnv::default()).is_nan());
    }

    #[test]
    fn test_sin() {
        let env = EvalEnv::default();
        assert!((parse_and_eval("sin(0)", &env)).abs() < 1e-10);
        assert!((parse_and_eval("sin(pi / 2)", &env) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_clamp() {
        let env = EvalEnv::default();
        assert!((parse_and_eval("clamp(0.5, 0, 1)", &env) - 0.5).abs() < 1e-10);
        assert!((parse_and_eval("clamp(-1, 0, 1)", &env)).abs() < 1e-10);
    }

    #[test]
    fn test_ternary_true() {
        assert!((parse_and_eval("1 > 0 ? 42 : 0", &EvalEnv::default()) - 42.0).abs() < 1e-10);
    }

    #[test]
    fn test_ternary_false() {
        assert!((parse_and_eval("1 < 0 ? 42 : 99", &EvalEnv::default()) - 99.0).abs() < 1e-10);
    }

    #[test]
    fn test_chain_compare_true() {
        assert!((parse_and_eval("1 < 2 < 3", &EvalEnv::default()) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_chain_compare_false() {
        assert!((parse_and_eval("1 < 3 < 2", &EvalEnv::default())).abs() < 1e-10);
    }

    #[test]
    fn test_easing_linear() {
        let env = EvalEnv::new(2.0, 0.0, 0.0);
        let result = parse_and_eval("easeLinear(b, 0.0, 4.0, 0, 100, 0, 1)", &env);
        assert!((result - 50.0).abs() < 1e-10);
    }

    #[test]
    fn test_easing_clamp_left() {
        let env = EvalEnv::new(0.0, 0.0, 0.0);
        let result = parse_and_eval("easeOutSine(b, 0.0, 4.0, 0, 100, 0, 1)", &env);
        assert!((result - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_easing_clamp_right() {
        let env = EvalEnv::new(4.0, 0.0, 0.0);
        let result = parse_and_eval("easeOutSine(b, 0.0, 4.0, 0, 100, 0, 1)", &env);
        assert!((result - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_all_29_easings_produce_valid_numbers() {
        // Back, elastic, and bounce easings can overshoot [0, 1] — that's correct.
        // We just verify they don't produce NaN or Inf.
        let names = [
            "easeLinear",
            "easeOutSine",
            "easeInSine",
            "easeOutQuad",
            "easeInQuad",
            "easeInOutSine",
            "easeInOutQuad",
            "easeOutCubic",
            "easeInCubic",
            "easeOutQuart",
            "easeInQuart",
            "easeInOutCubic",
            "easeInOutQuart",
            "easeOutQuint",
            "easeInQuint",
            "easeOutExpo",
            "easeInExpo",
            "easeOutCirc",
            "easeInCirc",
            "easeOutBack",
            "easeInBack",
            "easeInOutCirc",
            "easeInOutBack",
            "easeOutElastic",
            "easeInElastic",
            "easeOutBounce",
            "easeInBounce",
            "easeInOutBounce",
            "easeInOutElastic",
        ];
        for name in &names {
            let result = eval_easing_by_name(name, 0.5);
            assert!(
                result.is_finite(),
                "{name}(0.5) = {result}, expected finite"
            );
        }
    }

    #[test]
    fn test_easing_endpoints_0_and_1() {
        let names = [
            "easeLinear",
            "easeOutSine",
            "easeInSine",
            "easeOutQuad",
            "easeInQuad",
            "easeInOutSine",
            "easeInOutQuad",
            "easeOutCubic",
            "easeInCubic",
            "easeOutQuart",
            "easeInQuart",
            "easeInOutCubic",
            "easeInOutQuart",
            "easeOutQuint",
            "easeInQuint",
            "easeOutExpo",
            "easeInExpo",
            "easeOutCirc",
            "easeInCirc",
            "easeOutBack",
            "easeInBack",
            "easeInOutCirc",
            "easeInOutBack",
            "easeOutElastic",
            "easeInElastic",
            "easeOutBounce",
            "easeInBounce",
            "easeInOutBounce",
            "easeInOutElastic",
        ];
        for name in &names {
            let at_0 = eval_easing_by_name(name, 0.0);
            let at_1 = eval_easing_by_name(name, 1.0);
            assert!((at_0).abs() < 1e-12, "{name}(0) = {at_0}, expected 0");
            assert!((at_1 - 1.0).abs() < 1e-12, "{name}(1) = {at_1}, expected 1");
        }
    }
}
