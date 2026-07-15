use chumsky::{input::ValueInput, prelude::*};

use crate::ast::{
    ConstDeclaration, Definition, DefinitionsBlock, FunctionDeclaration, FunctionParameter,
    FunctionStatement, IfStatement, LetStatement, ReturnEntityStatement, ReturnStatement,
    SourceSpan, TemplateDeclaration, TemplateIfStatement, TemplateParameter, TemplateStatement,
};

use super::{
    entities::entity_expression_parser,
    expression::{expression_parser, type_parser},
    input::{ChumskySpan, ParserExtra, source_span},
    token::{Keyword, Punctuation, Token},
};

pub(super) fn definitions_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, DefinitionsBlock, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Definitions))
        .ignore_then(
            definition_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace())),
        )
        .map_with(|declarations, extra| DefinitionsBlock {
            declarations,
            span: source_span(extra.span()),
        })
}

fn definition_parser<'tokens, I>()
-> impl Parser<'tokens, I, Definition, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    choice((
        const_declaration_parser().map(Definition::Const),
        function_declaration_parser().map(Definition::Function),
        template_declaration_parser().map(Definition::Template),
    ))
}

fn const_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, ConstDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Const))
        .ignore_then(identifier_with_span())
        .then_ignore(just(colon()))
        .then(type_parser())
        .then_ignore(just(equal()))
        .then(expression_parser())
        .then_ignore(just(semicolon()))
        .map_with(
            |(((name, name_span), ty), initializer), extra| ConstDeclaration {
                name,
                name_span,
                ty,
                initializer,
                span: source_span(extra.span()),
            },
        )
}

fn function_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, FunctionDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Fn))
        .ignore_then(function_name_with_span())
        .then(function_parameters_parser())
        .then_ignore(just(Token::Punctuation(Punctuation::Arrow)))
        .then(type_parser())
        .then(function_block_parser())
        .map_with(
            |((((name, name_span), parameters), return_type), body), extra| FunctionDeclaration {
                name,
                name_span,
                parameters,
                return_type,
                body,
                span: source_span(extra.span()),
            },
        )
}

fn function_parameters_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<FunctionParameter>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    identifier_with_span()
        .then_ignore(just(colon()))
        .then(type_parser())
        .map_with(|((name, name_span), ty), extra| FunctionParameter {
            name,
            name_span,
            ty,
            span: source_span(extra.span()),
        })
        .separated_by(just(comma()))
        .allow_trailing()
        .collect()
        .delimited_by(just(left_parenthesis()), just(right_parenthesis()))
}

fn function_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<FunctionStatement>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    function_statement_parser()
        .repeated()
        .collect()
        .delimited_by(just(left_brace()), just(right_brace()))
}

fn function_statement_parser<'tokens, I>()
-> impl Parser<'tokens, I, FunctionStatement, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|statement| {
        let block = statement
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(left_brace()), just(right_brace()));
        let conditional = just(Token::Keyword(Keyword::If))
            .ignore_then(expression_parser())
            .then(block.clone())
            .then(
                just(Token::Keyword(Keyword::Else))
                    .ignore_then(block)
                    .or_not(),
            )
            .map_with(|((condition, then_branch), else_branch), extra| {
                FunctionStatement::If(IfStatement {
                    condition,
                    then_branch,
                    else_branch: else_branch.unwrap_or_default(),
                    span: source_span(extra.span()),
                })
            });
        choice((
            let_statement_parser().map(FunctionStatement::Let),
            just(Token::Keyword(Keyword::Return))
                .ignore_then(expression_parser())
                .then_ignore(just(semicolon()))
                .map_with(|value, extra| {
                    FunctionStatement::Return(ReturnStatement {
                        value,
                        span: source_span(extra.span()),
                    })
                }),
            conditional,
        ))
    })
}

