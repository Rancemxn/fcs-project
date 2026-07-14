use crate::ast::{Document, DocumentProfile, SourceSpan};
use crate::validation::validate_profile;

use super::entities::{parse_collections, parse_templates};
use super::{
    ParseError, ParseLimits, definitions::parse_definitions, header::parse_header_inner,
    output_from_result, tempo::parse_tempo_map,
};

pub fn parse_document(input: &str) -> crate::diagnostic::ParseOutput<Document> {
    parse_document_with_limits(input, ParseLimits::default())
}

pub fn parse_document_with_limits<L: Into<ParseLimits>>(
    input: &str,
    limits: L,
) -> crate::diagnostic::ParseOutput<Document> {
    let limits = limits.into();
    if input.len() > limits.max_input_bytes {
        return output_from_result(input, Err(ParseError::InvalidSyntax("resource limit")));
    }
    if count_document_tokens(input) > limits.max_tokens
        || document_nesting_depth(input) > limits.max_nesting_depth
    {
        return output_from_result(input, Err(ParseError::InvalidSyntax("resource limit")));
    }
    let result = parse_document_inner(input);
    match result {
        Ok(document) => output_from_result(input, Ok(document)),
        Err(error) => {
            let mut diagnostics = vec![error.diagnostic(input)];
            if let Some(span) = invalid_profile_span(input)
                && !diagnostics.iter().any(|diagnostic| {
                    matches!(
                        diagnostic.code(),
                        crate::diagnostic::DiagnosticCode::SYNTAX_UNCLOSED_COMMENT
                            | crate::diagnostic::DiagnosticCode::SYNTAX_UNCLOSED_STRING
                    )
                })
                && !diagnostics.iter().any(|diagnostic| {
                    diagnostic.code() == crate::diagnostic::DiagnosticCode::SYNTAX_INVALID_TOKEN
                        && diagnostic.primary_span() == span
                })
            {
                diagnostics.push(crate::diagnostic::Diagnostic::new(
                    crate::diagnostic::DiagnosticCode::SYNTAX_INVALID_TOKEN,
                    crate::diagnostic::DiagnosticStage::Parse,
                    "invalid document profile",
                    span,
                ));
            }
            crate::diagnostic::ParseOutput::new(None, diagnostics)
        }
    }
}

fn count_document_tokens(input: &str) -> usize {
    let mut offset = 0;
    let mut count = 0usize;
    while offset < input.len() {
        let remaining = &input[offset..];
        let character = remaining
            .chars()
            .next()
            .expect("offset stays on a boundary");
        if character.is_whitespace() {
            offset += character.len_utf8();
        } else if remaining.starts_with("//") {
            offset += 2;
            while offset < input.len() {
                let character = input[offset..].chars().next().expect("offset is valid");
                offset += character.len_utf8();
                if character == '\n' {
                    break;
                }
            }
        } else if remaining.starts_with("/*") {
            offset += 2;
            if let Some(end) = input[offset..].find("*/") {
                offset += end + 2;
            } else {
                break;
            }
        } else if character == '"' {
            count = count.saturating_add(1);
            offset += character.len_utf8();
            let mut escaped = false;
            while offset < input.len() {
                let character = input[offset..].chars().next().expect("offset is valid");
                offset += character.len_utf8();
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    break;
                }
            }
        } else if character.is_ascii_alphanumeric() || character == '_' {
            count = count.saturating_add(1);
            offset += character.len_utf8();
            while offset < input.len() {
                let character = input[offset..].chars().next().expect("offset is valid");
                if character.is_ascii_alphanumeric() || character == '_' {
                    offset += character.len_utf8();
                } else {
                    break;
                }
            }
        } else {
            count = count.saturating_add(1);
            offset += character.len_utf8();
        }
    }
    count
}

