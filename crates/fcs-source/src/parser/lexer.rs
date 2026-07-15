use chumsky::error::RichReason;
use chumsky::prelude::*;

use crate::ast::{Beat, Bpm, Color, SourceLiteral, SourceSpan};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use crate::version::{FCS_SOURCE_VERSION, Version};

use super::ParseLimits;
use super::input::{ChumskySpan, SpannedToken, source_span};
use super::token::{Keyword, Punctuation, Token};

type LexerExtra<'source> = extra::Err<Rich<'source, char, ChumskySpan>>;

struct HeaderPrefix<'source> {
    header: Option<(Version, ChumskySpan)>,
    body: &'source str,
    offset: usize,
}

pub(super) fn lex(source: &str, limits: ParseLimits) -> Result<Vec<SpannedToken>, Vec<Diagnostic>> {
    if source.len() > limits.max_source_bytes {
        return Err(vec![resource_limit(SourceSpan::new(0, source.len()))]);
    }
    let HeaderPrefix {
        header,
        body,
        offset,
    } = header_prefix(source)?;
    if let Some(diagnostic) = validate_trivia(body, limits) {
        return Err(vec![shift_diagnostic(diagnostic, offset)]);
    }
    let (tokens, errors) = lexer(limits).parse(body).into_output_errors();
    if !errors.is_empty() {
        return Err(errors
            .into_iter()
            .map(rich_diagnostic)
            .map(|diagnostic| shift_diagnostic(diagnostic, offset))
            .collect());
    }
    let mut tokens = tokens.expect("a complete lexer produces tokens when it has no errors");
    for (_, span) in &mut tokens {
        *span = ChumskySpan::new((), span.start + offset..span.end + offset);
    }
    if let Some((version, span)) = header {
        tokens.insert(0, (Token::Header(version), span));
    }
    if tokens.len() > limits.max_tokens {
        return Err(vec![resource_limit(SourceSpan::new(0, source.len()))]);
    }
    if let Some((_, span)) = tokens.iter().find(|(token, span)| {
        matches!(token, Token::Literal(_))
            && span.end.saturating_sub(span.start) > limits.max_literal_bytes
    }) {
        return Err(vec![resource_limit(source_span(*span))]);
    }
    if let Some(span) = nesting_limit(&tokens, limits.max_nesting_depth) {
        return Err(vec![resource_limit(span)]);
    }
    Ok(tokens)
}