pub(super) fn let_statement_parser<'tokens, I>()
-> impl Parser<'tokens, I, LetStatement, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Let))
        .ignore_then(identifier_with_span())
        .then_ignore(just(colon()))
        .then(type_parser())
        .then_ignore(just(equal()))
        .then(expression_parser())
        .then_ignore(just(semicolon()))
        .map_with(
            |(((name, name_span), ty), initializer), extra| LetStatement {
                name,
                name_span,
                ty,
                initializer,
                span: source_span(extra.span()),
            },
        )
}

fn template_declaration_parser<'tokens, I>()
-> impl Parser<'tokens, I, TemplateDeclaration, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(Token::Keyword(Keyword::Template))
        .ignore_then(type_parser())
        .then(identifier_with_span())
        .then(template_parameters_parser())
        .then(template_block_parser())
        .map_with(
            |(((return_type, (name, name_span)), parameters), body), extra| TemplateDeclaration {
                return_type,
                name,
                name_span,
                parameters,
                body,
                span: source_span(extra.span()),
            },
        )
}

fn template_parameters_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<TemplateParameter>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    identifier_with_span()
        .then_ignore(just(colon()))
        .then(type_parser())
        .map_with(|((name, name_span), ty), extra| TemplateParameter {
            name,
            name_span,
            ty,
            span: source_span(extra.span()),
        })
        .separated_by(just(comma()))
        .allow_trailing()
        .collect()
        .delimited_by(just(left_parenthesis()), just(right_parenthesis()))
}

fn template_block_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<TemplateStatement>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    template_statement_parser()
        .repeated()
        .collect()
        .delimited_by(just(left_brace()), just(right_brace()))
}

fn template_statement_parser<'tokens, I>()
-> impl Parser<'tokens, I, TemplateStatement, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|statement| {
        let block = statement
            .clone()
            .repeated()
            .collect::<Vec<_>>()
            .delimited_by(just(left_brace()), just(right_brace()));
        let conditional = just(Token::Keyword(Keyword::If))
            .ignore_then(expression_parser())
            .then(block.clone())
            .then(
                just(Token::Keyword(Keyword::Else))
                    .ignore_then(block)
                    .or_not(),
            )
            .map_with(|((condition, then_branch), else_branch), extra| {
                TemplateStatement::If(TemplateIfStatement {
                    condition,
                    then_branch,
                    else_branch: else_branch.unwrap_or_default(),
                    span: source_span(extra.span()),
                })
            });
        choice((
            let_statement_parser().map(TemplateStatement::Let),
            just(Token::Keyword(Keyword::Return))
                .ignore_then(entity_expression_parser())
                .then_ignore(just(semicolon()))
                .map_with(|value, extra| {
                    TemplateStatement::Return(ReturnEntityStatement {
                        value,
                        span: source_span(extra.span()),
                    })
                }),
            conditional,
        ))
    })
}

pub(super) fn identifier_with_span<'tokens, I>()
-> impl Parser<'tokens, I, (String, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! { Token::Identifier(name) => name }
        .map_with(|name, extra| (name, source_span(extra.span())))
}

fn function_name_with_span<'tokens, I>()
-> impl Parser<'tokens, I, (String, SourceSpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(name) => name,
        Token::Keyword(Keyword::Choose) => "choose".to_owned(),
    }
    .map_with(|name, extra| (name, source_span(extra.span())))
}

fn left_parenthesis() -> Token {
    Token::Punctuation(Punctuation::LeftParenthesis)
}
fn right_parenthesis() -> Token {
    Token::Punctuation(Punctuation::RightParenthesis)
}
fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}
fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
fn colon() -> Token {
    Token::Punctuation(Punctuation::Colon)
}
fn comma() -> Token {
    Token::Punctuation(Punctuation::Comma)
}
fn equal() -> Token {
    Token::Punctuation(Punctuation::Equal)
}
fn semicolon() -> Token {
    Token::Punctuation(Punctuation::Semicolon)
}
