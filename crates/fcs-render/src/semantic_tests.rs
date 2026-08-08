use super::{GradientStopDrawOp, LinearGradientDrawOp, gradient_color, linear_axis};

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
