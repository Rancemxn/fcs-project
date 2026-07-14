use crate::ast::{
    ConstDeclaration, Definition, DefinitionsBlock, FunctionDeclaration, FunctionParameter,
    FunctionStatement, IfStatement, LetStatement, ReturnStatement, SourceSpan,
};

use super::ParseError;
use super::expression::parse_expression_at;

pub(super) fn parse_definitions(
    body: &str,
    body_offset: usize,
    block_span: SourceSpan,
) -> Result<DefinitionsBlock, ParseError> {
    let mut parser = DefinitionParser::new(body, body_offset);
    let mut declarations = Vec::new();
    while parser.skip_trivia()? {
        if parser.starts_keyword("const") {
            declarations.push(Definition::Const(parser.parse_const()?));
        } else if parser.starts_keyword("fn") {
            declarations.push(Definition::Function(parser.parse_function()?));
        } else {
            return Err(ParseError::InvalidSyntax("definitions block"));
        }
    }
    Ok(DefinitionsBlock {
        declarations,
        span: block_span,
    })
}

struct DefinitionParser<'a> {
    input: &'a str,
    base: usize,
    position: usize,
}

impl<'a> DefinitionParser<'a> {
    const fn new(input: &'a str, base: usize) -> Self {
        Self {
            input,
            base,
            position: 0,
        }
    }

    fn parse_const(&mut self) -> Result<ConstDeclaration, ParseError> {
        let start = self.position;
        self.expect_keyword("const")?;
        let (name, name_span) = self.identifier()?;
        self.expect_char(':')?;
        let ty = self.type_until(&['='])?;
        self.expect_char('=')?;
        let initializer = self.expression_until(';')?;
        self.expect_char(';')?;
        Ok(ConstDeclaration {
            name,
            name_span,
            ty,
            initializer,
            span: self.span(start, self.position),
        })
    }

    fn parse_function(&mut self) -> Result<FunctionDeclaration, ParseError> {
        let start = self.position;
        self.expect_keyword("fn")?;
        let (name, name_span) = self.identifier()?;
        self.expect_char('(')?;
        let mut parameters = Vec::new();
        self.skip_trivia()?;
        if self.peek_char() != Some(')') {
            loop {
                let parameter_start = self.position;
                let (parameter_name, parameter_name_span) = self.identifier()?;
                self.expect_char(':')?;
                let parameter_type = self.type_until(&[',', ')'])?;
                parameters.push(FunctionParameter {
                    name: parameter_name,
                    name_span: parameter_name_span,
                    ty: parameter_type,
                    span: self.span(parameter_start, self.position),
                });
                self.skip_trivia()?;
                if self.peek_char() != Some(',') {
                    break;
                }
                self.position += 1;
            }
        }
        self.expect_char(')')?;
        self.skip_trivia()?;
        if !self.remaining().starts_with("->") {
            return Err(ParseError::InvalidSyntax("function declaration"));
        }
        self.position += 2;
        let return_type = self.type_until(&['{'])?;
        self.expect_char('{')?;
        let body = self.parse_statements()?;
        self.expect_char('}')?;
        Ok(FunctionDeclaration {
            name,
            name_span,
            parameters,
            return_type,
            body,
            span: self.span(start, self.position),
        })
    }

    fn parse_statements(&mut self) -> Result<Vec<FunctionStatement>, ParseError> {
        let mut statements = Vec::new();
        loop {
            self.skip_trivia()?;
            if self.peek_char() == Some('}') {
                return Ok(statements);
            }
            if self.position == self.input.len() {
                return Err(ParseError::InvalidSyntax("function body"));
            }
            if self.starts_keyword("let") {
                statements.push(FunctionStatement::Let(self.parse_let()?));
            } else if self.starts_keyword("return") {
                statements.push(FunctionStatement::Return(self.parse_return()?));
            } else if self.starts_keyword("if") {
                statements.push(FunctionStatement::If(self.parse_if()?));
            } else {
                return Err(ParseError::InvalidSyntax("function statement"));
            }
        }
    }

    fn parse_let(&mut self) -> Result<LetStatement, ParseError> {
        let start = self.position;
        self.expect_keyword("let")?;
        let (name, name_span) = self.identifier()?;
        self.expect_char(':')?;
        let ty = self.type_until(&['='])?;
        self.expect_char('=')?;
        let initializer = self.expression_until(';')?;
        self.expect_char(';')?;
        Ok(LetStatement {
            name,
            name_span,
            ty,
            initializer,
            span: self.span(start, self.position),
        })
    }

    fn parse_return(&mut self) -> Result<ReturnStatement, ParseError> {
        let start = self.position;
        self.expect_keyword("return")?;
        let value = self.expression_until(';')?;
        self.expect_char(';')?;
        Ok(ReturnStatement {
            value,
            span: self.span(start, self.position),
        })
    }

