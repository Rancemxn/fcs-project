use std::{cmp::Ordering, fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VersionComponent(VersionComponentRepr);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum VersionComponentRepr {
    Small(u16),
    Large(Box<str>),
}

impl VersionComponent {
    pub const fn new(value: u16) -> Self {
        Self(VersionComponentRepr::Small(value))
    }

    pub const fn as_u16(&self) -> Option<u16> {
        match self.0 {
            VersionComponentRepr::Small(value) => Some(value),
            VersionComponentRepr::Large(_) => None,
        }
    }

    fn parse(component: Option<&str>) -> Result<Self, VersionParseError> {
        let component = component.ok_or(VersionParseError)?;
        if component.is_empty()
            || !component.bytes().all(|byte| byte.is_ascii_digit())
            || (component.len() > 1 && component.starts_with('0'))
        {
            return Err(VersionParseError);
        }
        Ok(component.parse::<u16>().map_or_else(
            |_| Self(VersionComponentRepr::Large(component.into())),
            Self::new,
        ))
    }
}

impl fmt::Display for VersionComponent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            VersionComponentRepr::Small(value) => value.fmt(formatter),
            VersionComponentRepr::Large(value) => formatter.write_str(value),
        }
    }
}

impl PartialOrd for VersionComponent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VersionComponent {
    fn cmp(&self, other: &Self) -> Ordering {
        match (&self.0, &other.0) {
            (VersionComponentRepr::Small(left), VersionComponentRepr::Small(right)) => {
                left.cmp(right)
            }
            (VersionComponentRepr::Small(_), VersionComponentRepr::Large(_)) => Ordering::Less,
            (VersionComponentRepr::Large(_), VersionComponentRepr::Small(_)) => Ordering::Greater,
            (VersionComponentRepr::Large(left), VersionComponentRepr::Large(right)) => left
                .len()
                .cmp(&right.len())
                .then_with(|| left.as_bytes().cmp(right.as_bytes())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: VersionComponent,
    pub minor: VersionComponent,
    pub patch: VersionComponent,
}

impl Version {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major: VersionComponent::new(major),
            minor: VersionComponent::new(minor),
            patch: VersionComponent::new(patch),
        }
    }

    pub fn supports_source(&self, source: &Self) -> bool {
        self.major == source.major && self.minor >= source.minor
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionParseError;

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('.');
        let major = VersionComponent::parse(parts.next())?;
        let minor = VersionComponent::parse(parts.next())?;
        let patch = VersionComponent::parse(parts.next())?;
        if parts.next().is_some() {
            return Err(VersionParseError);
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

pub const FCS_SOURCE_VERSION: Version = Version::new(5, 0, 0);
pub const FCBC_FORMAT_VERSION: Version = Version::new(2, 0, 0);
pub const EXECUTION_ABI_VERSION: Version = Version::new(1, 0, 0);
