use crate::ast::SourceLiteral;
use crate::diagnostic::ParseOutput;

use super::ParseLimits;
use super::document::parse_document_tokens;
use super::input::SpannedToken;
use super::lexer::lex_document;
use super::token::{Keyword, Punctuation, Token};

/// Rewrite source trivia and token separators into one deterministic layout.
pub fn canonicalize_source_layout(source: &str) -> ParseOutput<String> {
    let normalized = normalize_source_trivia(source);
    let source = normalized.as_str();
    let tokens = match lex_document(source, ParseLimits::default()) {
        Ok(tokens) => tokens,
        Err(diagnostics) => return ParseOutput::new(None, diagnostics),
    };
    if let Err(diagnostics) = parse_document_tokens(source, &tokens).into_result() {
        return ParseOutput::new(None, diagnostics);
    }

    ParseOutput::new(Some(layout_source(source, &tokens)), Vec::new())
}

/// Rewrite numeric token spellings without changing source trivia or token order.
pub fn canonicalize_numeric_literals(source: &str) -> ParseOutput<String> {
    let tokens = match lex_document(source, ParseLimits::default()) {
        Ok(tokens) => tokens,
        Err(diagnostics) => return ParseOutput::new(None, diagnostics),
    };
    if let Err(diagnostics) = parse_document_tokens(source, &tokens).into_result() {
        return ParseOutput::new(None, diagnostics);
    }

    let mut formatted = String::with_capacity(source.len());
    let mut cursor = 0;
    for (token, span) in &tokens {
        formatted.push_str(&source[cursor..span.start]);
        let raw = &source[span.start..span.end];
        if let Some(replacement) = numeric_lexeme(token) {
            formatted.push_str(&replacement);
        } else {
            formatted.push_str(raw);
        }
        cursor = span.end;
    }
    formatted.push_str(&source[cursor..]);
    ParseOutput::new(Some(formatted), Vec::new())
}

fn numeric_lexeme(token: &Token) -> Option<String> {
    match token {
        Token::TempoBpm(value) => Some(format_unit(value.get(), "bpm", false)),
        Token::Literal(literal) => match literal {
            SourceLiteral::Int(value) => Some(value.to_string()),
            SourceLiteral::Float(value) => Some(format_unit(*value, "", true)),
            SourceLiteral::Time(value) => Some(format_unit(*value, "s", false)),
            SourceLiteral::Length(value) => Some(format_unit(*value, "px", false)),
            SourceLiteral::Angle(value) => Some(format_unit(*value, "rad", false)),
            SourceLiteral::IntMagnitude(_)
            | SourceLiteral::Beat(_)
            | SourceLiteral::Bool(_)
            | SourceLiteral::Null
            | SourceLiteral::String(_)
            | SourceLiteral::Color(_)
            | SourceLiteral::Line(_) => None,
        },
        _ => None,
    }
}

fn format_unit(value: f64, suffix: &str, force_float: bool) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let mut number = value.to_string();
    if let Some(index) = number.find('e').or_else(|| number.find('E')) {
        let (mantissa, exponent) = number.split_at(index);
        let exponent = exponent[1..].parse::<i32>().unwrap_or_default();
        number = format!("{mantissa}e{exponent}");
    }
    if force_float && !number.contains(['.', 'e']) {
        number.push_str(".0");
    }
    number.push_str(suffix);
    number
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenMark {
    Header,
    Atom,
    Keyword(Keyword),
    Punctuation(Punctuation),
}

impl TokenMark {
    const fn is_atom(self) -> bool {
        matches!(self, Self::Atom | Self::Keyword(_))
    }
}

#[derive(Debug)]
struct TriviaComment {
    text: String,
    line: bool,
    own_line: bool,
}

fn layout_source(source: &str, tokens: &[SpannedToken]) -> String {
    let mut layout = Layout::new();
    let mut cursor = 0;
    for (index, (token, span)) in tokens.iter().enumerate() {
        layout.trivia(&source[cursor..span.start]);
        layout.token(token, &source[span.start..span.end], tokens.get(index + 1));
        cursor = span.end;
    }
    layout.trivia(&source[cursor..]);
    layout.finish()
}

struct Layout {
    output: String,
    indent: usize,
    line_start: bool,
    previous: Option<TokenMark>,
    delimiters: Vec<Punctuation>,
}