fn lexer<'source>(
    _limits: ParseLimits,
) -> impl Parser<'source, &'source str, Vec<SpannedToken>, LexerExtra<'source>> {
    let digit = one_of("0123456789");
    let integer = just('0')
        .ignored()
        .or(one_of("123456789").then(digit.repeated()).ignored())
        .ignored();
    let fraction = just('.').then(digit.repeated().at_least(1)).ignored();
    let exponent = one_of("eE")
        .then(one_of("+-").or_not())
        .then(digit.repeated().at_least(1))
        .ignored();
    let unit = choice((
        just("beat"),
        just("min"),
        just("rad"),
        just("turn"),
        just("deg"),
        just("bpm"),
        just("ns"),
        just("us"),
        just("ms"),
        just("px"),
        just("s"),
    ))
    .or_not()
    .ignored();
    let number = integer
        .then(fraction.or_not())
        .then(exponent.or_not())
        .then(unit)
        .to_slice()
        .try_map(|text: &str, span| {
            parse_number_token(text).map_err(|message| Rich::custom(span, message))
        });

    let escape = just('\\').ignore_then(choice((
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        just('\\').to('\\'),
        just('"').to('"'),
        just('0').to('\0'),
        just('u')
            .ignore_then(just('{'))
            .ignore_then(
                one_of("0123456789abcdefABCDEF")
                    .repeated()
                    .at_least(1)
                    .at_most(6)
                    .collect::<String>(),
            )
            .then_ignore(just('}'))
            .try_map(|digits, span| {
                u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                    .ok_or_else(|| Rich::custom(span, "invalid unicode escape"))
            }),
    )));
    let string = just('"')
        .ignore_then(
            escape
                .or(none_of("\\\"\r\n"))
                .repeated()
                .collect::<String>(),
        )
        .then_ignore(just('"'))
        .map(|value| Token::Literal(SourceLiteral::String(value)));

    let color_digits = one_of("0123456789abcdefABCDEF")
        .repeated()
        .exactly(8)
        .collect::<String>()
        .or(one_of("0123456789abcdefABCDEF")
            .repeated()
            .exactly(6)
            .collect::<String>());
    let color = just('#').then(color_digits).try_map(|(_, digits), span| {
        format!("#{digits}")
            .parse::<Color>()
            .map(SourceLiteral::Color)
            .map(Token::Literal)
            .map_err(|_| Rich::custom(span, "malformed color literal"))
    });

    let identifier = text::ascii::ident().map(|identifier: &str| match identifier {
        "true" => Token::Literal(SourceLiteral::Bool(true)),
        "false" => Token::Literal(SourceLiteral::Bool(false)),
        identifier => Keyword::from_identifier(identifier)
            .map_or_else(|| Token::Identifier(identifier.to_owned()), Token::Keyword),
    });

    let compound_punctuation = choice((
        just("..<").to(Token::Punctuation(Punctuation::RangeExclusive)),
        just("..=").to(Token::Punctuation(Punctuation::RangeInclusive)),
        just("->").to(Token::Punctuation(Punctuation::Arrow)),
        just("=>").to(Token::Punctuation(Punctuation::FatArrow)),
        just("**").to(Token::Punctuation(Punctuation::Power)),
        just("==").to(Token::Punctuation(Punctuation::EqualEqual)),
        just("!=").to(Token::Punctuation(Punctuation::BangEqual)),
        just("<=").to(Token::Punctuation(Punctuation::LessThanOrEqual)),
        just(">=").to(Token::Punctuation(Punctuation::GreaterThanOrEqual)),
        just("&&").to(Token::Punctuation(Punctuation::AndAnd)),
        just("||").to(Token::Punctuation(Punctuation::OrOr)),
        just("..").try_map(|_, span| Err::<Token, _>(Rich::custom(span, "bare range operator"))),
    ));
    let simple_punctuation = choice((
        just('(').to(Token::Punctuation(Punctuation::LeftParenthesis)),
        just(')').to(Token::Punctuation(Punctuation::RightParenthesis)),
        just('[').to(Token::Punctuation(Punctuation::LeftBracket)),
        just(']').to(Token::Punctuation(Punctuation::RightBracket)),
        just('{').to(Token::Punctuation(Punctuation::LeftBrace)),
        just('}').to(Token::Punctuation(Punctuation::RightBrace)),
        just(',').to(Token::Punctuation(Punctuation::Comma)),
        just(':').to(Token::Punctuation(Punctuation::Colon)),
        just(';').to(Token::Punctuation(Punctuation::Semicolon)),
        just('.')
            .then_ignore(just('.').not())
            .to(Token::Punctuation(Punctuation::Dot)),
        just('@').to(Token::Punctuation(Punctuation::At)),
        just('+').to(Token::Punctuation(Punctuation::Plus)),
        just('-').to(Token::Punctuation(Punctuation::Minus)),
        just('*').to(Token::Punctuation(Punctuation::Star)),
        just('/').to(Token::Punctuation(Punctuation::Slash)),
        just('%').to(Token::Punctuation(Punctuation::Percent)),
        just('!').to(Token::Punctuation(Punctuation::Bang)),
        just('=').to(Token::Punctuation(Punctuation::Equal)),
        just('<').to(Token::Punctuation(Punctuation::LessThan)),
        just('>').to(Token::Punctuation(Punctuation::GreaterThan)),
    ));
    let punctuation = choice((compound_punctuation, simple_punctuation));

    let line_comment = just("//")
        .ignore_then(any().and_is(just('\n').not()).repeated())
        .ignored();
    let block_comment = recursive(|comment| {
        just("/*")
            .ignore_then(
                choice((
                    comment,
                    any()
                        .and_is(just("/*").not())
                        .and_is(just("*/").not())
                        .ignored(),
                ))
                .repeated(),
            )
            .then_ignore(just("*/"))
            .ignored()
    });
    let trivia = choice((
        line_comment,
        block_comment,
        one_of(" \t\r\n").ignored(),
        just('\u{feff}').ignored(),
    ))
    .repeated();

    let token = choice((string, color, number, identifier, punctuation))
        .map_with(|token, extra| (token, extra.span()));
    trivia
        .clone()
        .ignore_then(token.then_ignore(trivia).repeated().collect())
        .then_ignore(end())
}

