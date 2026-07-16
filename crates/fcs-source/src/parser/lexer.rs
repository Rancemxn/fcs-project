use chumsky::error::RichReason;
use chumsky::inspector::RollbackState;
use chumsky::prelude::*;

use crate::ast::{Beat, Color, SourceBpm, SourceLiteral, SourceSpan};
use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticStage};
use crate::version::{FCS_SOURCE_VERSION, Version};

use super::ParseLimits;
use super::input::{ChumskySpan, SpannedToken, source_span};
use super::token::{Keyword, Punctuation, Token};

type LexerExtra<'source> =
    extra::Full<Rich<'source, char, ChumskySpan>, RollbackState<LexerState>, ()>;

const UNCLOSED_COMMENT: &str = "unclosed block comment";
const UNCLOSED_STRING: &str = "unclosed string literal";
const RESOURCE_LIMIT: &str = "parser resource limit exceeded";
const FORBIDDEN_SOURCE_SCALAR: &str = "forbidden source scalar";

#[derive(Debug, Clone)]
struct LexerState {
    comment_depth: usize,
    max_comment_depth: usize,
    token_count: usize,
    max_tokens: usize,
    max_token_bytes: usize,
    literal_bytes: usize,
    max_literal_bytes: usize,
}

pub(super) fn lex(source: &str, limits: ParseLimits) -> Result<Vec<SpannedToken>, Vec<Diagnostic>> {
    lex_with_header_policy(source, limits, false)
}

pub(super) fn lex_document(
    source: &str,
    limits: ParseLimits,
) -> Result<Vec<SpannedToken>, Vec<Diagnostic>> {
    lex_with_header_policy(source, limits, true)
}

fn lex_with_header_policy(
    source: &str,
    limits: ParseLimits,
    require_header_at_start: bool,
) -> Result<Vec<SpannedToken>, Vec<Diagnostic>> {
    if source.len() > limits.max_source_bytes {
        return Err(vec![resource_limit(
            "max_source_bytes",
            limits.max_source_bytes,
            source.len(),
            SourceSpan::new(0, source.len()),
        )]);
    }
    let mut state = RollbackState(LexerState {
        comment_depth: 0,
        max_comment_depth: limits.max_comment_depth,
        token_count: 0,
        max_tokens: limits.max_tokens,
        max_token_bytes: limits.max_token_bytes,
        literal_bytes: 0,
        max_literal_bytes: limits.max_literal_bytes,
    });
    let (tokens, errors) = lexer()
        .parse_with_state(source, &mut state)
        .into_output_errors();
    if !errors.is_empty() {
        return Err(errors.into_iter().map(rich_diagnostic).collect());
    }
    let (has_bom, tokens) = tokens.expect("a complete lexer produces tokens when it has no errors");
    let header_start = usize::from(has_bom) * '\u{feff}'.len_utf8();
    if require_header_at_start
        && matches!(tokens.first(), Some((Token::Header(_), span)) if span.start != header_start)
    {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::VERSION_MISSING_HEADER,
            DiagnosticStage::Parse,
            "source is missing an #fcs header",
            SourceSpan::new(header_start, header_start),
        )]);
    }
    if let Some((token, span)) = tokens.first() {
        let code = match token {
            Token::InvalidVersion => Some(DiagnosticCode::VERSION_INVALID),
            Token::UnsupportedVersion => Some(DiagnosticCode::VERSION_UNSUPPORTED),
            _ => None,
        };
        if let Some(code) = code {
            let span = source_span(*span);
            let span = if has_bom {
                SourceSpan::new(0, span.end)
            } else {
                span
            };
            return Err(vec![version_diagnostic(code, span)]);
        }
    }
    if let Some((
        Token::ResourceLimit {
            kind,
            limit,
            observed,
        },
        span,
    )) = tokens
        .iter()
        .find(|(token, _)| matches!(token, Token::ResourceLimit { .. }))
    {
        return Err(vec![resource_limit(
            kind,
            *limit,
            *observed,
            source_span(*span),
        )]);
    }
    if let Some((_, span)) = tokens.iter().find(|(token, _)| {
        matches!(
            token,
            Token::InvalidLexeme | Token::InvalidSemver | Token::InvalidColor
        )
    }) {
        return Err(vec![syntax(
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            source_span(*span),
        )]);
    }
    if let Some((token, span)) = tokens
        .iter()
        .find(|(token, _)| matches!(token, Token::InvalidNumeric | Token::NonFiniteNumeric))
    {
        let code = if *token == Token::NonFiniteNumeric {
            DiagnosticCode::NUMERIC_NON_FINITE
        } else {
            DiagnosticCode::SYNTAX_INVALID_TOKEN
        };
        return Err(vec![syntax(code, source_span(*span))]);
    }
    if let Some((span, observed)) = nesting_limit(&tokens, limits.max_nesting_depth) {
        return Err(vec![resource_limit(
            "max_nesting_depth",
            limits.max_nesting_depth,
            observed,
            span,
        )]);
    }
    Ok(tokens)
}