impl Layout {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            line_start: true,
            previous: None,
            delimiters: Vec::new(),
        }
    }

    fn trivia(&mut self, gap: &str) {
        if self.output.is_empty() && gap.starts_with('\u{feff}') {
            self.output.push('\u{feff}');
        }
        for comment in trivia_comments(gap) {
            if comment.own_line {
                self.newline();
            } else if self.line_start && self.output.ends_with('\n') {
                self.output.pop();
                self.line_start = false;
                self.space();
            } else if !self.line_start {
                self.space();
            }
            self.write(&comment.text);
            if comment.line || comment.text.contains(['\r', '\n']) {
                self.newline();
            } else {
                self.space();
            }
        }
    }

    fn token(&mut self, token: &Token, raw: &str, next: Option<&SpannedToken>) {
        let mark = token_mark(token);
        let next = next.map(|(token, _)| token_mark(token));
        if matches!(mark, TokenMark::Header) {
            self.write(normalize_line_endings(raw).trim_end_matches('\n'));
            self.newline();
            self.previous = Some(mark);
            return;
        }

        match mark {
            TokenMark::Punctuation(Punctuation::LeftBrace) => {
                self.space_before_atom_or_delimiter();
                self.write(raw);
                self.delimiters.push(Punctuation::LeftBrace);
                if next != Some(TokenMark::Punctuation(Punctuation::RightBrace)) {
                    self.indent += 1;
                    self.newline();
                }
            }
            TokenMark::Punctuation(Punctuation::RightBrace) => {
                if self.previous != Some(TokenMark::Punctuation(Punctuation::LeftBrace))
                    && !self.line_start
                {
                    self.newline();
                }
                self.indent = self.indent.saturating_sub(1);
                self.write(raw);
                self.pop_delimiter(Punctuation::LeftBrace);
                if !matches!(
                    next,
                    Some(
                        TokenMark::Punctuation(
                            Punctuation::Comma
                                | Punctuation::Semicolon
                                | Punctuation::RightBrace
                                | Punctuation::RightBracket
                                | Punctuation::RightParenthesis,
                        ) | TokenMark::Keyword(Keyword::Else)
                    )
                ) {
                    self.newline();
                }
            }
            TokenMark::Punctuation(Punctuation::LeftParenthesis) => {
                self.write(raw);
                self.delimiters.push(Punctuation::LeftParenthesis);
            }
            TokenMark::Punctuation(Punctuation::RightParenthesis) => {
                self.trim_line();
                self.write(raw);
                self.pop_delimiter(Punctuation::LeftParenthesis);
            }
            TokenMark::Punctuation(Punctuation::LeftBracket) => {
                self.write(raw);
                self.delimiters.push(Punctuation::LeftBracket);
            }
            TokenMark::Punctuation(Punctuation::RightBracket) => {
                self.trim_line();
                self.write(raw);
                self.pop_delimiter(Punctuation::LeftBracket);
            }
            TokenMark::Punctuation(Punctuation::Comma) => {
                self.trim_line();
                self.write(raw);
                if self.delimiters.last() == Some(&Punctuation::LeftBrace) {
                    self.newline();
                } else {
                    self.space();
                }
            }
            TokenMark::Punctuation(Punctuation::Colon) => {
                self.trim_line();
                self.write(raw);
                self.space();
            }
            TokenMark::Punctuation(Punctuation::Semicolon) => {
                self.trim_line();
                self.write(raw);
                self.newline();
            }
            TokenMark::Punctuation(Punctuation::Dot) => {
                self.trim_line();
                self.write(raw);
            }
            TokenMark::Punctuation(Punctuation::At) => self.write(raw),
            TokenMark::Punctuation(punctuation) if is_range(punctuation) => {
                self.trim_line();
                self.write(raw);
            }
            TokenMark::Punctuation(punctuation) if is_operator(punctuation) => {
                if is_unary(punctuation, self.previous) {
                    self.write(raw);
                } else {
                    self.space_before_atom_or_delimiter();
                    self.write(raw);
                    self.space();
                }
            }
            mark => {
                if needs_space_before(mark, self.previous) {
                    self.space();
                }
                let text = numeric_lexeme(token).unwrap_or_else(|| raw.to_owned());
                self.write(&text);
            }
        }
        self.previous = Some(mark);
    }

    fn finish(mut self) -> String {
        self.trim_line();
        while self.output.ends_with('\n') {
            self.output.pop();
        }
        self.output.push('\n');
        self.output
    }

    fn write(&mut self, text: &str) {
        if self.line_start && !text.is_empty() {
            self.output.push_str(&"    ".repeat(self.indent));
            self.line_start = false;
        }
        self.output.push_str(text);
        self.line_start = text.ends_with('\n');
    }

    fn newline(&mut self) {
        self.trim_line();
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.line_start = true;
    }

    fn space(&mut self) {
        if !self.line_start && !self.output.ends_with([' ', '\t', '\n']) {
            self.output.push(' ');
        }
    }

    fn trim_line(&mut self) {
        while self.output.ends_with([' ', '\t']) {
            self.output.pop();
        }
    }

    fn space_before_atom_or_delimiter(&mut self) {
        if !self.line_start
            && !matches!(
                self.previous,
                Some(TokenMark::Punctuation(
                    Punctuation::LeftParenthesis
                        | Punctuation::LeftBracket
                        | Punctuation::Dot
                        | Punctuation::At
                ))
            )
        {
            self.space();
        }
    }

    fn pop_delimiter(&mut self, expected: Punctuation) {
        if self.delimiters.last() == Some(&expected) {
            self.delimiters.pop();
        }
    }
}