fn parse_number_token(text: &str) -> Result<Token, &'static str> {
    const UNITS: [&str; 10] = [
        "beat", "turn", "min", "rad", "deg", "bpm", "ns", "us", "ms", "px",
    ];
    let (number, unit) = UNITS
        .into_iter()
        .find_map(|unit| text.strip_suffix(unit).map(|number| (number, unit)))
        .or_else(|| text.strip_suffix('s').map(|number| (number, "s")))
        .unwrap_or((text, ""));
    if number.starts_with('0')
        && number.len() > 1
        && !number.starts_with("0.")
        && !number.starts_with("0e")
        && !number.starts_with("0E")
    {
        return Err("leading zero in numeric literal");
    }
    if unit.is_empty() {
        if number.contains(['.', 'e', 'E']) {
            return finite_float(number)
                .map(SourceLiteral::Float)
                .map(Token::Literal);
        }
        return number
            .parse::<i64>()
            .map(SourceLiteral::Int)
            .map(Token::Literal)
            .map_err(|_| "invalid integer literal");
    }
    if unit == "beat" {
        return parse_beat(number)
            .map(SourceLiteral::Beat)
            .map(Token::Literal);
    }
    let value = finite_float(number)?;
    if unit == "bpm" {
        return Bpm::new(value)
            .map(Token::TempoBpm)
            .map_err(|_| "invalid bpm literal");
    }
    let literal = match unit {
        "ns" => SourceLiteral::Time(value / 1_000_000_000.0),
        "us" => SourceLiteral::Time(value / 1_000_000.0),
        "ms" => SourceLiteral::Time(value / 1_000.0),
        "s" => SourceLiteral::Time(value),
        "min" => SourceLiteral::Time(value * 60.0),
        "px" => SourceLiteral::Length(value),
        "deg" => SourceLiteral::Angle(value.to_radians()),
        "rad" => SourceLiteral::Angle(value),
        "turn" => SourceLiteral::Angle(value * std::f64::consts::TAU),
        _ => return Err("unknown numeric unit"),
    };
    matches!(literal, SourceLiteral::Time(value) | SourceLiteral::Length(value) | SourceLiteral::Angle(value) if value.is_finite())
        .then_some(literal).map(Token::Literal).ok_or("non-finite numeric literal")
}

