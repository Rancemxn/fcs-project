//! Double-double arithmetic for Execution ABI section 15 scroll floors.
//!
//! Section 15 keeps the time difference, the speed/tempo product, and the
//! integral in a high-precision domain and rounds the absolute floor to
//! binary64 exactly once, after the initial floor position is added. A plain
//! binary64 chain rounds the integral before the initial offset can cancel
//! it, losing the low bits (issue #649).

/// A double-double number `hi + lo`.
///
/// ponytail: 106 bits of significand cover the section 15 conformance
/// vectors (ordinary-sized cancellations and the power-of-two vector). A
/// crafted input cancelling more than 53 bits with a full tie bit pattern
/// can still round differently than the exact real; upgrade to Shewchuk
/// expansions if that ever matters.
#[derive(Debug, Clone, Copy)]
pub struct DoubleDouble {
    hi: f64,
    lo: f64,
}

impl DoubleDouble {
    pub const fn from_f64(value: f64) -> Self {
        Self { hi: value, lo: 0.0 }
    }

    pub fn add(self, other: Self) -> Self {
        let (sum, error) = two_sum(self.hi, other.hi);
        let correction = self.lo + other.lo + error;
        let (hi, lo) = two_sum(sum, correction);
        Self { hi, lo }
    }

    pub fn neg(self) -> Self {
        Self {
            hi: -self.hi,
            lo: -self.lo,
        }
    }

    /// Multiplies by a binary64 factor, carrying both product errors.
    pub fn scale(self, factor: f64) -> Self {
        let (hi, hi_error) = two_product(self.hi, factor);
        let (lo, lo_error) = two_product(self.lo, factor);
        Self { hi, lo: hi_error }.add(Self {
            hi: lo,
            lo: lo_error,
        })
    }

    pub fn to_f64(self) -> f64 {
        if self.lo == 0.0 {
            self.hi
        } else {
            self.hi + self.lo
        }
    }

    /// Rounds once for public consumption. An exact binary64 passes through
    /// unchanged so a raw initial floor keeps its signed zero; a computed
    /// real zero is normalized to +0.0 (section 15).
    pub fn round_once(self) -> f64 {
        if self.lo == 0.0 {
            return self.hi;
        }
        let value = self.hi + self.lo;
        if value == 0.0 { 0.0 } else { value }
    }
}

/// Knuth two-sum: `left + right` split into the rounded sum and its exact error.
fn two_sum(left: f64, right: f64) -> (f64, f64) {
    let sum = left + right;
    let virtual_right = sum - left;
    let virtual_left = sum - virtual_right;
    let right_error = right - virtual_right;
    let left_error = left - virtual_left;
    (sum, left_error + right_error)
}

fn two_diff(left: f64, right: f64) -> (f64, f64) {
    two_sum(left, -right)
}

fn two_product(left: f64, right: f64) -> (f64, f64) {
    let product = left * right;
    let error = left.mul_add(right, -product);
    (product, error)
}

/// `scale * (to - from)` with the difference and both products carried
/// exactly, summed in double-double. This is the section 15 building block
/// for analytic floors and step-interval integrals.
pub fn scaled_difference(scale: f64, from: f64, to: f64) -> DoubleDouble {
    let (difference_hi, difference_lo) = two_diff(to, from);
    let (product_hi, product_lo) = two_product(scale, difference_hi);
    let (tail_hi, tail_lo) = two_product(scale, difference_lo);
    DoubleDouble {
        hi: product_hi,
        lo: product_lo,
    }
    .add(DoubleDouble {
        hi: tail_hi,
        lo: tail_lo,
    })
}
