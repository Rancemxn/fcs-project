use super::{
    GradientStopDrawOp, ImagePatternDrawOp, LinearGradientDrawOp, RadialGradientDrawOp,
    gradient_color, linear_axis, pattern_local_point, pattern_repeat_axes, pattern_texel_index,
    radial_gradient_color,
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