fn header_prefix(source: &str) -> Result<HeaderPrefix<'_>, Vec<Diagnostic>> {
    let bom_len = source
        .strip_prefix('\u{feff}')
        .map_or(0, |_| '\u{feff}'.len_utf8());
    let source_without_bom = &source[bom_len..];
    if !source_without_bom.starts_with("#fcs ") {
        return Ok(HeaderPrefix {
            header: None,
            body: source,
            offset: 0,
        });
    }
    let Some(line_end) = source_without_bom.find('\n') else {
        return Err(vec![version_diagnostic(
            DiagnosticCode::VERSION_INVALID,
            SourceSpan::new(0, source.len()),
        )]);
    };
    let line = source_without_bom[..line_end].trim_end_matches('\r');
    let version = line[5..].parse::<Version>().map_err(|_| {
        vec![version_diagnostic(
            DiagnosticCode::VERSION_INVALID,
            SourceSpan::new(bom_len, bom_len + line.len()),
        )]
    })?;
    if !FCS_SOURCE_VERSION.supports_source(version) {
        return Err(vec![version_diagnostic(
            DiagnosticCode::VERSION_UNSUPPORTED,
            SourceSpan::new(bom_len, bom_len + line.len()),
        )]);
    }
    let offset = bom_len + line_end + 1;
    Ok(HeaderPrefix {
        header: Some((version, ChumskySpan::new((), bom_len..offset))),
        body: &source[offset..],
        offset,
    })
}

fn version_diagnostic(code: DiagnosticCode, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Parse, "invalid source version", span)
}

fn shift_diagnostic(diagnostic: Diagnostic, offset: usize) -> Diagnostic {
    let span = diagnostic.primary_span();
    Diagnostic::new(
        diagnostic.code(),
        diagnostic.stage(),
        diagnostic.message(),
        SourceSpan::new(span.start + offset, span.end + offset),
    )
}

fn finite_float(number: &str) -> Result<f64, &'static str> {
    let value = number.parse::<f64>().map_err(|_| "invalid float literal")?;
    value
        .is_finite()
        .then_some(value)
        .ok_or("non-finite numeric literal")
}