fn token_mark(token: &Token) -> TokenMark {
    match token {
        Token::Header(_) => TokenMark::Header,
        Token::Keyword(keyword) => TokenMark::Keyword(*keyword),
        Token::Punctuation(punctuation) => TokenMark::Punctuation(*punctuation),
        _ => TokenMark::Atom,
    }
}

fn needs_space_before(current: TokenMark, previous: Option<TokenMark>) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    if !current.is_atom() {
        return false;
    }
    !matches!(
        previous,
        TokenMark::Punctuation(
            Punctuation::At
                | Punctuation::Dot
                | Punctuation::LeftParenthesis
                | Punctuation::LeftBracket
                | Punctuation::RangeExclusive
                | Punctuation::RangeInclusive
        ) | TokenMark::Punctuation(Punctuation::Minus | Punctuation::Plus | Punctuation::Bang)
    )
}

const fn is_range(punctuation: Punctuation) -> bool {
    matches!(
        punctuation,
        Punctuation::RangeExclusive | Punctuation::RangeInclusive
    )
}

const fn is_operator(punctuation: Punctuation) -> bool {
    matches!(
        punctuation,
        Punctuation::Arrow
            | Punctuation::FatArrow
            | Punctuation::Plus
            | Punctuation::Minus
            | Punctuation::Star
            | Punctuation::Power
            | Punctuation::Slash
            | Punctuation::Percent
            | Punctuation::Bang
            | Punctuation::Equal
            | Punctuation::EqualEqual
            | Punctuation::BangEqual
            | Punctuation::LessThan
            | Punctuation::LessThanOrEqual
            | Punctuation::GreaterThan
            | Punctuation::GreaterThanOrEqual
            | Punctuation::AndAnd
            | Punctuation::OrOr
    )
}

fn is_unary(punctuation: Punctuation, previous: Option<TokenMark>) -> bool {
    if !matches!(
        punctuation,
        Punctuation::Minus | Punctuation::Plus | Punctuation::Bang
    ) {
        return false;
    }
    previous.is_none_or(|mark| {
        matches!(
            mark,
            TokenMark::Punctuation(
                Punctuation::LeftParenthesis
                    | Punctuation::LeftBracket
                    | Punctuation::LeftBrace
                    | Punctuation::Comma
                    | Punctuation::Colon
                    | Punctuation::Semicolon
                    | Punctuation::Arrow
                    | Punctuation::FatArrow
                    | Punctuation::Equal
                    | Punctuation::EqualEqual
                    | Punctuation::BangEqual
                    | Punctuation::LessThan
                    | Punctuation::LessThanOrEqual
                    | Punctuation::GreaterThan
                    | Punctuation::GreaterThanOrEqual
                    | Punctuation::AndAnd
                    | Punctuation::OrOr
            )
        )
    })
}

