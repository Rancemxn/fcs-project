use crate::v5::ast::{
    CollectionBlock, CollectionItem, CollectionsBlock, EntityConstructor, EntityExpression,
    EntityField, FieldPath, Generator, GeneratorItem, NoteVariant, SourceExpression, SourceRange,
    SourceSpan, TemplateDeclaration, TemplateParameter, TemplatesBlock, Type, WithExpression,
};

use super::{ParseError, expression::parse_expression_at, expression::parse_type};

pub(super) fn parse_templates(
    input: &str,
    base: usize,
    span: SourceSpan,
) -> Result<TemplatesBlock, ParseError> {
    let mut cursor = Cursor::new(input, base);
    let mut declarations = Vec::new();
    while !cursor.done()? {
        let start = cursor.position();
        cursor.keyword("template")?;
        let (name, name_span) = cursor.identifier()?;
        cursor.char('(')?;
        let mut parameters = Vec::new();
        cursor.skip();
        if !cursor.peek_is(')') {
            loop {
                let parameter_start = cursor.position();
                let (parameter_name, parameter_name_span) = cursor.identifier()?;
                cursor.char(':')?;
                let type_start = cursor.position();
                let type_text = cursor.until_top_level(&[',', ')'])?;
                let ty = parse_type(type_text.trim())?;
                let type_end = type_start + type_text.len();
                parameters.push(TemplateParameter {
                    name: parameter_name,
                    name_span: parameter_name_span,
                    ty,
                    span: SourceSpan::new(parameter_start, type_end),
                });
                cursor.skip();
                if cursor.take_char(',') {
                    cursor.skip();
                    continue;
                }
                break;
            }
        }
        cursor.char(')')?;
        cursor.char('-')?;
        cursor.char('>')?;
        let return_start = cursor.position();
        let return_text = cursor.until_top_level(&['{'])?;
        let return_type = parse_type(return_text.trim())?;
        cursor.char('{')?;
        cursor.keyword("return")?;
        let body = parse_entity_expression(&mut cursor)?;
        cursor.char(';')?;
        cursor.char('}')?;
        declarations.push(TemplateDeclaration {
            name,
            name_span,
            parameters,
            return_type,
            body,
            span: SourceSpan::new(start, cursor.position()),
        });
        let _ = return_start;
    }
    Ok(TemplatesBlock { declarations, span })
}

pub(super) fn parse_collections(
    input: &str,
    base: usize,
    span: SourceSpan,
) -> Result<CollectionsBlock, ParseError> {
    let mut cursor = Cursor::new(input, base);
    let mut collections = Vec::new();
    while !cursor.done()? {
        let start = cursor.position();
        let (collection_name, _) = cursor.identifier()?;
        cursor.char('{')?;
        let items = parse_collection_items_until(&mut cursor, '}')?;
        cursor.char('}')?;
        collections.push(CollectionBlock {
            collection_name,
            items,
            span: SourceSpan::new(start, cursor.position()),
        });
    }
    Ok(CollectionsBlock { collections, span })
}

fn parse_collection_items_until(
    cursor: &mut Cursor<'_>,
    terminator: char,
) -> Result<Vec<CollectionItem>, ParseError> {
    let mut items = Vec::new();
    while !cursor.peek_is(terminator) {
        if cursor.take_keyword("generate") {
            let start = cursor.position().saturating_sub("generate".len());
            items.push(CollectionItem::Generator(parse_generator(cursor, start)?));
        } else if cursor.take_keyword("if") {
            let start = cursor.position().saturating_sub(2);
            let condition_text = cursor.until_top_level(&['{'])?;
            let condition_offset = condition_text.len() - condition_text.trim_start().len();
            let condition = parse_expression_at(
                condition_text.trim(),
                cursor.position_before(condition_text) + condition_offset,
            )?;
            cursor.char('{')?;
            let then_items = parse_collection_items_until(cursor, '}')?;
            cursor.char('}')?;
            cursor.skip();
            let else_items = if cursor.take_keyword("else") {
                cursor.char('{')?;
                let items = parse_collection_items_until(cursor, '}')?;
                cursor.char('}')?;
                items
            } else {
                Vec::new()
            };
            items.push(CollectionItem::Conditional {
                condition,
                then_items,
                else_items,
                span: SourceSpan::new(start, cursor.position()),
            });
        } else {
            let expression = parse_entity_expression(cursor)?;
            cursor.char(';')?;
            items.push(match expression {
                EntityExpression::Constructor(constructor) => {
                    CollectionItem::Constructor(constructor)
                }
                expression => CollectionItem::Expression(expression),
            });
        }
    }
    Ok(items)
}