fn lexer<'source>()
-> impl Parser<'source, &'source str, (bool, Vec<SpannedToken>), LexerExtra<'source>> {
    let source_character = any().try_map(|character, span| {
        if is_forbidden_source_scalar(character) {
            Err(Rich::custom(span, FORBIDDEN_SOURCE_SCALAR))
        } else {
            Ok(character)
        }
    });
    let digit = one_of("0123456789");
    let version_component = digit.repeated().at_least(1);
    let semver = version_component
        .then_ignore(just('.'))
        .then(version_component)
        .then_ignore(just('.'))
        .then(version_component)
        .to_slice()
        .map_with(|text: &str, extra| {
            let state: &mut RollbackState<LexerState> = extra.state();
            if text.len() > state.max_token_bytes {
                return Token::ResourceLimit {
                    kind: "max_token_bytes",
                    limit: state.max_token_bytes,
                    observed: state.max_token_bytes.saturating_add(1),
                };
            }
            text.parse::<Version>()
                .map(Token::Semver)
                .unwrap_or(Token::InvalidSemver)
        });
    let integer = digit.repeated().at_least(1).ignored();
    let fraction = just('.').then(digit.repeated().at_least(1)).ignored();
    let exponent = choice((
        one_of("eE")
            .then(one_of("+-").or_not())
            .then(digit.repeated())
            .ignored(),
        one_of("eE").not().ignored(),
    ));
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
    let adjacent_identifier_continuation = any()
        .filter(|character: &char| character.is_ascii_alphanumeric() || *character == '_')
        .repeated()
        .ignored();
    let number = integer
        .then(fraction.or_not())
        .then(exponent)
        .then(unit)
        .then(adjacent_identifier_continuation)
        .to_slice()
        .map_with(|text: &str, extra| {
            if let Some(limit) = literal_limit_token(text.len(), extra.state()) {
                return limit;
            }
            parse_number_token(text).unwrap_or_else(|message| {
                if message == "non-finite numeric literal" {
                    Token::NonFiniteNumeric
                } else {
                    Token::InvalidNumeric
                }
            })
        });

    let escape = just('\\').ignore_then(choice((
        just('n').to('\n'),
        just('r').to('\r'),
        just('t').to('\t'),
        just('\\').to('\\'),
        just('"').to('"'),
        just('0').to('\0'),
        just('u').ignore_then(
            just('{')
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
                        .filter(|character| !is_unicode_noncharacter(*character))
                        .ok_or_else(|| Rich::custom(span, "invalid unicode escape"))
                })
                .map_err(|error| preserve_custom(error, "invalid unicode escape")),
        ),
        any().try_map(|_, span| Err(Rich::custom(span, "invalid string escape"))),
    )));
    let string_character = none_of("\\\"\r\n").try_map(|character, span| {
        if is_forbidden_source_scalar(character) {
            Err(Rich::custom(span, FORBIDDEN_SOURCE_SCALAR))
        } else {
            Ok(character)
        }
    });
    let string_open =
        just('"')
            .map_with(|_, extra| extra.span())
            .try_map_with(|span: ChumskySpan, extra| {
                let state: &mut RollbackState<LexerState> = extra.state();
                state.literal_bytes = 0;
                charge_literal_bytes(span, state)
            });
    let string_piece = escape
        .or(string_character)
        .map_with(|character, extra| (character, extra.span()))
        .try_map_with(|(character, span), extra| {
            charge_literal_bytes(span, extra.state())?;
            Ok(character)
        });
    let string_close = just('"')
        .map_with(|_, extra| extra.span())
        .try_map_with(|span, extra| charge_literal_bytes(span, extra.state()));
    let string_body = string_piece
        .repeated()
        .collect::<String>()
        .then_ignore(string_close)
        .map_err(|error| preserve_custom(error, UNCLOSED_STRING));
    let string = string_open
        .ignore_then(string_body)
        .map(|value| Token::Literal(SourceLiteral::String(value)));

    let color_digits = any()
        .filter(|character: &char| character.is_ascii_alphanumeric())
        .repeated();
    let color = just('#')
        .ignore_then(color_digits)
        .to_slice()
        .map_with(|text: &str, extra| {
            if let Some(limit) = literal_limit_token(text.len(), extra.state()) {
                return limit;
            }
            text.parse::<Color>()
                .map(SourceLiteral::Color)
                .map(Token::Literal)
                .unwrap_or(Token::InvalidColor)
        });

    let identifier = text::ascii::ident().map_with(|identifier: &str, extra| {
        let state: &mut RollbackState<LexerState> = extra.state();
        if identifier.len() > state.max_token_bytes {
            return Token::ResourceLimit {
                kind: "max_token_bytes",
                limit: state.max_token_bytes,
                observed: state.max_token_bytes.saturating_add(1),
            };
        }
        match identifier {
            "true" => Token::Literal(SourceLiteral::Bool(true)),
            "false" => Token::Literal(SourceLiteral::Bool(false)),
            identifier => Keyword::from_identifier(identifier)
                .map_or_else(|| Token::Identifier(identifier.to_owned()), Token::Keyword),
        }
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
        just("..").to(Token::InvalidLexeme),
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
        just('/')
            .then_ignore(one_of("*/").not())
            .to(Token::Punctuation(Punctuation::Slash)),
        just('%').to(Token::Punctuation(Punctuation::Percent)),
        just('!').to(Token::Punctuation(Punctuation::Bang)),
        just('=').to(Token::Punctuation(Punctuation::Equal)),
        just('<').to(Token::Punctuation(Punctuation::LessThan)),
        just('>').to(Token::Punctuation(Punctuation::GreaterThan)),
    ));
    let invalid_punctuation = one_of("&|?").to(Token::InvalidLexeme);
    let punctuation = choice((
        compound_punctuation,
        simple_punctuation,
        invalid_punctuation,
    ));

    let line_comment = just("//")
        .ignore_then(source_character.and_is(just('\n').not()).repeated())
        .ignored();
    let block_comment = recursive(|comment| {
        let open = just("/*")
            .map_with(|_, extra| extra.span())
            .try_map_with(|span, extra| {
                let state: &mut RollbackState<LexerState> = extra.state();
                state.comment_depth += 1;
                if state.comment_depth > state.max_comment_depth {
                    Err(Rich::custom(
                        span,
                        resource_limit_message(
                            "max_comment_depth",
                            state.max_comment_depth,
                            state.comment_depth,
                        ),
                    ))
                } else {
                    Ok(())
                }
            });
        let close = just("*/").map_with(|_, extra| {
            let state: &mut RollbackState<LexerState> = extra.state();
            state.comment_depth -= 1;
        });
        open.ignore_then(
            choice((
                comment,
                source_character
                    .and_is(just("/*").not())
                    .and_is(just("*/").not())
                    .ignored(),
            ))
            .repeated()
            .then_ignore(close)
            .map_err(|error| preserve_custom(error, UNCLOSED_COMMENT)),
        )
        .ignored()
    });
    let trivia = choice((line_comment, block_comment, one_of(" \t\r\n").ignored())).repeated();

    let raw_token = choice((string, color, identifier, semver, number, punctuation))
        .map_with(|token, extra| (token, extra.span()))
        .try_map_with(|token, extra| enforce_literal_token(token, extra.state()));
    let line_ending = choice((
        just("\r\n").ignored(),
        just('\n').ignored(),
        end().ignored(),
    ));
    let header = just("#fcs ")
        .ignore_then(none_of("\r\n").repeated().to_slice())
        .then_ignore(line_ending)
        .map_with(|version: &str, extra| {
            let span: ChumskySpan = extra.span();
            let state: &mut RollbackState<LexerState> = extra.state();
            if span.end.saturating_sub(span.start) > state.max_token_bytes {
                return (
                    Token::ResourceLimit {
                        kind: "max_token_bytes",
                        limit: state.max_token_bytes,
                        observed: state.max_token_bytes.saturating_add(1),
                    },
                    span,
                );
            }
            match version.parse::<Version>() {
                Ok(version) if FCS_SOURCE_VERSION.supports_source(&version) => {
                    (Token::Header(version), span)
                }
                Ok(_) => (Token::UnsupportedVersion, span),
                _ => (Token::InvalidVersion, span),
            }
        });
    let malformed_header = just("#fcs")
        .ignore_then(none_of("\r\n").repeated())
        .then_ignore(choice((
            just("\r\n").ignored(),
            just('\n').ignored(),
            just('\r').ignored(),
            end().ignored(),
        )))
        .map_with(|_, extra| extra.span())
        .map_with(|span: ChumskySpan, extra| {
            let state: &mut RollbackState<LexerState> = extra.state();
            let token = if span.end.saturating_sub(span.start) > state.max_token_bytes {
                Token::ResourceLimit {
                    kind: "max_token_bytes",
                    limit: state.max_token_bytes,
                    observed: state.max_token_bytes.saturating_add(1),
                }
            } else {
                Token::InvalidVersion
            };
            (token, span)
        });
    let leading_bom = just('\u{feff}').or_not().map(|bom| bom.is_some());
    let token_guard = empty()
        .map_with(|_, extra| extra.span())
        .try_map_with(|span, extra| begin_token(span, extra.state()));
    let first_token =
        token_guard.ignore_then(choice((header, malformed_header, raw_token.clone())));
    let token = token_guard.ignore_then(raw_token);
    leading_bom
        .then(trivia.clone())
        .then(choice((
            first_token
                .then_ignore(trivia.clone())
                .then(token.then_ignore(trivia).repeated().collect::<Vec<_>>())
                .map(|(first, mut tokens)| {
                    tokens.insert(0, first);
                    tokens
                }),
            end().to(Vec::new()),
        )))
        .then_ignore(end())
        .map(|((has_bom, ()), tokens)| (has_bom, tokens))
}

