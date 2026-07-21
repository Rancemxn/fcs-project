use std::fmt;

/// Stable product diagnostic category for FCBC framing failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FcbcError {
    category: &'static str,
    message: String,
}

impl FcbcError {
    pub fn new(category: &'static str, message: impl Into<String>) -> Self {
        Self {
            category,
            message: message.into(),
        }
    }

    pub const fn category(&self) -> &'static str {
        self.category
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for FcbcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.category, self.message)
    }
}

impl std::error::Error for FcbcError {}

pub type FcbcResult<T> = Result<T, FcbcError>;
