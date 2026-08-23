use super::{
    GradientStopDrawOp, ImagePatternDrawOp, LinearGradientDrawOp, LocalShape, PathCurve,
    PathSubpath, RadialGradientDrawOp, StrokeDrawOp, flatten_curve, gradient_color, linear_axis,
    local_shape_contains, path_contains, pattern_local_point, pattern_repeat_axes,
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
fn polyline_stroke_joins_caps_and_closure_follow_section_15_2() {
    let elbow = vec![[-2.0, 0.0], [0.0, 0.0], [0.0, 2.0]];
    let open = LocalShape::Polygon {
        points: elbow.clone(),
        closed: false,
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
    // Both legs are covered to `width/2`, and butt truncates at each open end.
    assert!(stroke_contains(&open, [-1.0, 0.49], &stroke).unwrap());
    assert!(stroke_contains(&open, [0.49, 1.0], &stroke).unwrap());
    assert!(!stroke_contains(&open, [-1.0, 0.51], &stroke).unwrap());
    assert!(!stroke_contains(&open, [-2.1, 0.0], &stroke).unwrap());

    // The turn at the origin is a left turn, so its outer corner is `(0.5, -0.5)`. Bevel cuts
    // that corner off, miter fills it, and round reaches only `width/2` from the vertex.
    let corner = [0.45, -0.45];
    assert!(!stroke_contains(&open, corner, &stroke).unwrap());
    stroke.join = 2;
    assert!(!stroke_contains(&open, corner, &stroke).unwrap());
    stroke.join = 3;
    assert!(stroke_contains(&open, corner, &stroke).unwrap());

    // A right-angle turn has `miterLength/halfWidth = sqrt(2)`, so a limit below that degrades
    // the miter to the same bevel that rejected this point.
    stroke.miter_limit = 1.4;
    assert!(!stroke_contains(&open, corner, &stroke).unwrap());
    stroke.miter_limit = 2.0;

    // Round reaches a point inside the disc that bevel excludes.
    let near = [0.2, -0.4];
    stroke.join = 1;
    assert!(!stroke_contains(&open, near, &stroke).unwrap());
    stroke.join = 2;
    assert!(stroke_contains(&open, near, &stroke).unwrap());

    // Section 15.2 closes a Polygon stroke, so the implicit closing segment is stroked and the
    // open path's endpoints stop being endpoints.
    stroke.join = 1;
    let closed = LocalShape::Polygon {
        points: elbow,
        closed: true,
    };
    let closing = [-1.0, 1.0];
    assert!(!stroke_contains(&open, closing, &stroke).unwrap());
    assert!(stroke_contains(&closed, closing, &stroke).unwrap());

    // Dash runs along exact accumulated arc length from the first declared point. The legs are
    // 2 and 2 long, so a 1-on 1-off pattern covers `[0,1]` and `[2,3]`.
    stroke.dash = vec![1.0, 1.0];
    assert!(stroke_contains(&open, [-1.5, 0.0], &stroke).unwrap());
    assert!(!stroke_contains(&open, [-0.6, 0.0], &stroke).unwrap());
    assert!(stroke_contains(&open, [0.0, 0.5], &stroke).unwrap());
    assert!(!stroke_contains(&open, [0.0, 1.5], &stroke).unwrap());

    // Every dash endpoint is an endpoint, so a square cap extends past the `[0,1]` dash end.
    assert!(!stroke_contains(&open, [-0.9, 0.0], &stroke).unwrap());
    stroke.cap = 3;
    assert!(stroke_contains(&open, [-0.9, 0.0], &stroke).unwrap());
}

#[test]
fn path_fill_rules_cover_implicit_subpath_closures() {
    let outer = PathSubpath {
        points: vec![[-2.0, -2.0], [2.0, -2.0], [2.0, 2.0], [-2.0, 2.0]],
    };
    let inner_same_direction = PathSubpath {
        points: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
    };
    let inner_reverse_direction = PathSubpath {
        points: vec![[-1.0, -1.0], [-1.0, 1.0], [1.0, 1.0], [1.0, -1.0]],
    };

    assert!(path_contains(std::slice::from_ref(&outer), 1, [0.0, 0.0]));
    assert!(path_contains(
        &[outer.clone(), inner_same_direction.clone()],
        1,
        [0.0, 0.0]
    ));
    assert!(!path_contains(
        &[outer.clone(), inner_same_direction],
        2,
        [0.0, 0.0]
    ));
    assert!(!path_contains(
        &[outer, inner_reverse_direction],
        1,
        [0.0, 0.0]
    ));

    let polygon = LocalShape::Polygon {
        points: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0]],
        closed: false,
    };
    assert!(local_shape_contains(&polygon, [0.0, -0.5]));
}

#[test]
fn path_flattening_is_ordered_and_depth_bounded() {
    let curve = PathCurve::Quadratic {
        start: [0.0, 0.0],
        control: [0.0, 1.0],
        end: [1.0, 1.0],
    };
    let mut points = vec![curve.point(0.0).unwrap()];
    flatten_curve(curve, &mut points, 0).expect("quadratic flatten");
    assert_eq!(points.first(), Some(&[0.0, 0.0]));
    assert_eq!(points.last(), Some(&[1.0, 1.0]));
    assert!(points.windows(2).all(|pair| pair[0][0] <= pair[1][0]));

    let overshoot = PathCurve::Quadratic {
        start: [0.0, 0.0],
        control: [10.0, 0.0],
        end: [1.0, 0.0],
    };
    let mut points = vec![[0.0, 0.0]];
    flatten_curve(overshoot, &mut points, 0).expect("collinear overshoot flatten");
    assert!(points.iter().any(|point| point[0] > 1.0));

    let pathological = PathCurve::Quadratic {
        start: [0.0, 0.0],
        control: [1.0e150, 1.0e150],
        end: [2.0e150, 0.0],
    };
    let mut points = vec![[0.0, 0.0]];
    assert_eq!(
        flatten_curve(pathological, &mut points, 0),
        Err("render.limit-exceeded")
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