fn begin_token<'source>(
    span: ChumskySpan,
    state: &mut RollbackState<LexerState>,
) -> Result<(), Rich<'source, char, ChumskySpan>> {
    state.token_count = state.token_count.saturating_add(1);
    if state.token_count > state.max_tokens {
        Err(Rich::custom(
            span,
            resource_limit_message("max_tokens", state.max_tokens, state.token_count),
        ))
    } else {
        Ok(())
    }
}

fn enforce_literal_token<'source>(
    token: SpannedToken,
    state: &mut RollbackState<LexerState>,
) -> Result<SpannedToken, Rich<'source, char, ChumskySpan>> {
    if matches!(&token.0, Token::ResourceLimit { .. }) {
        return Ok(token);
    }
    enforce_token_length(token.1.end.saturating_sub(token.1.start), token.1, state)?;
    if matches!(
        &token.0,
        Token::Literal(_) | Token::TempoBpm(_) | Token::InvalidNumeric | Token::NonFiniteNumeric
    ) {
        enforce_literal_length(token.1.end.saturating_sub(token.1.start), token.1, state)?;
    }
    Ok(token)
}

fn literal_limit_token(length: usize, state: &RollbackState<LexerState>) -> Option<Token> {
    if length > state.max_token_bytes {
        Some(Token::ResourceLimit {
            kind: "max_token_bytes",
            limit: state.max_token_bytes,
            observed: state.max_token_bytes.saturating_add(1),
        })
    } else if length > state.max_literal_bytes {
        Some(Token::ResourceLimit {
            kind: "max_literal_bytes",
            limit: state.max_literal_bytes,
            observed: state.max_literal_bytes.saturating_add(1),
        })
    } else {
        None
    }
}

