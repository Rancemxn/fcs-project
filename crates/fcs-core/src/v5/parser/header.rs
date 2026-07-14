use std::str::FromStr;

use crate::version::{FCS_SOURCE_VERSION, Version};

use super::ParseError;

pub fn parse_header(input: &str) -> Result<(&str, Version), ParseError> {
    let (line, rest) = match input.find('\n') {
        Some(index) => {
            let line = input[..index].strip_suffix('\r').unwrap_or(&input[..index]);
            (line, &input[index + 1..])
        }
        None => (input, ""),
    };

    let version_text = line
        .strip_prefix("#fcs ")
        .ok_or(ParseError::MissingHeader)?;
    let version = Version::from_str(version_text).map_err(|_| ParseError::InvalidVersion)?;

    if !FCS_SOURCE_VERSION.supports_source(version) {
        return Err(ParseError::UnsupportedSourceVersion(version));
    }

    Ok((rest, version))
}
