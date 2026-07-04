//! Easing functions (§7.2) — 29 built-in easings + cubic bezier (§7.3).

use crate::vm::math;

pub fn evaluate(easing_id: u8, t: f64) -> f64 {
    let t = math::clamp(t, 0.0, 1.0);
    match easing_id {
        1 => t,
        2 => (t * std::f64::consts::FRAC_PI_2).sin(),
        3 => 1.0 - (t * std::f64::consts::FRAC_PI_2).cos(),
        4 => 1.0 - (1.0 - t).powi(2),
        5 => t * t,
        6 => -((std::f64::consts::PI * t).cos() - 1.0) / 2.0,
        7 => if t < 0.5 { 2.0*t*t } else { 1.0 - (-2.0*t+2.0).powi(2)/2.0 },
        8 => 1.0 - (1.0 - t).powi(3),
        9 => t.powi(3),
        10 => 1.0 - (1.0 - t).powi(4),
        11 => t.powi(4),
        12 => if t < 0.5 { 4.0*t.powi(3) } else { 1.0 - (-2.0*t+2.0).powi(3)/2.0 },
        13 => if t < 0.5 { 8.0*t.powi(4) } else { 1.0 - (-2.0*t+2.0).powi(4)/2.0 },
        14 => 1.0 - (1.0 - t).powi(5),
        15 => t.powi(5),
        16 => if t >= 1.0 { 1.0 } else { 1.0 - 2.0f64.powf(-10.0*t) },
        17 => if t <= 0.0 { 0.0 } else { 2.0f64.powf(10.0*t - 10.0) },
        18 => (1.0 - (t - 1.0).powi(2)).sqrt(),
        19 => 1.0 - (1.0 - t*t).sqrt(),
        20 => { let c=1.70158; let c3=c+1.0; 1.0 + c3*(t-1.0).powi(3) + c*(t-1.0).powi(2) },
        21 => { let c=1.70158; let c3=c+1.0; c3*t.powi(3) - c*t.powi(2) },
        22 => if t < 0.5 { (1.0-(1.0-(2.0*t).powi(2)).sqrt())/2.0 } else { ((1.0-(-2.0*t+2.0).powi(2)).sqrt()+1.0)/2.0 },
        23 => { let c=2.5949095; if t<0.5 { ((2.0*t).powi(2)*((c+1.0)*2.0*t-c))/2.0 } else { ((2.0*t-2.0).powi(2)*((c+1.0)*(2.0*t-2.0)+c)+2.0)/2.0 } },
        24 => { if t<=0.0 { return 0.0; } if t>=1.0 { return 1.0; } 2.0f64.powf(-10.0*t)*((10.0*t-0.75)*2.0*std::f64::consts::PI/3.0).sin()+1.0 },
        25 => { if t<=0.0 { return 0.0; } if t>=1.0 { return 1.0; } -2.0f64.powf(10.0*t-10.0)*((10.0*t-10.75)*2.0*std::f64::consts::PI/3.0).sin() },
        26 => ease_out_bounce(t),
        27 => 1.0 - ease_out_bounce(1.0 - t),
        28 => if t < 0.5 { (1.0-ease_out_bounce(1.0-2.0*t))/2.0 } else { (1.0+ease_out_bounce(2.0*t-1.0))/2.0 },
        29 => ease_in_out_elastic(t),
        _ => t,
    }
}

fn ease_out_bounce(t: f64) -> f64 {
    let n1 = 7.5625; let d1 = 2.75;
    if t < 1.0/d1 { n1*t*t }
    else if t < 2.0/d1 { let t2=t-1.5/d1; n1*t2*t2+0.75 }
    else if t < 2.5/d1 { let t2=t-2.25/d1; n1*t2*t2+0.9375 }
    else { let t2=t-2.625/d1; n1*t2*t2+0.984375 }
}

fn ease_in_out_elastic(t: f64) -> f64 {
    if t <= 0.0 { return 0.0; } if t >= 1.0 { return 1.0; }
    let c = 2.0*std::f64::consts::PI/4.5;
    if t < 0.5 { -(2.0f64.powf(20.0*t-10.0)*((20.0*t-11.125)*c).sin())/2.0 }
    else { (2.0f64.powf(-20.0*t+10.0)*((20.0*t-11.125)*c).sin())/2.0+1.0 }
}

// ---------------------------------------------------------------------------
// Cubic bezier easing (§7.3)
// ---------------------------------------------------------------------------

pub fn evaluate_bezier(t: f64, cx1: f64, cy1: f64, cx2: f64, cy2: f64) -> f64 {
    let t = math::clamp(t, 0.0, 1.0);
    let mut u = t;
    for _ in 0..8 {
        let x = bezier_x(u, cx1, cx2);
        let dx = bezier_x_deriv(u, cx1, cx2);
        if dx.abs() < 1e-12 { break; }
        u -= (x - t) / dx;
        u = math::clamp(u, 0.0, 1.0);
    }
    bezier_y(u, cy1, cy2)
}

fn bezier_x(t: f64, cx1: f64, cx2: f64) -> f64 {
    3.0*(1.0-t).powi(2)*t*cx1 + 3.0*(1.0-t)*t.powi(2)*cx2 + t.powi(3)
}
fn bezier_x_deriv(t: f64, cx1: f64, cx2: f64) -> f64 {
    3.0*(1.0-t).powi(2)*cx1 + 6.0*(1.0-t)*t*(cx2-cx1) + 3.0*t.powi(2)*(1.0-cx2)
}
fn bezier_y(t: f64, cy1: f64, cy2: f64) -> f64 {
    3.0*(1.0-t).powi(2)*t*cy1 + 3.0*(1.0-t)*t.powi(2)*cy2 + t.powi(3)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boundaries() {
        for id in 1..=29 {
            assert!(evaluate(id, 0.0).abs() < 1e-10, "ease {} at t=0", id);
            assert!((evaluate(id, 1.0) - 1.0).abs() < 1e-10, "ease {} at t=1", id);
        }
    }

    #[test]
    fn test_in_out_quad() {
        assert!(evaluate(7, 0.0).abs() < 1e-10);
        assert!((evaluate(7, 1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_bezier_identity() {
        let r = evaluate_bezier(0.5, 0.0, 0.0, 1.0, 1.0);
        assert!((r - 0.5).abs() < 0.01);
    }
}