fn enforce_token_length<'source>(
    length: usize,
    span: ChumskySpan,
    state: &mut RollbackState<LexerState>,
) -> Result<(), Rich<'source, char, ChumskySpan>> {
    if length > state.max_token_bytes {
        Err(Rich::custom(
            span,
            resource_limit_message(
                "max_token_bytes",
                state.max_token_bytes,
                state.max_token_bytes.saturating_add(1),
            ),
        ))
    } else {
        Ok(())
    }
}

fn enforce_literal_length<'source>(
    length: usize,
    span: ChumskySpan,
    state: &mut RollbackState<LexerState>,
) -> Result<(), Rich<'source, char, ChumskySpan>> {
    if length > state.max_literal_bytes {
        Err(Rich::custom(
            span,
            resource_limit_message(
                "max_literal_bytes",
                state.max_literal_bytes,
                state.max_literal_bytes.saturating_add(1),
            ),
        ))
    } else {
        Ok(())
    }
}

fn charge_literal_bytes<'source>(
    span: ChumskySpan,
    state: &mut RollbackState<LexerState>,
) -> Result<(), Rich<'source, char, ChumskySpan>> {
    state.literal_bytes = state
        .literal_bytes
        .saturating_add(span.end.saturating_sub(span.start));
    if state.literal_bytes > state.max_token_bytes {
        return Err(Rich::custom(
            span,
            resource_limit_message(
                "max_token_bytes",
                state.max_token_bytes,
                state.max_token_bytes.saturating_add(1),
            ),
        ));
    }
    if state.literal_bytes > state.max_literal_bytes {
        Err(Rich::custom(
            span,
            resource_limit_message(
                "max_literal_bytes",
                state.max_literal_bytes,
                state.max_literal_bytes.saturating_add(1),
            ),
        ))
    } else {
        Ok(())
    }
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
        return match number.parse::<i64>() {
            Ok(value) => Ok(Token::Literal(SourceLiteral::Int(value))),
            Err(_) if number.bytes().all(|byte| byte.is_ascii_digit()) => Ok(Token::Literal(
                SourceLiteral::IntMagnitude(number.to_owned()),
            )),
            Err(_) => Err("invalid integer literal"),
        };
    }
    if unit == "beat" {
        return parse_beat(number)
            .map(SourceLiteral::Beat)
            .map(Token::Literal);
    }
    let value = finite_float(number)?;
    if unit == "bpm" {
        return Ok(Token::TempoBpm(SourceBpm::from_value(value)));
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

fn version_diagnostic(code: DiagnosticCode, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Parse, "invalid source version", span)
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

fn is_forbidden_source_scalar(character: char) -> bool {
    character == '\0' || is_unicode_noncharacter(character)
}

fn is_unicode_noncharacter(character: char) -> bool {
    let scalar = character as u32;
    (0xfdd0..=0xfdef).contains(&scalar) || scalar & 0xffff >= 0xfffe
}

fn nesting_limit(tokens: &[SpannedToken], maximum: usize) -> Option<(SourceSpan, usize)> {
    let mut depth = 0usize;
    for (token, span) in tokens {
        match token {
            Token::Punctuation(
                Punctuation::LeftParenthesis | Punctuation::LeftBracket | Punctuation::LeftBrace,
            ) => {
                depth += 1;
                if depth > maximum {
                    return Some((source_span(*span), depth));
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

fn preserve_custom<'source>(
    error: Rich<'source, char, ChumskySpan>,
    fallback: &'static str,
) -> Rich<'source, char, ChumskySpan> {
    if matches!(error.reason(), RichReason::Custom(_)) {
        error
    } else {
        Rich::custom(*error.span(), fallback)
    }
}

fn rich_diagnostic(error: Rich<'_, char, ChumskySpan>) -> Diagnostic {
    let span = source_span(*error.span());
    if let RichReason::Custom(message) = error.reason()
        && let Some((kind, limit, observed)) = parse_resource_limit_message(message)
    {
        return resource_limit(kind, limit, observed, span);
    }
    match error.reason() {
        RichReason::Custom(message) if message == UNCLOSED_COMMENT => {
            syntax(DiagnosticCode::SYNTAX_UNCLOSED_COMMENT, span)
        }
        RichReason::Custom(message) if message == UNCLOSED_STRING => {
            syntax(DiagnosticCode::SYNTAX_UNCLOSED_STRING, span)
        }
        RichReason::Custom(message) if message == "non-finite numeric literal" => {
            syntax(DiagnosticCode::NUMERIC_NON_FINITE, span)
        }
        _ => syntax(DiagnosticCode::SYNTAX_INVALID_TOKEN, span),
    }
}

fn syntax(code: DiagnosticCode, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(code, DiagnosticStage::Parse, "invalid source syntax", span)
}
fn resource_limit_message(kind: &str, limit: usize, observed: usize) -> String {
    format!("{RESOURCE_LIMIT}|{kind}|{limit}|{observed}")
}

fn parse_resource_limit_message(message: &str) -> Option<(&str, usize, usize)> {
    let mut fields = message
        .strip_prefix(RESOURCE_LIMIT)?
        .strip_prefix('|')?
        .split('|');
    let kind = fields.next()?;
    let limit = fields.next()?.parse().ok()?;
    let observed = fields.next()?.parse().ok()?;
    fields.next().is_none().then_some((kind, limit, observed))
}

fn resource_limit(kind: &str, limit: usize, observed: usize, span: SourceSpan) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED,
        DiagnosticStage::Parse,
        "parser resource limit exceeded",
        span,
    )
    .with_budget(kind, limit, observed)
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
    fn core_envelope_and_resource_words_are_reserved() {
        for source in [
            "person",
            "credit",
            "scrollTempoMap",
            "cubicBezier",
            "extension",
            "required",
            "optional",
            "source",
            "payload",
            "audio",
            "image",
            "font",
            "texture",
            "path",
            "shader",
            "binary",
        ] {
            assert!(
                matches!(tokens(source).as_slice(), [(Token::Keyword(_), _)]),
                "{source} must be a Core reserved word"
            );
        }
    }

    #[test]
    fn document_structure_words_are_reserved() {
        for source in ["profile", "features", "tempoMap"] {
            assert!(
                matches!(tokens(source).as_slice(), [(Token::Keyword(_), _)]),
                "{source} must be a Core reserved word"
            );
        }
    }

    #[test]
    fn closed_enum_spellings_remain_identifiers() {
        for source in [
            "custom",
            "above",
            "below",
            "replace",
            "add",
            "multiply",
            "base",
            "zero",
            "one",
            "error",
            "holdBefore",
            "holdAfter",
        ] {
            assert_eq!(
                tokens(source).as_slice(),
                [(
                    Token::Identifier(source.to_owned()),
                    ChumskySpan::new((), 0..source.len())
                )],
                "{source} is not a Core reserved word"
            );
        }
    }

    #[test]
    fn standalone_semver_is_one_token_before_float() {
        assert_eq!(
            tokens("1.0.0").as_slice(),
            [(
                Token::Semver(Version::new(1, 0, 0)),
                ChumskySpan::new((), 0..5)
            )]
        );
        assert!(matches!(
            tokens("1.0").as_slice(),
            [(Token::Literal(SourceLiteral::Float(value)), _)] if *value == 1.0
        ));

        let source = "65536.0.0";
        let tokens = tokens(source);
        assert!(matches!(tokens.as_slice(), [(Token::Semver(_), _)]));
        let Token::Semver(version) = &tokens[0].0 else {
            unreachable!()
        };
        assert_eq!(version.to_string(), source);
        assert_eq!(tokens[0].1, ChumskySpan::new((), 0..source.len()));
    }

    #[test]
    fn leading_zero_standalone_semver_is_one_invalid_lexeme() {
        for source in ["01.0.0", "1.00.0", "1.0.00"] {
            let diagnostics = lex(source, ParseLimits::default()).unwrap_err();
            assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
            assert_eq!(
                diagnostics[0].primary_span(),
                SourceSpan::new(0, source.len()),
                "{source}"
            );
        }
    }

    #[test]
    fn every_core_reserved_word_has_one_keyword_token() {
        for (source, expected) in [
            ("format", Keyword::Format),
            ("profile", Keyword::Profile),
            ("features", Keyword::Features),
            ("fragment", Keyword::Fragment),
            ("chart", Keyword::Chart),
            ("playable", Keyword::Playable),
            ("renderable", Keyword::Renderable),
            ("publishable", Keyword::Publishable),
            ("meta", Keyword::Meta),
            ("contributors", Keyword::Contributors),
            ("person", Keyword::Person),
            ("credits", Keyword::Credits),
            ("credit", Keyword::Credit),
            ("resources", Keyword::Resources),
            ("artwork", Keyword::Artwork),
            ("sync", Keyword::Sync),
            ("definitions", Keyword::Definitions),
            ("const", Keyword::Const),
            ("let", Keyword::Let),
            ("fn", Keyword::Fn),
            ("template", Keyword::Template),
            ("return", Keyword::Return),
            ("if", Keyword::If),
            ("else", Keyword::Else),
            ("choose", Keyword::Choose),
            ("when", Keyword::When),
            ("generate", Keyword::Generate),
            ("emit", Keyword::Emit),
            ("in", Keyword::In),
            ("step", Keyword::Step),
            ("with", Keyword::With),
            ("null", Keyword::Null),
            ("tempoMap", Keyword::TempoMap),
            ("lines", Keyword::Lines),
            ("line", Keyword::Line),
            ("collections", Keyword::Collections),
            ("notes", Keyword::Notes),
            ("judgelines", Keyword::Judgelines),
            ("tracks", Keyword::Tracks),
            ("track", Keyword::Track),
            ("segments", Keyword::Segments),
            ("segment", Keyword::Segment),
            ("keyframe", Keyword::Keyframe),
            ("point", Keyword::Point),
            ("using", Keyword::Using),
            ("scrollTempoMap", Keyword::ScrollTempoMap),
            ("cubicBezier", Keyword::CubicBezier),
            ("render", Keyword::Render),
            ("extensions", Keyword::Extensions),
            ("extension", Keyword::Extension),
            ("required", Keyword::Required),
            ("optional", Keyword::Optional),
            ("preserve", Keyword::Preserve),
            ("source", Keyword::Source),
            ("payload", Keyword::Payload),
            ("audio", Keyword::Audio),
            ("image", Keyword::Image),
            ("font", Keyword::Font),
            ("texture", Keyword::Texture),
            ("path", Keyword::Path),
            ("shader", Keyword::Shader),
            ("binary", Keyword::Binary),
            ("bool", Keyword::Bool),
            ("int", Keyword::Int),
            ("float", Keyword::Float),
            ("string", Keyword::String),
            ("time", Keyword::Time),
            ("beat", Keyword::Beat),
            ("length", Keyword::Length),
            ("angle", Keyword::Angle),
            ("color", Keyword::Color),
            ("vec2", Keyword::Vec2),
            ("array", Keyword::Array),
            ("Note", Keyword::Note),
            ("Line", Keyword::LineType),
            ("RenderNode", Keyword::RenderNode),
            ("Track", Keyword::TrackType),
            ("TrackSegment", Keyword::TrackSegment),
            ("Keyframe", Keyword::KeyframeType),
            ("tap", Keyword::Tap),
            ("hold", Keyword::Hold),
            ("flick", Keyword::Flick),
            ("drag", Keyword::Drag),
        ] {
            assert_eq!(
                tokens(source).as_slice(),
                [(
                    Token::Keyword(expected),
                    ChumskySpan::new((), 0..source.len())
                )],
                "{source}"
            );
        }
        assert!(matches!(
            tokens("true").as_slice(),
            [(Token::Literal(SourceLiteral::Bool(true)), _)]
        ));
        assert!(matches!(
            tokens("false").as_slice(),
            [(Token::Literal(SourceLiteral::Bool(false)), _)]
        ));
    }

    #[test]
    fn punctuation_and_longest_match_tokens_are_complete() {
        for (source, expected) in [
            ("(", Punctuation::LeftParenthesis),
            (")", Punctuation::RightParenthesis),
            ("[", Punctuation::LeftBracket),
            ("]", Punctuation::RightBracket),
            ("{", Punctuation::LeftBrace),
            ("}", Punctuation::RightBrace),
            (",", Punctuation::Comma),
            (":", Punctuation::Colon),
            (";", Punctuation::Semicolon),
            (".", Punctuation::Dot),
            ("@", Punctuation::At),
            ("->", Punctuation::Arrow),
            ("=>", Punctuation::FatArrow),
            ("..<", Punctuation::RangeExclusive),
            ("..=", Punctuation::RangeInclusive),
            ("+", Punctuation::Plus),
            ("-", Punctuation::Minus),
            ("*", Punctuation::Star),
            ("**", Punctuation::Power),
            ("/", Punctuation::Slash),
            ("%", Punctuation::Percent),
            ("!", Punctuation::Bang),
            ("=", Punctuation::Equal),
            ("==", Punctuation::EqualEqual),
            ("!=", Punctuation::BangEqual),
            ("<", Punctuation::LessThan),
            ("<=", Punctuation::LessThanOrEqual),
            (">", Punctuation::GreaterThan),
            (">=", Punctuation::GreaterThanOrEqual),
            ("&&", Punctuation::AndAnd),
            ("||", Punctuation::OrOr),
        ] {
            assert_eq!(
                tokens(source).as_slice(),
                [(
                    Token::Punctuation(expected),
                    ChumskySpan::new((), 0..source.len())
                )],
                "{source}"
            );
        }
    }

    #[test]
    fn every_literal_and_unit_suffix_has_one_token() {
        for source in ["0", "42"] {
            assert!(
                matches!(
                    tokens(source).as_slice(),
                    [(Token::Literal(SourceLiteral::Int(_)), _)]
                ),
                "{source}"
            );
        }
        for source in ["1.0", "1e2", "1.0e-2"] {
            assert!(
                matches!(
                    tokens(source).as_slice(),
                    [(Token::Literal(SourceLiteral::Float(_)), _)]
                ),
                "{source}"
            );
        }
        for source in ["1ns", "1us", "1ms", "1s", "1min"] {
            assert!(
                matches!(
                    tokens(source).as_slice(),
                    [(Token::Literal(SourceLiteral::Time(_)), _)]
                ),
                "{source}"
            );
        }
        assert!(matches!(
            tokens("1beat").as_slice(),
            [(Token::Literal(SourceLiteral::Beat(_)), _)]
        ));
        assert!(matches!(
            tokens("1px").as_slice(),
            [(Token::Literal(SourceLiteral::Length(_)), _)]
        ));
        for source in ["1deg", "1rad", "1turn"] {
            assert!(
                matches!(
                    tokens(source).as_slice(),
                    [(Token::Literal(SourceLiteral::Angle(_)), _)]
                ),
                "{source}"
            );
        }
        for source in ["0bpm", "120bpm"] {
            assert!(
                matches!(tokens(source).as_slice(), [(Token::TempoBpm(_), _)]),
                "{source}"
            );
        }
        assert!(matches!(
            tokens("\"text\"").as_slice(),
            [(Token::Literal(SourceLiteral::String(_)), _)]
        ));
        assert!(matches!(
            tokens("#102030").as_slice(),
            [(Token::Literal(SourceLiteral::Color(_)), _)]
        ));
    }

    #[test]
    fn leading_minus_is_always_a_separate_token() {
        for source in ["-1", "-1.0", "-1beat", "-120bpm"] {
            let tokens = tokens(source);
            assert_eq!(tokens.len(), 2, "{source}");
            assert_eq!(
                tokens[0],
                (
                    Token::Punctuation(Punctuation::Minus),
                    ChumskySpan::new((), 0..1)
                ),
                "{source}"
            );
            assert_eq!(
                tokens[1].1,
                ChumskySpan::new((), 1..source.len()),
                "{source}"
            );
        }

        let compact = tokens("a-1")
            .into_iter()
            .map(|(token, _)| token)
            .collect::<Vec<_>>();
        let spaced = tokens("a - 1")
            .into_iter()
            .map(|(token, _)| token)
            .collect::<Vec<_>>();
        assert_eq!(compact, spaced);
    }

    #[test]
    fn contextual_render_words_and_keyword_fields_keep_core_token_kinds() {
        for source in [
            "viewport",
            "layer",
            "children",
            "group",
            "clipGroup",
            "rect",
            "roundedRect",
            "circle",
            "ellipse",
            "polyline",
            "polygon",
            "text",
        ] {
            assert!(
                matches!(tokens(source).as_slice(), [(Token::Identifier(_), _)]),
                "{source} is contextual to Render"
            );
        }

        assert_eq!(
            tokens("value.length,@other").as_slice(),
            [
                (
                    Token::Identifier("value".to_owned()),
                    ChumskySpan::new((), 0..5)
                ),
                (
                    Token::Punctuation(Punctuation::Dot),
                    ChumskySpan::new((), 5..6)
                ),
                (Token::Keyword(Keyword::Length), ChumskySpan::new((), 6..12)),
                (
                    Token::Punctuation(Punctuation::Comma),
                    ChumskySpan::new((), 12..13)
                ),
                (
                    Token::Punctuation(Punctuation::At),
                    ChumskySpan::new((), 13..14)
                ),
                (
                    Token::Identifier("other".to_owned()),
                    ChumskySpan::new((), 14..19)
                ),
            ]
        );
    }

    #[test]
    fn unsupported_standalone_punctuation_is_rejected() {
        for source in ["&", "|", "?", ".."] {
            let diagnostics = lex(source, ParseLimits::default()).unwrap_err();
            assert_eq!(diagnostics[0].code(), DiagnosticCode::SYNTAX_INVALID_TOKEN);
            assert_eq!(
                diagnostics[0].primary_span(),
                SourceSpan::new(0, source.len()),
                "{source}"
            );
        }
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
        let document_tokens = lex(
            "#fcs 5.0.0\nformat { profile: fragment; }",
            ParseLimits::default(),
        )
        .expect("document header and body should lex");
        assert!(matches!(document_tokens[0].0, Token::Header(_)));
        assert!(matches!(
            document_tokens[1].0,
            Token::Keyword(Keyword::Format)
        ));
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
