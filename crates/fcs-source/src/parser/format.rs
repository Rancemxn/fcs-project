use crate::ast::SourceLiteral;
use crate::diagnostic::ParseOutput;

use super::ParseLimits;
use super::document::parse_document_tokens;
use super::lexer::lex_document;
use super::token::Token;

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

#[cfg(test)]
mod tests {
    use super::canonicalize_numeric_literals;

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
