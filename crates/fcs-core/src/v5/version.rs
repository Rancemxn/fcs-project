use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    pub const fn supports_source(self, source: Self) -> bool {
        self.major == source.major && self.minor >= source.minor
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParseError;

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('.');
        let major = parts
            .next()
            .ok_or(VersionParseError)?
            .parse()
            .map_err(|_| VersionParseError)?;
        let minor = parts
            .next()
            .ok_or(VersionParseError)?
            .parse()
            .map_err(|_| VersionParseError)?;
        let patch = parts
            .next()
            .ok_or(VersionParseError)?
            .parse()
            .map_err(|_| VersionParseError)?;
        if parts.next().is_some() {
            return Err(VersionParseError);
        }
        Ok(Self::new(major, minor, patch))
    }
}

pub const FCS_SOURCE_VERSION: Version = Version::new(5, 0, 0);
pub const FCBC_FORMAT_VERSION: Version = Version::new(2, 0, 0);
pub const EXECUTION_ABI_VERSION: Version = Version::new(1, 0, 0);
