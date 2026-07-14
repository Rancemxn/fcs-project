use std::str::FromStr;

use crate::version::{FCS_SOURCE_VERSION, Version};

use super::{ParseError, output_from_result};

pub fn parse_header(input: &str) -> crate::diagnostic::ParseOutput<Version> {
    output_from_result(input, parse_header_inner(input).map(|(_, version)| version))
}

pub(super) fn parse_header_inner(input: &str) -> Result<(&str, Version), ParseError> {
    let bom_len = input
        .strip_prefix('\u{feff}')
        .map_or(0, |without_bom| input.len() - without_bom.len());
    let header = &input[bom_len..];
    let (line, rest_start) = match header.find('\n') {
        Some(index) => {
            let line = header[..index]
                .strip_suffix('\r')
                .unwrap_or(&header[..index]);
            (line, bom_len + index + 1)
        }
        None => (header, input.len()),
    };

    let version_text = line
        .strip_prefix("#fcs ")
        .ok_or(ParseError::MissingHeader)?;
    let version = Version::from_str(version_text).map_err(|_| ParseError::InvalidVersion)?;

    if !FCS_SOURCE_VERSION.supports_source(version) {
        return Err(ParseError::UnsupportedSourceVersion(version));
    }

    Ok((&input[rest_start..], version))
}
