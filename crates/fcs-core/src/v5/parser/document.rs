use crate::v5::ast::{Document, DocumentProfile};

use super::{ParseError, parse_header};

pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    let (rest, source_version) = parse_header(input)?;
    let (format_body, trailing) = take_named_block(rest, "format")?;
    if !trailing.trim().is_empty() {
        return Err(ParseError::InvalidSyntax("trailing document input"));
    }

    let format_body = strip_comments(format_body)?;
    let declaration = format_body
        .trim()
        .strip_suffix(';')
        .map(str::trim)
        .ok_or(ParseError::InvalidSyntax("document profile"))?;
    if declaration.is_empty() || declaration.contains(';') {
        return Err(ParseError::InvalidSyntax("document profile"));
    }
    let (name, value) = declaration
        .split_once(':')
        .ok_or(ParseError::InvalidSyntax("document profile"))?;
    if name.trim() != "profile" {
        return Err(ParseError::InvalidSyntax("document profile"));
    }
    let profile = parse_profile(value.trim())?;
    Ok(Document {
        source_version,
        profile,
        tempo_map: None,
    })
}

fn parse_profile(value: &str) -> Result<DocumentProfile, ParseError> {
    match value {
        "fragment" => Ok(DocumentProfile::Fragment),
        "chart" => Ok(DocumentProfile::Chart),
        "playable" => Ok(DocumentProfile::Playable),
        "renderable" => Ok(DocumentProfile::Renderable),
        "publishable" => Ok(DocumentProfile::Publishable),
        _ => Err(ParseError::InvalidSyntax("document profile")),
    }
}

fn strip_comments(input: &str) -> Result<String, ParseError> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some(character) = chars.next() {
        if line_comment {
            if character == '\n' {
                line_comment = false;
                output.push(character);
            }
            continue;
        }
        if block_comment {
            if character == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_comment = false;
                output.push(' ');
            } else if character == '\n' {
                output.push(character);
            }
            continue;
        }
        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => {
                in_string = true;
                output.push(character);
            }
            '/' if chars.peek() == Some(&'/') => {
                chars.next();
                line_comment = true;
                output.push(' ');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                block_comment = true;
                output.push(' ');
            }
            _ => output.push(character),
        }
    }

    if in_string || block_comment {
        return Err(ParseError::InvalidSyntax("format block"));
    }
    Ok(output)
}

fn take_named_block<'a>(input: &'a str, name: &str) -> Result<(&'a str, &'a str), ParseError> {
    let input = input.trim_start();
    let rest = input
        .strip_prefix(name)
        .ok_or(ParseError::InvalidSyntax("format block"))?
        .trim_start();
    let rest = rest
        .strip_prefix('{')
        .ok_or(ParseError::InvalidSyntax("format block"))?;

    let mut depth = 1;
    let mut chars = rest.char_indices().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut line_comment = false;
    let mut block_comment = false;

    while let Some((index, character)) = chars.next() {
        if line_comment {
            if character == '\n' {
                line_comment = false;
            }
            continue;
        }
        if block_comment {
            if character == '*' && chars.peek().map(|(_, next)| *next) == Some('/') {
                chars.next();
                block_comment = false;
            }
            continue;
        }
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        match character {
            '"' => in_string = true,
            '/' if chars.peek().map(|(_, next)| *next) == Some('/') => {
                chars.next();
                line_comment = true;
            }
            '/' if chars.peek().map(|(_, next)| *next) == Some('*') => {
                chars.next();
                block_comment = true;
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok((&rest[..index], &rest[index + character.len_utf8()..]));
                }
            }
            _ => {}
        }
    }

    if in_string || block_comment {
        return Err(ParseError::InvalidSyntax("format block"));
    }
    Err(ParseError::InvalidSyntax("format block"))
}
