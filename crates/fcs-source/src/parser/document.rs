use crate::ast::{Document, DocumentProfile, SourceSpan};
use crate::validation::validate_profile;

use super::entities::{parse_collections, parse_templates};
use super::{ParseError, definitions::parse_definitions, parse_header, tempo::parse_tempo_map};

pub fn parse_document(input: &str) -> Result<Document, ParseError> {
    let (rest, source_version) = parse_header(input)?;
    let (format_body, trailing, _, _) = take_named_block(rest, "format", input.len())?;
    let mut trailing = skip_trivia(trailing)?;
    let mut tempo_map = None;
    let mut definitions = None;
    let mut templates = None;
    let mut collections = Vec::new();
    loop {
        if trailing.starts_with("tempoMap") && tempo_map.is_none() {
            let (tempo_body, rest, _, _) = take_named_block(trailing, "tempoMap", input.len())?;
            tempo_map = Some(parse_tempo_map(tempo_body)?);
            trailing = skip_trivia(rest)?;
        } else if trailing.starts_with("definitions") && definitions.is_none() {
            let (body, rest, body_offset, span) =
                take_named_block(trailing, "definitions", input.len())?;
            definitions = Some(parse_definitions(body, body_offset, span)?);
            trailing = skip_trivia(rest)?;
        } else if trailing.starts_with("templates") && templates.is_none() {
            let (body, rest, body_offset, span) =
                take_named_block(trailing, "templates", input.len())?;
            templates = Some(parse_templates(body, body_offset, span)?);
            trailing = skip_trivia(rest)?;
        } else if trailing.starts_with("collections") && collections.is_empty() {
            let (body, rest, body_offset, span) =
                take_named_block(trailing, "collections", input.len())?;
            collections = parse_collections(body, body_offset, span)?.collections;
            trailing = skip_trivia(rest)?;
        } else {
            break;
        }
    }
    if !skip_trivia(trailing)?.is_empty() {
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
    validate_profile(profile, tempo_map.as_ref())?;
    Ok(Document {
        source_version,
        profile,
        tempo_map,
        definitions,
        templates,
        collections,
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

pub(super) fn strip_comments(input: &str) -> Result<String, ParseError> {
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

fn skip_trivia(mut input: &str) -> Result<&str, ParseError> {
    loop {
        input = input.trim_start();
        if let Some(rest) = input.strip_prefix("//") {
            match rest.find('\n') {
                Some(index) => input = &rest[index + 1..],
                None => return Ok(""),
            }
        } else if let Some(rest) = input.strip_prefix("/*") {
            let index = rest
                .find("*/")
                .ok_or(ParseError::InvalidSyntax("trailing document input"))?;
            input = &rest[index + 2..];
        } else {
            return Ok(input);
        }
    }
}

fn take_named_block<'a>(
    input: &'a str,
    name: &str,
    source_len: usize,
) -> Result<(&'a str, &'a str, usize, SourceSpan), ParseError> {
    let input = input.trim_start();
    let block_start = source_len - input.len();
    let rest = input
        .strip_prefix(name)
        .ok_or(ParseError::InvalidSyntax("format block"))?
        .trim_start();
    let rest = rest
        .strip_prefix('{')
        .ok_or(ParseError::InvalidSyntax("format block"))?;
    let body_offset = source_len - rest.len();

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
                    let trailing = &rest[index + character.len_utf8()..];
                    let block_end = source_len - trailing.len();
                    return Ok((
                        &rest[..index],
                        trailing,
                        body_offset,
                        SourceSpan::new(block_start, block_end),
                    ));
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