fn parse_generator(cursor: &mut Cursor<'_>, start: usize) -> Result<Generator, ParseError> {
    let (variable, variable_span) = cursor.identifier()?;
    cursor.char(':')?;
    let type_text = cursor.until_keyword_or(&["in"])?;
    let variable_type = parse_type(type_text.trim())?;
    cursor.keyword("in")?;

    let range_start_text = cursor.until_range_operator()?;
    let range_start_offset = range_start_text.len() - range_start_text.trim_start().len();
    let range_start = parse_expression_at(
        range_start_text.trim(),
        cursor.position_before(range_start_text) + range_start_offset,
    )?;
    let inclusive_end = if cursor.take_text("..=") {
        true
    } else if cursor.take_text("..") {
        // `..<` is retained as a compatibility spelling for the documented half-open form.
        let _ = cursor.take_char('<');
        false
    } else {
        return Err(ParseError::InvalidSyntax("generator range"));
    };

    let range_end_text = cursor.until_keyword_or(&["step"])?;
    let range_end_offset = range_end_text.len() - range_end_text.trim_start().len();
    let range_end = parse_expression_at(
        range_end_text.trim(),
        cursor.position_before(range_end_text) + range_end_offset,
    )?;
    cursor.keyword("step")?;

    let range_step_text = cursor.until_top_level(&['{'])?;
    let range_step_offset = range_step_text.len() - range_step_text.trim_start().len();
    let range_step = parse_expression_at(
        range_step_text.trim(),
        cursor.position_before(range_step_text) + range_step_offset,
    )?;
    if is_literal_zero(&range_step) {
        return Err(ParseError::InvalidSyntax("generator step"));
    }
    cursor.char('{')?;
    let body = parse_generator_items_until(cursor, '}')?;
    cursor.char('}')?;

    let range = SourceRange {
        span: SourceSpan::new(range_start.span().start, range_step.span().end),
        start: range_start,
        end: range_end,
        step: range_step,
        inclusive_end,
    };
    Ok(Generator {
        variable,
        variable_span,
        variable_type,
        range,
        body,
        span: SourceSpan::new(start, cursor.position()),
    })
}

fn parse_generator_items_until(
    cursor: &mut Cursor<'_>,
    terminator: char,
) -> Result<Vec<GeneratorItem>, ParseError> {
    let mut items = Vec::new();
    while !cursor.peek_is(terminator) {
        if cursor.take_keyword("if") {
            let start = cursor.position().saturating_sub(2);
            let condition_text = cursor.until_top_level(&['{'])?;
            let condition_offset = condition_text.len() - condition_text.trim_start().len();
            let condition = parse_expression_at(
                condition_text.trim(),
                cursor.position_before(condition_text) + condition_offset,
            )?;
            cursor.char('{')?;
            let then_items = parse_generator_items_until(cursor, '}')?;
            cursor.char('}')?;
            cursor.skip();
            let else_items = if cursor.take_keyword("else") {
                cursor.char('{')?;
                let items = parse_generator_items_until(cursor, '}')?;
                cursor.char('}')?;
                items
            } else {
                Vec::new()
            };
            items.push(GeneratorItem::Conditional {
                condition,
                then_items,
                else_items,
                span: SourceSpan::new(start, cursor.position()),
            });
        } else if cursor.take_keyword("emit") {
            let expression = parse_entity_expression(cursor)?;
            cursor.char(';')?;
            items.push(GeneratorItem::Emit(expression));
        } else {
            return Err(ParseError::InvalidSyntax("generator item"));
        }
    }
    Ok(items)
}

fn is_literal_zero(expression: &SourceExpression) -> bool {
    match expression {
        SourceExpression::Literal {
            literal: crate::v5::ast::SourceLiteral::Int(value),
            ..
        } => *value == 0,
        SourceExpression::Literal {
            literal: crate::v5::ast::SourceLiteral::Beat(value),
            ..
        } => value.numerator() == 0,
        _ => false,
    }
}

fn parse_entity_expression(cursor: &mut Cursor<'_>) -> Result<EntityExpression, ParseError> {
    cursor.skip();
    let mut base = if let Some(variant) = cursor.take_variant() {
        parse_constructor(cursor, variant).map(EntityExpression::Constructor)?
    } else {
        let expression_start = cursor.position();
        let expression_text = cursor.until_keyword_or(&["with", ";", "}"])?;
        let trimmed_start = expression_text.len() - expression_text.trim_start().len();
        let expression =
            parse_expression_at(expression_text.trim(), expression_start + trimmed_start)?;
        EntityExpression::Source(expression)
    };
    cursor.skip();
    if cursor.take_keyword("with") {
        let with_start = base.span().start;
        let fields = parse_field_block(cursor)?;
        let end = cursor.position();
        base = EntityExpression::With(WithExpression {
            base: Box::new(base),
            fields,
            span: SourceSpan::new(with_start, end),
        });
    }
    Ok(base)
}

