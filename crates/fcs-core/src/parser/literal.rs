//! Literal parsers — numbers with units, strings, colors, booleans.

use crate::ast::Literal;
use crate::units::{AngleUnit, Color, LengthUnit, TimeUnit, Unit};
use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while_m_n},
    character::complete::{char, digit1, one_of, satisfy},
    combinator::{map, map_res, opt, recognize, value},
    multi::many0,
    sequence::{pair, preceded, tuple},
    IResult,
};

// ---------------------------------------------------------------------------
// Whitespace & comments
// ---------------------------------------------------------------------------

/// Skip whitespace and line comments between tokens.
pub fn ws<'a>(input: &'a str) -> IResult<&'a str, ()> {
    let mut remaining = input;
    loop {
        let (rest, _matched) = opt(alt((
            value((), nom::character::complete::multispace1),
            value((), recognize(pair(tag("//"), take_while(|c| c != '\n')))),
        )))(remaining)?;
        if rest.len() == remaining.len() {
            // No progress — done
            break;
        }
        remaining = rest;
    }
    Ok((remaining, ()))
}

// ---------------------------------------------------------------------------
// Numbers with optional unit suffix
// ---------------------------------------------------------------------------

/// Parse any number (with optional leading `-` and optional decimal/exponent).
fn parse_number_loose(input: &str) -> IResult<&str, f64> {
    map_res(recognize(tuple((opt(char('-')), digit1, opt(tuple((char('.'), digit1))), opt(tuple((one_of("eE"), opt(one_of("+-")), digit1)))))), |s: &str| s.parse::<f64>())(input)
}

/// Parse a float literal — must have a decimal point or exponent.
fn parse_float(input: &str) -> IResult<&str, f64> {
    let (input, s) = recognize(tuple((digit1, alt((recognize(tuple((char('.'), digit1))), recognize(tuple((one_of("eE"), opt(one_of("+-")), digit1))))))))(input)?;
    let v = s.parse::<f64>().map_err(|_| nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Float)))?;
    Ok((input, v))
}

fn parse_integer(input: &str) -> IResult<&str, i64> {
    map_res(recognize(digit1), |s: &str| s.parse::<i64>())(input)
}

fn time_unit(input: &str) -> IResult<&str, TimeUnit> {
    alt((
        value(TimeUnit::Millisecond, tag("ms")),
        value(TimeUnit::Second, tag("s")),
        value(TimeUnit::Beat, tag("b")),
    ))(input)
}

fn length_unit(input: &str) -> IResult<&str, LengthUnit> {
    alt((
        value(LengthUnit::Pixel, tag("px")),
        value(LengthUnit::ViewportWidth, tag("vw")),
        value(LengthUnit::ViewportHeight, tag("vh")),
    ))(input)
}

fn angle_unit(input: &str) -> IResult<&str, AngleUnit> {
    alt((
        value(AngleUnit::Degree, tag("deg")),
        value(AngleUnit::Radian, tag("rad")),
    ))(input)
}

fn parse_quantified(input: &str) -> IResult<&str, Literal> {
    let (input, value) = parse_number_loose(input)?;
    let (input, unit) = alt((
        map(time_unit, Unit::Time),
        map(length_unit, Unit::Length),
        map(angle_unit, Unit::Angle),
    ))(input)?;
    Ok((input, Literal::Quantified { value, unit }))
}

pub fn parse_numeric_literal(input: &str) -> IResult<&str, Literal> {
    alt((
        parse_quantified,
        // Float must come before integer to avoid "1.0" being parsed as integer "1"
        map(parse_float, Literal::Float),
        map(parse_integer, Literal::Integer),
    ))(input)
}

// ---------------------------------------------------------------------------
// String literals (§2.4)
// ---------------------------------------------------------------------------

pub fn parse_string(input: &str) -> IResult<&str, String> {
    let (input, _) = char('"')(input)?;
    let (input, chars) = many0(string_fragment)(input)?;
    let (input, _) = char('"')(input)?;
    Ok((input, chars.concat()))
}

