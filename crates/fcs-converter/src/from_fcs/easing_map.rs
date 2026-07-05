//! FCS easing ↔ RPE easingType mapping.
//!
//! FCS has 29 built-in easing functions (§7.2) plus `easeBezier` (§7.3).
//! RPE supports easing types 1–28.  RPE does NOT support type 29
//! (`easeInOutElastic`).
//!
//! Reference: `refer/phispler-ext/src/rpe_easing.py:bytecode_map`

/// Map an FCS easing function name to its numeric ID (1–29).
/// Returns `None` for `easeBezier` (which maps to RPE easingType 0 with
/// custom bezier points).
pub fn fcs_easing_id(name: &str) -> Option<u8> {
    match name {
        "easeLinear" => Some(1),
        "easeOutSine" => Some(2),
        "easeInSine" => Some(3),
        "easeOutQuad" => Some(4),
        "easeInQuad" => Some(5),
        "easeInOutSine" => Some(6),
        "easeInOutQuad" => Some(7),
        "easeOutCubic" => Some(8),
        "easeInCubic" => Some(9),
        "easeOutQuart" => Some(10),
        "easeInQuart" => Some(11),
        "easeInOutCubic" => Some(12),
        "easeInOutQuart" => Some(13),
        "easeOutQuint" => Some(14),
        "easeInQuint" => Some(15),
        "easeOutExpo" => Some(16),
        "easeInExpo" => Some(17),
        "easeOutCirc" => Some(18),
        "easeInCirc" => Some(19),
        "easeOutBack" => Some(20),
        "easeInBack" => Some(21),
        "easeInOutCirc" => Some(22),
        "easeInOutBack" => Some(23),
        "easeOutElastic" => Some(24),
        "easeInElastic" => Some(25),
        "easeOutBounce" => Some(26),
        "easeInBounce" => Some(27),
        "easeInOutBounce" => Some(28),
        "easeInOutElastic" => Some(29),
        _ => None,
    }
}

/// Map an FCS easing ID (1–29) to an RPE `easingType`.
///
/// RPE does not support type 29 (`easeInOutElastic`).
/// Returns `None` when the FCS easing has no direct RPE equivalent.
pub fn fcs_id_to_rpe_easing_type(fcs_id: u8) -> Option<u8> {
    if fcs_id == 29 {
        None // RPE does not support easeInOutElastic
    } else if (1..=28).contains(&fcs_id) {
        Some(fcs_id)
    } else {
        None
    }
}

/// RPE easing type name for display / debugging.
pub fn rpe_easing_name(easing_type: u8) -> &'static str {
    match easing_type {
        1 => "linear",
        2 => "easeOutSine",
        3 => "easeInSine",
        4 => "easeOutQuad",
        5 => "easeInQuad",
        6 => "easeInOutSine",
        7 => "easeInOutQuad",
        8 => "easeOutCubic",
        9 => "easeInCubic",
        10 => "easeOutQuart",
        11 => "easeInQuart",
        12 => "easeInOutCubic",
        13 => "easeInOutQuart",
        14 => "easeOutQuint",
        15 => "easeInQuint",
        16 => "easeOutExpo",
        17 => "easeInExpo",
        18 => "easeOutCirc",
        19 => "easeInCirc",
        20 => "easeOutBack",
        21 => "easeInBack",
        22 => "easeInOutCirc",
        23 => "easeInOutBack",
        24 => "easeOutElastic",
        25 => "easeInElastic",
        26 => "easeOutBounce",
        27 => "easeInBounce",
        28 => "easeInOutBounce",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_29_fcs_easings_have_ids() {
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
        for (i, name) in names.iter().enumerate() {
            assert_eq!(
                fcs_easing_id(name),
                Some((i + 1) as u8),
                "mismatch for {name}"
            );
        }
    }

    #[test]
    fn test_rpe_does_not_support_29() {
        assert_eq!(fcs_id_to_rpe_easing_type(29), None);
    }

    #[test]
    fn test_rpe_supports_1_through_28() {
        for id in 1..=28u8 {
            assert_eq!(fcs_id_to_rpe_easing_type(id), Some(id));
        }
    }
}