fn trivia_comments(gap: &str) -> Vec<TriviaComment> {
    let bytes = gap.as_bytes();
    let mut comments = Vec::new();
    let mut index = 0;
    while index + 1 < bytes.len() {
        if bytes[index] != b'/' || !matches!(bytes[index + 1], b'/' | b'*') {
            index += 1;
            continue;
        }
        let start = index;
        let line = bytes[index + 1] == b'/';
        let end = if line {
            index += 2;
            while index < bytes.len() && !matches!(bytes[index], b'\r' | b'\n') {
                index += 1;
            }
            index
        } else {
            let mut depth = 1;
            index += 2;
            while index + 1 < bytes.len() && depth != 0 {
                match &bytes[index..index + 2] {
                    b"/*" => {
                        depth += 1;
                        index += 2;
                    }
                    b"*/" => {
                        depth -= 1;
                        index += 2;
                    }
                    _ => index += 1,
                }
            }
            index
        };
        comments.push(TriviaComment {
            text: normalize_line_endings(&gap[start..end]),
            line,
            own_line: gap[..start].contains(['\r', '\n']),
        });
    }
    comments
}

fn normalize_line_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn normalize_source_trivia(source: &str) -> String {
    let normalized = normalize_line_endings(source);
    let mut lines: Vec<_> = normalized
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect();
    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    format!("{}\n", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_numeric_literals, canonicalize_source_layout};

    #[test]
    fn lays_out_tokens_deterministically_without_rewriting_strings_or_comments() {
        let compact = "#fcs 5.0.0\nformat{profile:fragment;}meta{custom:{\"value\":1e2,\"text\":\"1e2\"};}//keep\n";
        let spaced = "#fcs 5.0.0\r\nformat { profile : fragment ; }\r\nmeta { custom : { \"value\" : 1e2 , \"text\" : \"1e2\" } ; } //keep\r\n";
        let compact = canonicalize_source_layout(compact)
            .into_result()
            .expect("compact source should format");
        let spaced = canonicalize_source_layout(spaced)
            .into_result()
            .expect("spaced source should format");

        assert_eq!(compact, spaced);
        assert_eq!(
            compact,
            canonicalize_source_layout(&compact).into_result().unwrap()
        );
        assert!(compact.contains("//keep"));
        assert!(compact.contains("\"text\": \"1e2\""));
        assert!(compact.contains("100.0"));
    }

    #[test]
    fn keeps_balanced_extension_preserve_and_render_payload_tokens() {
        let source = "#fcs 5.0.0\nformat { profile: fragment; }\nrender profile 1.0.0 { layer opaque { value: \"{ raw }\"; } }\nextensions { extension(\"org.test\", 1.0.0) optional { \"raw\": \"1e2\", } }\npreserve { payload: extension(\"org.test\", 1.0.0) { \"raw\": \"1e2\", }; }";
        let formatted = canonicalize_source_layout(source)
            .into_result()
            .expect("opaque envelopes should format");

        assert_eq!(
            formatted,
            canonicalize_source_layout(&formatted)
                .into_result()
                .unwrap()
        );
        assert!(formatted.contains("layer opaque"));
        assert!(formatted.contains("\"{ raw }\""));
        assert!(formatted.matches("\"1e2\"").count() >= 2);
    }

    #[test]
    fn normalizes_line_endings_and_trailing_trivia_before_validation() {
        let source = "#fcs 5.0.0  \r\nformat { profile: fragment; }  \r\n";
        let formatted = canonicalize_source_layout(source)
            .into_result()
            .expect("trailing source trivia should be normalized");
        assert_eq!(
            formatted,
            "#fcs 5.0.0\nformat {\n    profile: fragment;\n}\n"
        );
    }

    #[test]
    fn rewrites_float_units_and_bpm_but_preserves_exact_literals() {
        let source = "#fcs 5.0.0\nformat { profile: chart; }\n\
tempoMap { 0beat -> 120.00e0bpm; }\n\
meta { custom: { \"beat\": 1.25beat, \"text\": \"1.25e2\" }; }";
        let formatted = canonicalize_numeric_literals(source)
            .into_result()
            .expect("source should format");
        assert!(formatted.contains("120bpm"));
        assert!(formatted.contains("1.25beat"));
        assert!(formatted.contains("\"1.25e2\""));
    }

    #[test]
    fn rejects_invalid_numeric_source_before_rewrite() {
        let errors = canonicalize_numeric_literals(
            "#fcs 5.0.0\nformat { profile: chart; }\nmeta { custom: { \"x\": 1e999 }; }",
        )
        .into_result()
        .expect_err("non-finite input must fail");
        assert_eq!(errors[0].code().as_str(), "numeric.non-finite");
    }
}