fn parse_constructor(
    cursor: &mut Cursor<'_>,
    variant: ConstructorKind,
) -> Result<EntityConstructor, ParseError> {
    let start = variant.span().start;
    cursor.char('{')?;
    let fields = parse_fields_until(cursor, '}')?;
    cursor.char('}')?;
    Ok(EntityConstructor {
        entity_type: variant.entity_type(),
        note_variant: variant.note_variant(),
        fields,
        span: SourceSpan::new(start, cursor.position()),
    })
}

fn parse_field_block(cursor: &mut Cursor<'_>) -> Result<Vec<EntityField>, ParseError> {
    cursor.char('{')?;
    let fields = parse_fields_until(cursor, '}')?;
    cursor.char('}')?;
    Ok(fields)
}

fn parse_fields_until(
    cursor: &mut Cursor<'_>,
    terminator: char,
) -> Result<Vec<EntityField>, ParseError> {
    let mut fields = Vec::new();
    while !cursor.peek_is(terminator) {
        let start = cursor.position();
        let path_text = cursor.until_top_level(&[':'])?;
        let path_trimmed = path_text.trim();
        let path_offset = path_text.len() - path_text.trim_start().len();
        let segments = path_trimmed
            .split('.')
            .map(str::trim)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if segments.is_empty() || segments.iter().any(|segment| segment.is_empty()) {
            return Err(ParseError::InvalidSyntax("entity field"));
        }
        cursor.char(':')?;
        let value_text = cursor.until_top_level(&[';'])?;
        let value_offset = value_text.len() - value_text.trim_start().len();
        let value = parse_expression_at(
            value_text.trim(),
            cursor.position_before(value_text) + value_offset,
        )?;
        cursor.char(';')?;
        let end = cursor.position();
        fields.push(EntityField {
            path: FieldPath {
                segments,
                span: SourceSpan::new(
                    start + path_offset,
                    start + path_offset + path_trimmed.len(),
                ),
            },
            value,
            span: SourceSpan::new(start, end),
        });
    }
    Ok(fields)
}

#[derive(Clone, Copy)]
enum ConstructorKind {
    Note(NoteVariant, SourceSpan),
    Line(SourceSpan),
}

impl ConstructorKind {
    fn span(self) -> SourceSpan {
        match self {
            Self::Note(_, span) | Self::Line(span) => span,
        }
    }

    fn entity_type(self) -> Type {
        match self {
            Self::Note(_, _) => Type::Note,
            Self::Line(_) => Type::Line,
        }
    }

    fn note_variant(self) -> Option<NoteVariant> {
        match self {
            Self::Note(variant, _) => Some(variant),
            Self::Line(_) => None,
        }
    }
}