fn document_nesting_depth(input: &str) -> usize {
    let mut depth = 0usize;
    let mut maximum = 0usize;
    let mut escaped = false;
    let mut in_string = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if in_line_comment {
            if character == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if in_block_comment {
            if character == '*' && characters.peek() == Some(&'/') {
                characters.next();
                in_block_comment = false;
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
            '/' if characters.peek() == Some(&'/') => {
                characters.next();
                in_line_comment = true;
            }
            '/' if characters.peek() == Some(&'*') => {
                characters.next();
                in_block_comment = true;
            }
            '(' | '[' | '{' => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }
    maximum
}

fn parse_document_inner(input: &str) -> Result<Document, ParseError> {
    let (rest, source_version) = parse_header_inner(input)?;
    let (format_body, trailing, _, format_span) = take_named_block(rest, "format", input.len())?;
    let mut trailing = skip_trivia(trailing)?;
    let mut tempo_map = None;
    let mut tempo_span = None;
    let mut definitions = None;
    let mut templates = None;
    let mut collections = Vec::new();
    loop {
        if trailing.starts_with("tempoMap") && tempo_map.is_none() {
            let (tempo_body, rest, _, span) = take_named_block(trailing, "tempoMap", input.len())?;
            tempo_map = Some(parse_tempo_map(tempo_body)?);
            tempo_span = Some(span);
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
    validate_profile(
        profile,
        tempo_map.as_ref(),
        tempo_span.unwrap_or(format_span),
    )
    .map_err(ParseError::Diagnostic)?;
    Ok(Document {
        source_version,
        profile,
        tempo_map,
        definitions,
        templates,
        collections,
    })
}

fn invalid_profile_span(input: &str) -> Option<SourceSpan> {
    let (rest, _) = super::header::parse_header_inner(input).ok()?;
    let (body, _, body_offset, _) = take_named_block(rest, "format", input.len()).ok()?;
    let (value_start, value_end) = find_profile_value_span(body)?;
    let value = &body[value_start..value_end];
    if matches!(
        value,
        "fragment" | "chart" | "playable" | "renderable" | "publishable"
    ) {
        return None;
    }
    Some(SourceSpan::new(
        body_offset + value_start,
        body_offset + value_end,
    ))
}

fn find_profile_value_span(body: &str) -> Option<(usize, usize)> {
    let mut offset = 0;
    let mut block_comment_depth = 0usize;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut escaped = false;
    while offset < body.len() {
        let remaining = &body[offset..];
        let character = remaining
            .chars()
            .next()
            .expect("offset stays on a boundary");
        let width = character.len_utf8();
        if in_line_comment {
            offset += width;
            if character == '\n' {
                in_line_comment = false;
            }
            continue;
        }
        if block_comment_depth > 0 {
            if remaining.starts_with("/*") {
                block_comment_depth += 1;
                offset += 2;
            } else if remaining.starts_with("*/") {
                block_comment_depth -= 1;
                offset += 2;
            } else {
                offset += width;
            }
            continue;
        }
        if in_string {
            offset += width;
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        if remaining.starts_with("//") {
            in_line_comment = true;
            offset += 2;
            continue;
        }
        if remaining.starts_with("/*") {
            block_comment_depth = 1;
            offset += 2;
            continue;
        }
        if character == '"' {
            in_string = true;
            offset += width;
            continue;
        }
        if remaining.starts_with("profile")
            && !body[..offset]
                .chars()
                .next_back()
                .is_some_and(is_profile_identifier_continue)
            && !body[offset + "profile".len()..]
                .chars()
                .next()
                .is_some_and(is_profile_identifier_continue)
        {
            let mut value_start = offset + "profile".len();
            while value_start < body.len()
                && body[value_start..]
                    .chars()
                    .next()
                    .is_some_and(char::is_whitespace)
            {
                value_start += body[value_start..]
                    .chars()
                    .next()
                    .expect("value start is a boundary")
                    .len_utf8();
            }
            if !body[value_start..].starts_with(':') {
                offset += width;
                continue;
            }
            value_start = skip_profile_trivia(body, value_start + 1);
            let raw_start = value_start;
            let mut cursor = value_start;
            let mut raw_end = value_start;
            let mut normalized = String::new();
            while cursor < body.len() {
                let remaining = &body[cursor..];
                if remaining.starts_with(';') {
                    break;
                }
                if remaining.starts_with("//") {
                    cursor += 2;
                    while cursor < body.len() {
                        let character = body[cursor..].chars().next().expect("cursor is valid");
                        cursor += character.len_utf8();
                        if character == '\n' {
                            break;
                        }
                    }
                    continue;
                }
                if remaining.starts_with("/*") {
                    cursor = skip_profile_trivia(body, cursor);
                    continue;
                }
                let character = remaining
                    .chars()
                    .next()
                    .expect("cursor stays on a boundary");
                if !character.is_whitespace() {
                    normalized.push(character);
                    raw_end = cursor + character.len_utf8();
                }
                cursor += character.len_utf8();
            }
            if matches!(
                normalized.as_str(),
                "fragment" | "chart" | "playable" | "renderable" | "publishable"
            ) {
                return None;
            }
            return Some((raw_start, raw_end.max(raw_start)));
        }
        offset += width;
    }
    None
}

fn skip_profile_trivia(body: &str, mut offset: usize) -> usize {
    loop {
        while offset < body.len()
            && body[offset..]
                .chars()
                .next()
                .is_some_and(char::is_whitespace)
        {
            offset += body[offset..]
                .chars()
                .next()
                .expect("trivia offset is a boundary")
                .len_utf8();
        }
        let remaining = &body[offset..];
        if remaining.starts_with("//") {
            offset += 2;
            while offset < body.len() {
                let character = body[offset..].chars().next().expect("offset is valid");
                offset += character.len_utf8();
                if character == '\n' {
                    break;
                }
            }
        } else if let Some(after_open) = remaining.strip_prefix("/*") {
            let Some(end) = after_open.find("*/") else {
                return body.len();
            };
            offset += end + 4;
        } else {
            return offset;
        }
    }
}

fn is_profile_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
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
