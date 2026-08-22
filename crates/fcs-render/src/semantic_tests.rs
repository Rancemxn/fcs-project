use super::{
    GradientStopDrawOp, ImagePatternDrawOp, LinearGradientDrawOp, LocalShape, RadialGradientDrawOp,
    StrokeDrawOp, gradient_color, linear_axis, pattern_local_point, pattern_repeat_axes,
    pattern_texel_index, radial_gradient_color, stroke_contains, stroke_segments,
};

#[test]
fn linear_taps_clamp_independently_at_source_edges() {
    let (left, next_left, left_fraction) = linear_axis(0.1, 0.0, 2.0, 2).unwrap();
    assert_eq!((left, next_left), (0, 0));
    assert!((left_fraction - 0.6).abs() < 1e-12);

    let (right, next_right, right_fraction) = linear_axis(2.0, 0.0, 2.0, 2).unwrap();
    assert_eq!((right, next_right), (1, 1));
    assert!((right_fraction - 0.5).abs() < 1e-12);
}

#[test]
fn linear_gradient_pad_and_repeat_apply_at_declared_boundaries() {
    let gradient = LinearGradientDrawOp {
        start: [0.0, 0.0],
        end: [1.0, 0.0],
        spread: 1,
        stops: vec![
            GradientStopDrawOp {
                offset: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
            },
            GradientStopDrawOp {
                offset: 1.0,
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ],
    };
    assert_eq!(
        gradient_color(&gradient, [-1.0, 0.0]).unwrap(),
        [1.0, 0.0, 0.0, 1.0]
    );
    assert_eq!(
        gradient_color(&gradient, [2.0, 0.0]).unwrap(),
        [0.0, 0.0, 1.0, 1.0]
    );

    let repeat = LinearGradientDrawOp {
        spread: 2,
        ..gradient
    };
    assert_eq!(
        gradient_color(&repeat, [2.0, 0.0]).unwrap(),
        [1.0, 0.0, 0.0, 1.0]
    );
}

#[test]
fn radial_gradient_solves_quadratic_and_applies_spread() {
    let gradient = RadialGradientDrawOp {
        start_center: [0.0, 0.0],
        start_radius: 0.0,
        end_center: [0.0, 0.0],
        end_radius: 1.0,
        spread: 1,
        stops: vec![
            GradientStopDrawOp {
                offset: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
            },
            GradientStopDrawOp {
                offset: 1.0,
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ],
    };
    assert_eq!(
        radial_gradient_color(&gradient, [0.5, 0.0]).unwrap(),
        [0.5, 0.0, 0.5, 1.0]
    );

    let linear_root = RadialGradientDrawOp {
        start_center: [0.0, 0.0],
        start_radius: 0.0,
        end_center: [1.0, 0.0],
        end_radius: 1.0,
        spread: 1,
        stops: gradient.stops.clone(),
    };
    assert_eq!(
        radial_gradient_color(&linear_root, [0.25, 0.0]).unwrap(),
        [0.875, 0.0, 0.125, 1.0]
    );

    assert_eq!(
        radial_gradient_color(&gradient, [2.0, 0.0]).unwrap(),
        [0.0, 0.0, 1.0, 1.0]
    );

    let repeat = RadialGradientDrawOp {
        spread: 2,
        ..gradient.clone()
    };
    assert_eq!(
        radial_gradient_color(&repeat, [2.0, 0.0]).unwrap(),
        [1.0, 0.0, 0.0, 1.0]
    );
    let reflect = RadialGradientDrawOp {
        spread: 3,
        ..gradient
    };
    assert_eq!(
        radial_gradient_color(&reflect, [1.5, 0.0]).unwrap(),
        [0.5, 0.0, 0.5, 1.0]
    );
}

#[test]
fn radial_gradient_with_no_nonnegative_root_is_transparent() {
    let gradient = RadialGradientDrawOp {
        start_center: [0.0, 0.0],
        start_radius: 1.0,
        end_center: [0.0, 0.0],
        end_radius: 1.0,
        spread: 1,
        stops: vec![
            GradientStopDrawOp {
                offset: 0.0,
                color: [1.0, 0.0, 0.0, 1.0],
            },
            GradientStopDrawOp {
                offset: 1.0,
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ],
    };
    assert_eq!(
        radial_gradient_color(&gradient, [0.0, 0.0]).unwrap(),
        [0.0, 0.0, 0.0, 0.0]
    );
    assert_eq!(
        radial_gradient_color(&gradient, [1.0, 0.0]).unwrap(),
        [1.0, 0.0, 0.0, 1.0]
    );
}

#[test]
fn image_pattern_transform_inverse_and_repeat_edges_are_stable() {
    let pattern = ImagePatternDrawOp {
        resource_id: 1,
        position: [1.0, 2.0],
        origin: [0.0, 0.0],
        rotation: 0.0,
        scale: [2.0, 4.0],
        repeat: 4,
        sampling: 1,
    };
    assert_eq!(pattern_local_point(pattern, [3.0, 6.0]), Some([1.0, 1.0]));
    assert_eq!(pattern_repeat_axes(1).unwrap(), (false, false));
    assert_eq!(pattern_repeat_axes(4).unwrap(), (true, true));
    assert_eq!(pattern_texel_index(-0.25, 2, true), Some(1));
    assert_eq!(pattern_texel_index(2.0, 2, false), None);

    let singular = ImagePatternDrawOp {
        scale: [0.0, 1.0],
        ..pattern
    };
    assert_eq!(pattern_local_point(singular, [1.0, 2.0]), None);
}

#[test]
fn line_stroke_caps_and_dash_boundaries_are_stable() {
    let line = LocalShape::Line {
        start: [-2.0, 0.0],
        end: [2.0, 0.0],
    };
    let mut stroke = StrokeDrawOp {
        width: 1.0,
        cap: 1,
        join: 1,
        miter_limit: 2.0,
        dash_offset: 0.0,
        dash: Vec::new(),
        fill_rgba: Some([1.0, 1.0, 1.0, 1.0]),
        linear_gradient: None,
        radial_gradient: None,
        image_pattern: None,
    };
    assert!(stroke_contains(&line, [0.0, 0.49], &stroke).unwrap());
    assert!(!stroke_contains(&line, [0.0, 0.51], &stroke).unwrap());
    assert!(!stroke_contains(&line, [-2.1, 0.0], &stroke).unwrap());

    stroke.cap = 2;
    assert!(stroke_contains(&line, [-2.3, 0.3], &stroke).unwrap());
    stroke.cap = 3;
    assert!(stroke_contains(&line, [-2.4, 0.0], &stroke).unwrap());

    assert_eq!(
        stroke_segments(4.0, 0.0, &[1.0, 1.0]).unwrap(),
        vec![(0.0, 1.0), (2.0, 3.0)]
    );
    assert_eq!(
        stroke_segments(4.0, 0.5, &[1.0, 1.0]).unwrap(),
        vec![(0.0, 0.5), (1.5, 2.5), (3.5, 4.0)]
    );
}

#[test]
fn dashed_circle_stroke_starts_at_three_oclock_and_winds_clockwise() {
    let quarter = std::f64::consts::FRAC_PI_2;
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let circle = LocalShape::Circle {
        center: [0.0, 0.0],
        radius: 1.0,
    };
    let mut stroke = StrokeDrawOp {
        width: 0.2,
        cap: 1,
        join: 1,
        miter_limit: 2.0,
        dash_offset: 0.0,
        dash: Vec::new(),
        fill_rgba: Some([1.0, 1.0, 1.0, 1.0]),
        linear_gradient: None,
        radial_gradient: None,
        image_pattern: None,
    };
    // With no dash the closed stroke is the whole annulus, so both diagonals are covered.
    assert!(stroke_contains(&circle, [diagonal, -diagonal], &stroke).unwrap());
    assert!(stroke_contains(&circle, [diagonal, diagonal], &stroke).unwrap());

    // A quarter-on quarter-off pattern therefore covers the first and third quarter turns
    // travelled from three o'clock. Under FCS Y-up, clockwise means the first quarter turn
    // runs from three o'clock down to six o'clock, not up to twelve.
    stroke.dash = vec![quarter, quarter];
    assert!(stroke_contains(&circle, [diagonal, -diagonal], &stroke).unwrap());
    assert!(!stroke_contains(&circle, [diagonal, diagonal], &stroke).unwrap());
    assert!(stroke_contains(&circle, [-diagonal, diagonal], &stroke).unwrap());
    assert!(!stroke_contains(&circle, [-diagonal, -diagonal], &stroke).unwrap());

    // Each dash segment now has two endpoints, so cap participates at the six-o'clock end of
    // the first segment. Butt truncates on that endpoint's radial line.
    let just_past = -(quarter + 0.05);
    let just_past = [just_past.cos(), just_past.sin()];
    assert!(!stroke_contains(&circle, just_past, &stroke).unwrap());
    stroke.cap = 2;
    assert!(stroke_contains(&circle, just_past, &stroke).unwrap());
    stroke.cap = 3;
    assert!(stroke_contains(&circle, just_past, &stroke).unwrap());

    // The square cap is a tangential rectangle rather than a disc, so it reaches a corner
    // that lies `0.09` along and `0.09` across from the same endpoint while round does not.
    let corner = [-0.09, -0.91];
    assert!(stroke_contains(&circle, corner, &stroke).unwrap());
    stroke.cap = 2;
    assert!(!stroke_contains(&circle, corner, &stroke).unwrap());

    // No cap reaches beyond `width/2` past the endpoint.
    let beyond = -(quarter + 0.2);
    let beyond = [beyond.cos(), beyond.sin()];
    for cap in 1..=3 {
        stroke.cap = cap;
        assert!(!stroke_contains(&circle, beyond, &stroke).unwrap());
    }
}
