use chumsky::error::RichReason;
use chumsky::inspector::RollbackState;
use chumsky::prelude::*;

use crate::ast::{Beat, Bpm, Color, SourceLiteral, SourceSpan};
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
const INVALID_NUMERIC: &str = "invalid numeric literal";

#[derive(Debug, Clone)]
struct LexerState {
    comment_depth: usize,
    max_comment_depth: usize,
}

pub(super) fn lex(source: &str, limits: ParseLimits) -> Result<Vec<SpannedToken>, Vec<Diagnostic>> {
    if source.len() > limits.max_source_bytes {
        return Err(vec![resource_limit(SourceSpan::new(0, source.len()))]);
    }
    let mut state = RollbackState(LexerState {
        comment_depth: 0,
        max_comment_depth: limits.max_comment_depth,
    });
    let (tokens, errors) = lexer()
        .parse_with_state(source, &mut state)
        .into_output_errors();
    if !errors.is_empty() {
        return Err(errors.into_iter().map(rich_diagnostic).collect());
    }
    let (has_bom, tokens) = tokens.expect("a complete lexer produces tokens when it has no errors");
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

fn lexer<'source>()
-> impl Parser<'source, &'source str, (bool, Vec<SpannedToken>), LexerExtra<'source>> {
    let digit = one_of("0123456789");
    let integer = just('0')
        .ignored()
        .or(one_of("123456789").then(digit.repeated()).ignored())
        .ignored();
    let fraction = just('.').then(digit.repeated().at_least(1)).ignored();
    let exponent = choice((
        one_of("eE")
            .then(one_of("+-").or_not())
            .then(digit.repeated().at_least(1))
            .ignored(),
        one_of("eE")
            .then(one_of("+-").or_not())
            .try_map(|_, span| Err::<(), _>(Rich::custom(span, INVALID_NUMERIC))),
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
    let number = integer
        .then(fraction.or_not())
        .then(exponent)
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
                        .ok_or_else(|| Rich::custom(span, "invalid unicode escape"))
                })
                .map_err(|error| preserve_custom(error, "invalid unicode escape")),
        ),
        any().try_map(|_, span| Err(Rich::custom(span, "invalid string escape"))),
    )));
    let string = just('"').ignore_then(
        escape
            .or(none_of("\\\"\r\n"))
            .repeated()
            .collect::<String>()
            .then_ignore(just('"'))
            .map(|value| Token::Literal(SourceLiteral::String(value)))
            .map_err(|error| preserve_custom(error, UNCLOSED_STRING)),
    );

    let color_digits = any()
        .filter(|character: &char| character.is_ascii_alphanumeric())
        .repeated()
        .collect::<String>();
    let color = just('#').ignore_then(color_digits).try_map(|digits, span| {
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
        just('/')
            .then_ignore(one_of("*/").not())
            .to(Token::Punctuation(Punctuation::Slash)),
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
        let open = just("/*")
            .map_with(|_, extra| extra.span())
            .try_map_with(|span, extra| {
                let state: &mut RollbackState<LexerState> = extra.state();
                state.comment_depth += 1;
                if state.comment_depth > state.max_comment_depth {
                    Err(Rich::custom(span, RESOURCE_LIMIT))
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
                any()
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

    let token = choice((string, color, number, identifier, punctuation))
        .map_with(|token, extra| (token, extra.span()));
    let line_ending = choice((just("\r\n").to(true), just('\n').to(true), end().to(false)));
    let header = just("#fcs ")
        .ignore_then(none_of("\r\n").repeated().collect::<String>())
        .then(line_ending)
        .map_with(|(version, terminated), extra| {
            let span: ChumskySpan = extra.span();
            match version.parse::<Version>() {
                Ok(version) if terminated && FCS_SOURCE_VERSION.supports_source(version) => {
                    (Token::Header(version), span)
                }
                Ok(_) if terminated => (Token::UnsupportedVersion, span),
                _ => (Token::InvalidVersion, span),
            }
        });
    let leading_bom = just('\u{feff}').or_not().map(|bom| bom.is_some());
    let first_token = choice((header, token.clone()));
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
    match error.reason() {
        RichReason::Custom(message) if message == UNCLOSED_COMMENT => {
            syntax(DiagnosticCode::SYNTAX_UNCLOSED_COMMENT, span)
        }
        RichReason::Custom(message) if message == UNCLOSED_STRING => {
            syntax(DiagnosticCode::SYNTAX_UNCLOSED_STRING, span)
        }
        RichReason::Custom(message) if message == RESOURCE_LIMIT => resource_limit(span),
        RichReason::Custom(message) if message == "non-finite numeric literal" => {
            syntax(DiagnosticCode::NUMERIC_NON_FINITE, span)
        }
        _ => syntax(DiagnosticCode::SYNTAX_INVALID_TOKEN, span),
    }
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
