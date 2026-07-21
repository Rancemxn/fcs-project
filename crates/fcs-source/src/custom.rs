//! Public implementation limits for FCS typed custom values.

/// Public compiler-profile limits for one typed-custom value tree.
///
/// FCS §7.5 requires depth, field-count, string-length, and total-byte bounds.
/// These defaults are the Core implementation profile; hosts may tighten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomValueLimits {
    max_depth: usize,
    max_fields: usize,
    max_string_bytes: usize,
    max_total_bytes: usize,
}

impl CustomValueLimits {
    pub const DEFAULT_MAX_DEPTH: usize = 32;
    pub const DEFAULT_MAX_FIELDS: usize = 4_096;
    pub const DEFAULT_MAX_STRING_BYTES: usize = 64 * 1024;
    pub const DEFAULT_MAX_TOTAL_BYTES: usize = 1024 * 1024;

    pub const fn new(
        max_depth: usize,
        max_fields: usize,
        max_string_bytes: usize,
        max_total_bytes: usize,
    ) -> Self {
        Self {
            max_depth,
            max_fields,
            max_string_bytes,
            max_total_bytes,
        }
    }

    pub const fn max_depth(self) -> usize {
        self.max_depth
    }

    pub const fn max_fields(self) -> usize {
        self.max_fields
    }

    pub const fn max_string_bytes(self) -> usize {
        self.max_string_bytes
    }

    pub const fn max_total_bytes(self) -> usize {
        self.max_total_bytes
    }
}

impl Default for CustomValueLimits {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_MAX_DEPTH,
            Self::DEFAULT_MAX_FIELDS,
            Self::DEFAULT_MAX_STRING_BYTES,
            Self::DEFAULT_MAX_TOTAL_BYTES,
        )
    }
}