fn parse_beat(number: &str) -> Result<Beat, &'static str> {
    let (mantissa, exponent) = if let Some(index) = number.find(['e', 'E']) {
        (
            &number[..index],
            number[index + 1..]
                .parse::<i32>()
                .map_err(|_| "invalid beat literal")?,
        )
    } else {
        (number, 0)
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut numerator = format!("{whole}{fraction}")
        .parse::<i128>()
        .map_err(|_| "invalid beat literal")?;
    let mut denominator = 10_i128
        .checked_pow(u32::try_from(fraction.len()).map_err(|_| "invalid beat literal")?)
        .ok_or("invalid beat literal")?;
    if exponent >= 0 {
        numerator = numerator
            .checked_mul(
                10_i128
                    .checked_pow(exponent.unsigned_abs())
                    .ok_or("invalid beat literal")?,
            )
            .ok_or("invalid beat literal")?;
    } else {
        denominator = denominator
            .checked_mul(
                10_i128
                    .checked_pow(exponent.unsigned_abs())
                    .ok_or("invalid beat literal")?,
            )
            .ok_or("invalid beat literal")?;
    }
    if numerator == 0 {
        return Beat::new(0, 1).map_err(|_| "invalid beat literal");
    }
    let divisor = gcd(numerator.unsigned_abs(), denominator.unsigned_abs()) as i128;
    Beat::new(
        i64::try_from(numerator / divisor).map_err(|_| "invalid beat literal")?,
        i64::try_from(denominator / divisor).map_err(|_| "invalid beat literal")?,
    )
    .map_err(|_| "invalid beat literal")
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn validate_trivia(source: &str, limits: ParseLimits) -> Option<Diagnostic> {
    let mut offset = 0;
    let mut depth = 0usize;
    let mut string = false;
    let mut escaped = false;
    while offset < source.len() {
        let rest = &source[offset..];
        if string {
            let character = rest.chars().next()?;
            if character == '\r' || character == '\n' {
                return Some(syntax(
                    DiagnosticCode::SYNTAX_UNCLOSED_STRING,
                    SourceSpan::new(offset, offset + character.len_utf8()),
                ));
            }
            offset += character.len_utf8();
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                string = false;
            }
            continue;
        }
        if depth > 0 {
            if rest.starts_with("/*") {
                depth += 1;
                if depth > limits.max_comment_depth {
                    return Some(resource_limit(SourceSpan::new(offset, offset + 2)));
                }
                offset += 2;
            } else if rest.starts_with("*/") {
                depth -= 1;
                offset += 2;
            } else {
                offset += rest.chars().next()?.len_utf8();
            }
            continue;
        }
        if rest.starts_with("//") {
            offset = rest
                .find('\n')
                .map_or(source.len(), |index| offset + index + 1);
        } else if rest.starts_with("/*") {
            depth = 1;
            if depth > limits.max_comment_depth {
                return Some(resource_limit(SourceSpan::new(offset, offset + 2)));
            }
            offset += 2;
        } else {
            let character = rest.chars().next()?;
            if character == '"' {
                string = true;
            } else if character == '\u{feff}' && offset != 0 {
                return Some(syntax(
                    DiagnosticCode::SYNTAX_INVALID_TOKEN,
                    SourceSpan::new(offset, offset + character.len_utf8()),
                ));
            } else if character == '#' {
                let end = color_candidate_end(source, offset);
                let candidate = &source[offset..end];
                if !(candidate.len() == 7 || candidate.len() == 9)
                    || !candidate[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
                {
                    return Some(syntax(
                        DiagnosticCode::SYNTAX_INVALID_TOKEN,
                        SourceSpan::new(offset, end),
                    ));
                }
            } else if character.is_ascii_digit()
                && let Some(end) = malformed_exponent_end(source, offset)
            {
                return Some(syntax(
                    DiagnosticCode::SYNTAX_INVALID_TOKEN,
                    SourceSpan::new(offset, end),
                ));
            }
            offset += character.len_utf8();
        }
    }
    if depth > 0 {
        Some(syntax(
            DiagnosticCode::SYNTAX_UNCLOSED_COMMENT,
            SourceSpan::new(source.len(), source.len()),
        ))
    } else if string {
        Some(syntax(
            DiagnosticCode::SYNTAX_UNCLOSED_STRING,
            SourceSpan::new(source.len(), source.len()),
        ))
    } else {
        None
    }
}

fn malformed_exponent_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut end = start;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if bytes.get(end) == Some(&b'.') {
        end += 1;
        while bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }
    }
    if !matches!(bytes.get(end), Some(b'e' | b'E')) {
        return None;
    }
    end += 1;
    if matches!(bytes.get(end), Some(b'+' | b'-')) {
        end += 1;
    }
    let digits_start = end;
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    (end == digits_start).then_some(end)
}

fn color_candidate_end(source: &str, start: usize) -> usize {
    let mut end = start + 1;
    while let Some(character) = source[end..].chars().next() {
        if !character.is_ascii_alphanumeric() {
            break;
        }
        end += character.len_utf8();
    }
    end
}