struct Cursor<'a> {
    input: &'a str,
    base: usize,
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str, base: usize) -> Self {
        Self {
            input,
            base,
            offset: 0,
        }
    }

    fn position(&self) -> usize {
        self.base + self.offset
    }

    fn position_before(&self, text: &str) -> usize {
        self.position().saturating_sub(text.len())
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.offset..]
    }

    fn done(&mut self) -> Result<bool, ParseError> {
        self.skip();
        Ok(self.offset == self.input.len())
    }

    fn skip(&mut self) {
        loop {
            let before = self.offset;
            while let Some(character) = self.remaining().chars().next() {
                if character.is_whitespace() {
                    self.offset += character.len_utf8();
                } else {
                    break;
                }
            }
            if self.remaining().starts_with("//") {
                if let Some(index) = self.remaining().find('\n') {
                    self.offset += index + 1;
                } else {
                    self.offset = self.input.len();
                }
            } else if self.remaining().starts_with("/*") {
                if let Some(index) = self.remaining()[2..].find("*/") {
                    self.offset += index + 4;
                } else {
                    self.offset = self.input.len();
                }
            }
            if before == self.offset {
                break;
            }
        }
    }

    fn peek_is(&mut self, expected: char) -> bool {
        self.skip();
        self.remaining().starts_with(expected)
    }

    fn take_char(&mut self, expected: char) -> bool {
        self.skip();
        if let Some(character) = self.remaining().chars().next()
            && character == expected
        {
            self.offset += character.len_utf8();
            return true;
        }
        false
    }

    fn take_text(&mut self, expected: &str) -> bool {
        self.skip();
        if self.remaining().starts_with(expected) {
            self.offset += expected.len();
            true
        } else {
            false
        }
    }

    fn char(&mut self, expected: char) -> Result<(), ParseError> {
        self.take_char(expected)
            .then_some(())
            .ok_or(ParseError::InvalidSyntax("entity syntax"))
    }

    fn identifier(&mut self) -> Result<(String, SourceSpan), ParseError> {
        self.skip();
        let start = self.position();
        let local_start = self.offset;
        let mut chars = self.remaining().char_indices();
        let Some((_, first)) = chars.next() else {
            return Err(ParseError::InvalidSyntax("identifier"));
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return Err(ParseError::InvalidSyntax("identifier"));
        }
        let mut end = first.len_utf8();
        for (index, character) in chars {
            if character.is_ascii_alphanumeric() || character == '_' {
                end = index + character.len_utf8();
            } else {
                break;
            }
        }
        self.offset = local_start + end;
        Ok((
            self.input[local_start..self.offset].to_owned(),
            SourceSpan::new(start, self.position()),
        ))
    }

    fn keyword(&mut self, keyword: &str) -> Result<(), ParseError> {
        self.take_keyword(keyword)
            .then_some(())
            .ok_or(ParseError::InvalidSyntax("keyword"))
    }

    fn take_keyword(&mut self, keyword: &str) -> bool {
        self.skip();
        let rest = self.remaining();
        if !rest.starts_with(keyword) {
            return false;
        }
        let next = rest[keyword.len()..].chars().next();
        if next.is_some_and(|character| character.is_ascii_alphanumeric() || character == '_') {
            return false;
        }
        self.offset += keyword.len();
        true
    }

    fn take_variant(&mut self) -> Option<ConstructorKind> {
        self.skip();
        let variants = [
            ("tap", NoteVariant::Tap),
            ("hold", NoteVariant::Hold),
            ("flick", NoteVariant::Flick),
            ("drag", NoteVariant::Drag),
        ];
        for (name, variant) in variants {
            if self.take_keyword(name) {
                return Some(ConstructorKind::Note(
                    variant,
                    SourceSpan::new(self.position() - name.len(), self.position()),
                ));
            }
        }
        if self.take_keyword("Line") {
            return Some(ConstructorKind::Line(SourceSpan::new(
                self.position() - 4,
                self.position(),
            )));
        }
        None
    }

    fn until_top_level(&mut self, delimiters: &[char]) -> Result<&'a str, ParseError> {
        self.skip();
        let start = self.offset;
        let mut parentheses = 0usize;
        let mut angle = 0usize;
        let mut string = false;
        let mut escaped = false;
        for (index, character) in self.remaining().char_indices() {
            if string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    string = false;
                }
                continue;
            }
            match character {
                '"' => string = true,
                '(' => parentheses += 1,
                ')' if parentheses > 0 => parentheses -= 1,
                '<' => angle += 1,
                '>' if angle > 0 => angle -= 1,
                _ if parentheses == 0 && angle == 0 && delimiters.contains(&character) => {
                    self.offset = start + index;
                    return Ok(&self.input[start..self.offset]);
                }
                _ => {}
            }
        }
        Err(ParseError::InvalidSyntax("entity expression"))
    }

    fn until_range_operator(&mut self) -> Result<&'a str, ParseError> {
        self.skip();
        let start = self.offset;
        let mut parentheses = 0usize;
        let mut string = false;
        let mut escaped = false;
        for (index, character) in self.remaining().char_indices() {
            if string {
                if escaped {
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    string = false;
                }
                continue;
            }
            match character {
                '"' => string = true,
                '(' => parentheses += 1,
                ')' if parentheses > 0 => parentheses -= 1,
                '.' if parentheses == 0 && self.input[start + index..].starts_with("..") => {
                    self.offset = start + index;
                    return Ok(&self.input[start..self.offset]);
                }
                _ => {}
            }
        }
        Err(ParseError::InvalidSyntax("generator range"))
    }

    fn until_keyword_or(&mut self, keywords: &[&str]) -> Result<&'a str, ParseError> {
        self.skip();
        let start = self.offset;
        let mut parentheses = 0usize;
        for (index, character) in self.remaining().char_indices() {
            match character {
                '(' => parentheses += 1,
                ')' if parentheses > 0 => parentheses -= 1,
                ';' | '}' if parentheses == 0 => {
                    self.offset = start + index;
                    return Ok(&self.input[start..self.offset]);
                }
                _ if parentheses == 0 => {
                    let tail = &self.input[start + index..];
                    if keywords.iter().any(|keyword| {
                        tail.starts_with(keyword)
                            && tail[keyword.len()..]
                                .chars()
                                .next()
                                .is_none_or(|c| c.is_whitespace() || c == '{')
                    }) {
                        self.offset = start + index;
                        return Ok(&self.input[start..self.offset]);
                    }
                }
                _ => {}
            }
        }
        Err(ParseError::InvalidSyntax("entity expression"))
    }
}
