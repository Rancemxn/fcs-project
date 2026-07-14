use crate::ast::Color;
use crate::ast::{Beat, SourceLiteral, SourceSpan};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Token {
    pub(super) kind: TokenKind,
    pub(super) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum TokenKind {
    Literal(SourceLiteral),
    Identifier(String),
    Symbol(Symbol),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Symbol {
    LeftParenthesis,
    RightParenthesis,
    Comma,
    Dot,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Bang,
    EqualEqual,
    BangEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    AndAnd,
    OrOr,
}

pub(super) fn lex(input: &str) -> Result<Vec<Token>, ()> {
    Lexer { input, offset: 0 }.lex()
}

struct Lexer<'a> {
    input: &'a str,
    offset: usize,
}

impl Lexer<'_> {
    fn lex(mut self) -> Result<Vec<Token>, ()> {
        let mut tokens = Vec::new();
        while self.offset < self.input.len() {
            let remaining = &self.input[self.offset..];
            let character = remaining.chars().next().ok_or(())?;

            if character.is_whitespace() {
                self.offset += character.len_utf8();
            } else if remaining.starts_with("//") {
                self.skip_line_comment();
            } else if remaining.starts_with("/*") {
                self.skip_block_comment()?;
            } else if is_identifier_start(character) {
                tokens.push(self.lex_identifier());
            } else if character.is_ascii_digit() {
                tokens.push(self.lex_number()?);
            } else if character == '"' {
                tokens.push(self.lex_string()?);
            } else if character == '#' {
                tokens.push(self.lex_color()?);
            } else {
                tokens.push(self.lex_symbol()?);
            }
        }
        Ok(tokens)
    }

    fn skip_line_comment(&mut self) {
        let remaining = &self.input[self.offset + 2..];
        self.offset = remaining
            .find('\n')
            .map_or(self.input.len(), |index| self.offset + 2 + index);
    }

    fn skip_block_comment(&mut self) -> Result<(), ()> {
        let remaining = &self.input[self.offset + 2..];
        let end = remaining.find("*/").ok_or(())?;
        self.offset += 2 + end + 2;
        Ok(())
    }

    fn lex_identifier(&mut self) -> Token {
        let start = self.offset;
        self.advance_while(is_identifier_continue);
        let text = &self.input[start..self.offset];
        let kind = match text {
            "true" => TokenKind::Literal(SourceLiteral::Bool(true)),
            "false" => TokenKind::Literal(SourceLiteral::Bool(false)),
            _ => TokenKind::Identifier(text.to_owned()),
        };
        Token {
            kind,
            span: SourceSpan::new(start, self.offset),
        }
    }

    fn lex_number(&mut self) -> Result<Token, ()> {
        let start = self.offset;
        self.advance_while(|character| character.is_ascii_digit());

        if self.remaining().starts_with('.')
            && self
                .remaining()
                .get(1..)
                .and_then(|remaining| remaining.chars().next())
                .is_some_and(|character| character.is_ascii_digit())
        {
            self.offset += 1;
            self.advance_while(|character| character.is_ascii_digit());
        }

        if self
            .remaining()
            .chars()
            .next()
            .is_some_and(|character| matches!(character, 'e' | 'E'))
        {
            let exponent_start = self.offset;
            self.offset += 1;
            if self
                .remaining()
                .chars()
                .next()
                .is_some_and(|character| matches!(character, '+' | '-'))
            {
                self.offset += 1;
            }
            let digits_start = self.offset;
            self.advance_while(|character| character.is_ascii_digit());
            if self.offset == digits_start {
                self.offset = exponent_start;
            }
        }

        let number_end = self.offset;
        self.advance_while(|character| character.is_ascii_alphabetic());
        let number = &self.input[start..number_end];
        let unit = &self.input[number_end..self.offset];
        let literal = parse_number_literal(number, unit)?;
        Ok(Token {
            kind: TokenKind::Literal(literal),
            span: SourceSpan::new(start, self.offset),
        })
    }

    fn lex_string(&mut self) -> Result<Token, ()> {
        let start = self.offset;
        self.offset += 1;
        let mut value = String::new();

        while self.offset < self.input.len() {
            let character = self.remaining().chars().next().ok_or(())?;
            match character {
                '"' => {
                    self.offset += 1;
                    return Ok(Token {
                        kind: TokenKind::Literal(SourceLiteral::String(value)),
                        span: SourceSpan::new(start, self.offset),
                    });
                }
                '\\' => {
                    self.offset += 1;
                    value.push(self.lex_escape()?);
                }
                '\n' | '\r' => return Err(()),
                _ => {
                    self.offset += character.len_utf8();
                    value.push(character);
                }
            }
        }
        Err(())
    }

    fn lex_escape(&mut self) -> Result<char, ()> {
        let escaped = self.remaining().chars().next().ok_or(())?;
        self.offset += escaped.len_utf8();
        match escaped {
            'n' => Ok('\n'),
            't' => Ok('\t'),
            '\\' => Ok('\\'),
            '"' => Ok('"'),
            'u' => self.lex_unicode_escape(),
            _ => Err(()),
        }
    }

    fn lex_unicode_escape(&mut self) -> Result<char, ()> {
        if !self.remaining().starts_with('{') {
            return Err(());
        }
        self.offset += 1;
        let digits_start = self.offset;
        self.advance_while(|character| character.is_ascii_hexdigit());
        let digits = &self.input[digits_start..self.offset];
        if !(1..=6).contains(&digits.len()) || !self.remaining().starts_with('}') {
            return Err(());
        }
        self.offset += 1;
        let codepoint = u32::from_str_radix(digits, 16).map_err(|_| ())?;
        char::from_u32(codepoint).ok_or(())
    }

    fn lex_color(&mut self) -> Result<Token, ()> {
        let start = self.offset;
        self.offset += 1;
        self.advance_while(|character| character.is_ascii_hexdigit());
        let text = &self.input[start..self.offset];
        let color = text.parse::<Color>().map_err(|_| ())?;
        Ok(Token {
            kind: TokenKind::Literal(SourceLiteral::Color(color)),
            span: SourceSpan::new(start, self.offset),
        })
    }

    fn lex_symbol(&mut self) -> Result<Token, ()> {
        let start = self.offset;
        let (symbol, length) = if self.remaining().starts_with("==") {
            (Symbol::EqualEqual, 2)
        } else if self.remaining().starts_with("!=") {
            (Symbol::BangEqual, 2)
        } else if self.remaining().starts_with("<=") {
            (Symbol::LessThanOrEqual, 2)
        } else if self.remaining().starts_with(">=") {
            (Symbol::GreaterThanOrEqual, 2)
        } else if self.remaining().starts_with("&&") {
            (Symbol::AndAnd, 2)
        } else if self.remaining().starts_with("||") {
            (Symbol::OrOr, 2)
        } else {
            let symbol = match self.remaining().chars().next().ok_or(())? {
                '(' => Symbol::LeftParenthesis,
                ')' => Symbol::RightParenthesis,
                ',' => Symbol::Comma,
                '.' => Symbol::Dot,
                '+' => Symbol::Plus,
                '-' => Symbol::Minus,
                '*' => Symbol::Star,
                '/' => Symbol::Slash,
                '%' => Symbol::Percent,
                '!' => Symbol::Bang,
                '<' => Symbol::LessThan,
                '>' => Symbol::GreaterThan,
                _ => return Err(()),
            };
            (symbol, 1)
        };
        self.offset += length;
        Ok(Token {
            kind: TokenKind::Symbol(symbol),
            span: SourceSpan::new(start, self.offset),
        })
    }

    fn remaining(&self) -> &str {
        &self.input[self.offset..]
    }

    fn advance_while(&mut self, predicate: impl Fn(char) -> bool) {
        while let Some(character) = self.remaining().chars().next() {
            if !predicate(character) {
                break;
            }
            self.offset += character.len_utf8();
        }
    }
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn parse_number_literal(number: &str, unit: &str) -> Result<SourceLiteral, ()> {
    if unit.is_empty() {
        if number.contains(['.', 'e', 'E']) {
            return parse_finite_float(number).map(SourceLiteral::Float);
        }
        return number
            .parse::<i64>()
            .map(SourceLiteral::Int)
            .map_err(|_| ());
    }

    if unit == "beat" {
        return parse_beat(number).map(SourceLiteral::Beat);
    }

    let value = parse_finite_float(number)?;
    let literal = match unit {
        "ms" => Ok(SourceLiteral::Time(value / 1_000.0)),
        "s" => Ok(SourceLiteral::Time(value)),
        "min" => Ok(SourceLiteral::Time(value * 60.0)),
        "px" => Ok(SourceLiteral::Length(value)),
        "vw" => Ok(SourceLiteral::Length(value * 19.2)),
        "vh" => Ok(SourceLiteral::Length(value * 10.8)),
        "deg" => Ok(SourceLiteral::Angle(value.to_radians())),
        "rad" => Ok(SourceLiteral::Angle(value)),
        _ => Err(()),
    }?;
    match &literal {
        SourceLiteral::Time(value) | SourceLiteral::Length(value) | SourceLiteral::Angle(value)
            if value.is_finite() =>
        {
            Ok(literal)
        }
        _ => Err(()),
    }
}

fn parse_finite_float(number: &str) -> Result<f64, ()> {
    let value = number.parse::<f64>().map_err(|_| ())?;
    value.is_finite().then_some(value).ok_or(())
}

fn parse_beat(number: &str) -> Result<Beat, ()> {
    let (mantissa, exponent) = if let Some(index) = number.find(['e', 'E']) {
        (
            &number[..index],
            number[index + 1..].parse::<i32>().map_err(|_| ())?,
        )
    } else {
        (number, 0_i32)
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{whole}{fraction}");
    let mut numerator = digits.parse::<i128>().map_err(|_| ())?;
    let mut denominator = 10_i128
        .checked_pow(u32::try_from(fraction.len()).map_err(|_| ())?)
        .ok_or(())?;
    if exponent >= 0 {
        numerator = numerator
            .checked_mul(10_i128.checked_pow(exponent.unsigned_abs()).ok_or(())?)
            .ok_or(())?;
    } else {
        denominator = denominator
            .checked_mul(10_i128.checked_pow(exponent.unsigned_abs()).ok_or(())?)
            .ok_or(())?;
    }
    if numerator == 0 {
        return Beat::new(0, 1).map_err(|_| ());
    }
    let divisor = i128::try_from(greatest_common_divisor(
        numerator.unsigned_abs(),
        denominator.unsigned_abs(),
    ))
    .map_err(|_| ())?;
    Beat::new(
        i64::try_from(numerator / divisor).map_err(|_| ())?,
        i64::try_from(denominator / divisor).map_err(|_| ())?,
    )
    .map_err(|_| ())
}

fn greatest_common_divisor(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