fn nesting_limit(tokens: &[SpannedToken], maximum: usize) -> Option<SourceSpan> {
    let mut depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(
                Punctuation::LeftParenthesis | Punctuation::LeftBracket | Punctuation::LeftBrace,
            ) => {
                depth += 1;
                if depth > maximum {
                    return Some(source_span(*span));
                }
            }
            Token::Punctuation(
                Punctuation::RightParenthesis | Punctuation::RightBracket | Punctuation::RightBrace,
            ) => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn rich_diagnostic(error: Rich<'_, char, ChumskySpan>) -> Diagnostic {
    let code = match error.reason() {
        RichReason::Custom(message) if message == "non-finite numeric literal" => {
            DiagnosticCode::NUMERIC_NON_FINITE
        }
        _ => DiagnosticCode::SYNTAX_INVALID_TOKEN,
    };
    syntax(code, source_span(*error.span()))
}
fn syntax(code: DiagnosticCode, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Parse, "invalid source syntax", span)
}
fn resource_limit(span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
        DiagnosticStage::Parse,
        "parser resource limit exceeded",
        span,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostic::DiagnosticCode;

    fn tokens(source: &str) -> Vec<SpannedToken> {
        lex(source, ParseLimits::default()).expect("source should lex")
    }

    #[test]
    fn longest_match_distinguishes_range_operators() {
        assert!(matches!(
            tokens("0beat..<4beat")[1].0,
            Token::Punctuation(Punctuation::RangeExclusive)
        ));
        assert!(matches!(
            tokens("0beat..=4beat")[1].0,
            Token::Punctuation(Punctuation::RangeInclusive)
        ));
    }
    #[test]
    fn bare_range_is_not_two_dot_tokens() {
        assert_eq!(
            lex("0beat..4beat", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN
        );
    }
    #[test]
    fn unicode_spans_are_utf8_byte_offsets() {
        let source = "\"雪\" + value";
        let tokens = tokens(source);
        assert_eq!(source_span(tokens[0].1), SourceSpan::new(0, "\"雪\"".len()));
        assert_eq!(source_span(tokens[2].1).start, "\"雪\" + ".len());
    }
    #[test]
    fn nested_comments_and_string_escapes_are_deterministic() {
        let tokens = tokens("/* outer /* inner */ end */ \"a\\n\\u{96ea}\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(tokens[0].0, Token::Literal(_)));
    }
    #[test]
    fn trivia_only_input_produces_an_empty_token_stream() {
        assert!(tokens(" \t\r\n/* comment */ // trailing").is_empty());
    }
    #[test]
    fn lexer_contract_diagnostics_and_limits_are_stable() {
        assert!(lex("format { profile: fragment; }", ParseLimits::default()).is_ok());
        let HeaderPrefix { body, .. } = header_prefix("#fcs 5.0.0\nformat { profile: fragment; }")
            .expect("valid header prefix");
        assert_eq!(body, "format { profile: fragment; }");
        lex(
            "#fcs 5.0.0\nformat { profile: fragment; }",
            ParseLimits::default(),
        )
        .expect("document header and body should lex");
        for source in ["\u{feff}1", "1\r\n2"] {
            assert!(lex(source, ParseLimits::default()).is_ok());
        }
        assert_eq!(
            lex("/* unclosed", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::SYNTAX_UNCLOSED_COMMENT
        );
        assert_eq!(
            lex("\"unclosed", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::SYNTAX_UNCLOSED_STRING
        );
        assert_eq!(
            lex("\"\\q\"", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN
        );
        assert_eq!(
            lex("#12345", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::SYNTAX_INVALID_TOKEN
        );
        assert_eq!(
            lex("1e9999", ParseLimits::default()).unwrap_err()[0].code(),
            DiagnosticCode::NUMERIC_NON_FINITE
        );
        for source in ["1e", "1e+"] {
            assert_eq!(
                lex(source, ParseLimits::default()).unwrap_err()[0].code(),
                DiagnosticCode::SYNTAX_INVALID_TOKEN,
                "{source}"
            );
        }
        assert!(matches!(tokens("null")[0].0, Token::Keyword(Keyword::Null)));
        assert_eq!(
            lex(
                "x",
                ParseLimits {
                    max_source_bytes: 0,
                    ..ParseLimits::default()
                }
            )
            .unwrap_err()[0]
                .code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
        assert_eq!(
            lex(
                "x",
                ParseLimits {
                    max_tokens: 0,
                    ..ParseLimits::default()
                }
            )
            .unwrap_err()[0]
                .code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
        assert_eq!(
            lex(
                "\"x\"",
                ParseLimits {
                    max_literal_bytes: 2,
                    ..ParseLimits::default()
                }
            )
            .unwrap_err()[0]
                .code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
        assert_eq!(
            lex(
                "/* /* */ */",
                ParseLimits {
                    max_comment_depth: 1,
                    ..ParseLimits::default()
                }
            )
            .unwrap_err()[0]
                .code(),
            DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
        );
    }
}