    fn parse_if(&mut self) -> Result<IfStatement, ParseError> {
        let start = self.position;
        self.expect_keyword("if")?;
        let condition = self.expression_until('{')?;
        self.expect_char('{')?;
        let then_branch = self.parse_statements()?;
        self.expect_char('}')?;
        self.skip_trivia()?;
        let else_branch = if self.starts_keyword("else") {
            self.expect_keyword("else")?;
            self.expect_char('{')?;
            let statements = self.parse_statements()?;
            self.expect_char('}')?;
            statements
        } else {
            Vec::new()
        };
        Ok(IfStatement {
            condition,
            then_branch,
            else_branch,
            span: self.span(start, self.position),
        })
    }

    fn type_until(&mut self, delimiters: &[char]) -> Result<crate::ast::Type, ParseError> {
        let (text, _) = self.text_until(delimiters, true)?;
        super::expression::parse_type_inner(text.trim(), super::ParseLimits::default())
    }

    fn expression_until(
        &mut self,
        delimiter: char,
    ) -> Result<crate::ast::SourceExpression, ParseError> {
        let (text, start) = self.text_until(&[delimiter], false)?;
        let leading = text.len() - text.trim_start().len();
        parse_expression_at(text.trim(), self.base + start + leading)
    }

    fn text_until(
        &mut self,
        delimiters: &[char],
        track_angle_brackets: bool,
    ) -> Result<(&'a str, usize), ParseError> {
        self.skip_trivia()?;
        let start = self.position;
        let mut angle_depth = 0_usize;
        let mut parenthesis_depth = 0_usize;
        let mut in_string = false;
        let mut escaped = false;
        let mut line_comment = false;
        let mut block_comment = false;
        while let Some(character) = self.peek_char() {
            if line_comment {
                self.position += character.len_utf8();
                if character == '\n' {
                    line_comment = false;
                }
                continue;
            }
            if block_comment {
                if self.remaining().starts_with("*/") {
                    self.position += 2;
                    block_comment = false;
                } else {
                    self.position += character.len_utf8();
                }
                continue;
            }
            if in_string {
                self.position += character.len_utf8();
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    in_string = false;
                }
                continue;
            }
            if self.remaining().starts_with("//") {
                self.position += 2;
                line_comment = true;
                continue;
            }
            if self.remaining().starts_with("/*") {
                self.position += 2;
                block_comment = true;
                continue;
            }
            match character {
                '"' => in_string = true,
                '<' if track_angle_brackets => angle_depth += 1,
                '>' if track_angle_brackets && angle_depth > 0 => angle_depth -= 1,
                '(' => parenthesis_depth += 1,
                ')' if parenthesis_depth > 0 => parenthesis_depth -= 1,
                _ => {}
            }
            if angle_depth == 0 && parenthesis_depth == 0 && delimiters.contains(&character) {
                let text = &self.input[start..self.position];
                if text.trim().is_empty() {
                    return Err(ParseError::InvalidSyntax("definition expression"));
                }
                return Ok((text, start));
            }
            self.position += character.len_utf8();
        }
        Err(ParseError::InvalidSyntax("definitions block"))
    }

    fn identifier(&mut self) -> Result<(String, SourceSpan), ParseError> {
        self.skip_trivia()?;
        let start = self.position;
        let Some(first) = self.peek_char() else {
            return Err(ParseError::InvalidSyntax("identifier"));
        };
        if first != '_' && !first.is_ascii_alphabetic() {
            return Err(ParseError::InvalidSyntax("identifier"));
        }
        self.position += first.len_utf8();
        while let Some(character) = self.peek_char() {
            if character != '_' && !character.is_ascii_alphanumeric() {
                break;
            }
            self.position += character.len_utf8();
        }
        Ok((
            self.input[start..self.position].to_owned(),
            self.span(start, self.position),
        ))
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        self.skip_trivia()?;
        if !self.starts_keyword(keyword) {
            return Err(ParseError::InvalidSyntax("definition keyword"));
        }
        self.position += keyword.len();
        Ok(())
    }

    fn starts_keyword(&self, keyword: &str) -> bool {
        self.remaining().starts_with(keyword)
            && self.remaining()[keyword.len()..]
                .chars()
                .next()
                .is_none_or(|character| character != '_' && !character.is_ascii_alphanumeric())
    }

    fn expect_char(&mut self, expected: char) -> Result<(), ParseError> {
        self.skip_trivia()?;
        if self.peek_char() != Some(expected) {
            return Err(ParseError::InvalidSyntax("definition punctuation"));
        }
        self.position += expected.len_utf8();
        Ok(())
    }

    fn skip_trivia(&mut self) -> Result<bool, ParseError> {
        loop {
            while self.peek_char().is_some_and(char::is_whitespace) {
                self.position += self.peek_char().expect("checked above").len_utf8();
            }
            if self.remaining().starts_with("//") {
                self.position += 2;
                self.position = self.input[self.position..]
                    .find('\n')
                    .map_or(self.input.len(), |index| self.position + index + 1);
            } else if self.remaining().starts_with("/*") {
                let end = self.input[self.position + 2..]
                    .find("*/")
                    .ok_or(ParseError::InvalidSyntax("definitions block"))?;
                self.position += 2 + end + 2;
            } else {
                return Ok(self.position < self.input.len());
            }
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.position..]
    }

    const fn span(&self, start: usize, end: usize) -> SourceSpan {
        SourceSpan::new(self.base + start, self.base + end)
    }
}