fn string_fragment(input: &str) -> IResult<&str, String> {
    alt((
        map(parse_escape, |c| c.to_string()),
        map(satisfy(|c| c != '"' && c != '\\' && c != '\n'), |c| c.to_string()),
    ))(input)
}

fn parse_escape(input: &str) -> IResult<&str, char> {
    preceded(
        char('\\'),
        alt((
            value('\n', char('n')),
            value('\t', char('t')),
            value('\\', char('\\')),
            value('"', char('"')),
            parse_unicode_escape,
        )),
    )(input)
}

fn parse_unicode_escape(input: &str) -> IResult<&str, char> {
    let (input, _) = tag("u{")(input)?;
    let (input, hex) = take_while_m_n(1, 6, |c: char| c.is_ascii_hexdigit())(input)?;
    let (input, _) = char('}')(input)?;
    let code = u32::from_str_radix(hex, 16)
        .map_err(|_| nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;
    let c = char::from_u32(code)
        .ok_or_else(|| nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Fail)))?;
    Ok((input, c))
}

// ---------------------------------------------------------------------------
// Color literals (§3.5)
// ---------------------------------------------------------------------------

pub fn parse_color(input: &str) -> IResult<&str, Color> {
    let (input, _) = char('#')(input)?;
    let (input, hex) = take_while_m_n(6, 8, |c: char| c.is_ascii_hexdigit())(input)?;
    let color = match hex.len() {
        6 => Color::rgb(
            u8::from_str_radix(&hex[0..2], 16).unwrap(),
            u8::from_str_radix(&hex[2..4], 16).unwrap(),
            u8::from_str_radix(&hex[4..6], 16).unwrap(),
        ),
        8 => Color::rgba(
            u8::from_str_radix(&hex[0..2], 16).unwrap(),
            u8::from_str_radix(&hex[2..4], 16).unwrap(),
            u8::from_str_radix(&hex[4..6], 16).unwrap(),
            u8::from_str_radix(&hex[6..8], 16).unwrap(),
        ),
        _ => unreachable!(),
    };
    Ok((input, color))
}

// ---------------------------------------------------------------------------
// Boolean literals
// ---------------------------------------------------------------------------

pub fn parse_bool(input: &str) -> IResult<&str, bool> {
    alt((value(true, tag("true")), value(false, tag("false"))))(input)
}

// ---------------------------------------------------------------------------
// Combined literal parser
// ---------------------------------------------------------------------------

pub fn parse_literal(input: &str) -> IResult<&str, Literal> {
    alt((
        map(parse_color, Literal::Color),
        map(parse_string, Literal::String),
        map(parse_bool, Literal::Boolean),
        parse_numeric_literal,
    ))(input)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer() {
        assert_eq!(parse_integer("42"), Ok(("", 42)));
    }

    #[test]
    fn test_float() {
        assert!((parse_float("3.14").unwrap().1 - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_quantified_time() {
        assert_eq!(
            parse_quantified("120ms").unwrap().1,
            Literal::Quantified { value: 120.0, unit: Unit::Time(TimeUnit::Millisecond) }
        );
    }

    #[test]
    fn test_quantified_length() {
        assert_eq!(
            parse_quantified("200px").unwrap().1,
            Literal::Quantified { value: 200.0, unit: Unit::Length(LengthUnit::Pixel) }
        );
    }

    #[test]
    fn test_string() {
        assert_eq!(parse_string(r#""hello""#).unwrap().1, "hello");
        assert_eq!(parse_string(r#""a\nb""#).unwrap().1, "a\nb");
    }

    #[test]
    fn test_color() {
        assert_eq!(parse_color("#FF0000").unwrap().1, Color::rgb(255, 0, 0));
    }

    #[test]
    fn test_bool() {
        assert_eq!(parse_bool("true").unwrap().1, true);
        assert_eq!(parse_bool("false").unwrap().1, false);
    }
}
