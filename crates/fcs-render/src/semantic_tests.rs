use super::linear_axis;

#[test]
fn linear_taps_clamp_independently_at_source_edges() {
    let (left, next_left, left_fraction) = linear_axis(0.1, 0.0, 2.0, 2).unwrap();
    assert_eq!((left, next_left), (0, 0));
    assert!((left_fraction - 0.6).abs() < 1e-12);

    let (right, next_right, right_fraction) = linear_axis(2.0, 0.0, 2.0, 2).unwrap();
    assert_eq!((right, next_right), (1, 1));
    assert!((right_fraction - 0.5).abs() < 1e-12);
}
