use chumsky::{
    error::{Rich, RichReason},
    input::{Input as _, ValueInput},
    prelude::*,
};
use fcs_model::CanonicalRenderNodeKind;

use crate::ast::{
    GeneratorOwner, RenderBlock, RenderBodyItem, RenderChildrenBlock, RenderIf, RenderItem,
    RenderLayerDeclaration, RenderNodeDeclaration, RenderScene, RenderViewport, SourceSpan,
    render_node_kind_from_spelling,
};
use crate::diagnostic::{
    Diagnostic, DiagnosticCode, DiagnosticLabel, DiagnosticStage, ParseOutput,
};

use super::{
    MISPLACED_GENERATOR_ERROR, NESTED_GENERATOR_ERROR, ParseLimits,
    definitions::identifier_with_span,
    entities::{
        entity_expression_parser, generator_parser, render_schema_field_parser,
        render_schema_fields_parser,
    },
    expression::expression_parser,
    input::{ChumskySpan, ParserExtra, SpannedToken, source_span},
    lexer::lex,
    token::{Keyword, Punctuation, Token},
    tracks::tracks_block_parser,
};

/// Marker for an identifier in node-kind position that resolves to no kind.
/// The stable category stays `syntax.invalid-token` (`fcs-render.md` section 17).
const UNKNOWN_NODE_KIND_ERROR: &str = "unknown render node kind";

/// Parses the preserved payload of a Core [`RenderBlock`] against the
/// `fcs-render.md` section 2 grammar.
///
/// `source` must be the exact text the Core parser produced `block` from, so
/// every produced span is a byte span into that same source. The parse is
/// grammar-shaped only: field meaning, enum membership, node-kind schema and
/// the profile-version gate belong to Render static validation.
pub fn parse_render_scene(source: &str, block: &RenderBlock) -> ParseOutput<RenderScene> {
    parse_render_scene_with_limits(source, block, ParseLimits::default())
}

/// Parses a preserved Render payload with explicit resource limits.
pub fn parse_render_scene_with_limits<L: Into<ParseLimits>>(
    source: &str,
    block: &RenderBlock,
    limits: L,
) -> ParseOutput<RenderScene> {
    let payload_span = block.payload.span;
    let payload = source
        .get(payload_span.start..payload_span.end)
        .expect("render payload span must lie within its parsed source");
    match lex(payload, limits.into()) {
        Ok(tokens) => {
            let tokens = tokens
                .into_iter()
                .map(|(token, span)| (token, offset_chumsky_span(span, payload_span.start)))
                .collect::<Vec<_>>();
            parse_render_tokens(payload_span, &tokens)
        }
        Err(diagnostics) => ParseOutput::new(
            None,
            diagnostics
                .into_iter()
                .map(|diagnostic| offset_diagnostic(diagnostic, payload_span.start))
                .collect(),
        ),
    }
}

fn offset_chumsky_span(span: ChumskySpan, offset: usize) -> ChumskySpan {
    ChumskySpan::new((), span.start + offset..span.end + offset)
}

/// Rebases a payload-relative lexer diagnostic onto the enclosing source.
fn offset_diagnostic(diagnostic: Diagnostic, offset: usize) -> Diagnostic {
    let span = diagnostic.primary_span();
    let mut moved = Diagnostic::new(
        diagnostic.code(),
        diagnostic.stage(),
        diagnostic.message().to_owned(),
        SourceSpan::new(span.start + offset, span.end + offset),
    );
    for label in diagnostic.labels() {
        let span = label.span();
        moved = moved.with_label(DiagnosticLabel::new(
            SourceSpan::new(span.start + offset, span.end + offset),
            label.message().to_owned(),
        ));
    }
    if let Some(budget) = diagnostic.budget() {
        moved = moved.with_budget(budget.kind().to_owned(), budget.limit(), budget.observed());
    }
    moved
}

fn parse_render_tokens(
    payload_span: SourceSpan,
    tokens: &[SpannedToken],
) -> ParseOutput<RenderScene> {
    let end_span = ChumskySpan::new((), payload_span.end..payload_span.end);
    let input = tokens.map(end_span, |(token, span)| (token, span));
    let (parsed, errors) = render_payload_parser()
        .then_ignore(end())
        .parse(input)
        .into_output_errors();
    if !errors.is_empty() {
        let diagnostics = errors
            .into_iter()
            .map(parse_error_diagnostic)
            .collect::<Vec<_>>();
        return ParseOutput::new(None, diagnostics);
    }

    let parsed = parsed.expect("render payload parser produces output when it has no errors");
    finish_render_scene(parsed)
}

fn parse_error_diagnostic(error: Rich<'_, Token, ChumskySpan>) -> Diagnostic {
    let span = source_span(*error.span());
    let (code, message) = match error.reason() {
        RichReason::Custom(kind) if kind == NESTED_GENERATOR_ERROR => (
            DiagnosticCode::COMPILE_TIME_NESTED_GENERATOR,
            "nested generator is not allowed in a generator body",
        ),
        RichReason::Custom(kind) if kind == MISPLACED_GENERATOR_ERROR => (
            DiagnosticCode::COMPILE_TIME_MISPLACED_GENERATOR,
            "generator is not allowed in this owner",
        ),
        RichReason::Custom(kind) if kind == UNKNOWN_NODE_KIND_ERROR => (
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            UNKNOWN_NODE_KIND_ERROR,
        ),
        _ => (
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            "invalid render payload syntax",
        ),
    };
    Diagnostic::new(code, DiagnosticStage::Parse, message, span)
}

/// Applies the section 2 viewport rule: syntactically exactly one, before any
/// layer. The categories mirror the Core envelope conventions; the missing
/// case is pinned to `syntax.invalid-token` by `fcs-render.md` section 17.
fn finish_render_scene(parsed: ParsedPayload) -> ParseOutput<RenderScene> {
    let missing_span = parsed
        .items
        .first()
        .map_or(parsed.close_span, ProfileItem::span);

    let mut viewport: Option<RenderViewport> = None;
    let mut layers = Vec::new();
    let mut diagnostics = Vec::new();
    for item in parsed.items {
        match item {
            ProfileItem::Viewport(candidate) => {
                if let Some(first) = viewport.as_ref() {
                    diagnostics.push(
                        Diagnostic::new(
                            DiagnosticCode::NAME_DUPLICATE,
                            DiagnosticStage::Parse,
                            "viewport is declared more than once",
                            candidate.keyword_span,
                        )
                        .with_label(DiagnosticLabel::new(
                            first.keyword_span,
                            "first declaration",
                        )),
                    );
                } else {
                    if !layers.is_empty() {
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::SYNTAX_MISPLACED_BLOCK,
                            DiagnosticStage::Parse,
                            "viewport must be declared before any layer",
                            candidate.keyword_span,
                        ));
                    }
                    viewport = Some(candidate);
                }
            }
            ProfileItem::Layer(layer) => layers.push(layer),
        }
    }

    let Some(viewport) = viewport else {
        diagnostics.push(Diagnostic::new(
            DiagnosticCode::SYNTAX_INVALID_TOKEN,
            DiagnosticStage::Parse,
            "render profile requires a viewport block",
            missing_span,
        ));
        return ParseOutput::new(None, diagnostics);
    };
    ParseOutput::new(
        Some(RenderScene {
            viewport,
            layers,
            span: parsed.span,
        }),
        diagnostics,
    )
}

fn render_payload_parser<'tokens, I>()
-> impl Parser<'tokens, I, ParsedPayload, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    just(left_brace())
        .ignore_then(profile_item_parser().repeated().collect::<Vec<_>>())
        .then(just(right_brace()).map_with(|_, extra| source_span(extra.span())))
        .map_with(|(items, close_span), extra| ParsedPayload {
            items: items.into_iter().flatten().collect(),
            close_span,
            span: source_span(extra.span()),
        })
}

fn profile_item_parser<'tokens, I>()
-> impl Parser<'tokens, I, Option<ProfileItem>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    let viewport = contextual_keyword("viewport")
        .then(render_schema_fields_parser())
        .map_with(|(keyword_span, fields), extra| RenderViewport {
            fields,
            span: source_span(extra.span()),
            keyword_span,
        });
    let layer = contextual_keyword("layer")
        .ignore_then(identifier_with_span())
        .then(render_body_parser())
        .map_with(|((name, name_span), mut items), extra| {
            assign_render_generator_owners(&mut items, &name);
            RenderLayerDeclaration {
                name,
                name_span,
                items,
                span: source_span(extra.span()),
            }
        });
    choice((
        viewport.map(|viewport| Some(ProfileItem::Viewport(viewport))),
        layer.map(|layer| Some(ProfileItem::Layer(layer))),
        misplaced_generator_parser().map(|()| None),
    ))
}

fn render_body_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<RenderBodyItem>, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    recursive(|render_body| {
        let render_item = recursive(|render_item| {
            let branch = render_item
                .clone()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(left_brace()), just(right_brace()));
            let render_if = just(Token::Keyword(Keyword::If))
                .map_with(|_, extra| source_span(extra.span()))
                .then(expression_parser())
                .then(branch.clone())
                .then(
                    just(Token::Keyword(Keyword::Else))
                        .ignore_then(branch)
                        .or_not(),
                )
                .map_with(
                    |(((keyword_span, condition), then_items), else_items), extra| RenderIf {
                        condition,
                        then_items,
                        else_items: else_items.unwrap_or_default(),
                        span: source_span(extra.span()),
                        keyword_span,
                    },
                );
            // The structural node shape is committed before the kind resolves,
            // so an unknown or miscased spelling reports at the kind token
            // instead of degrading into the entity-expression alternative.
            let node = node_kind_spelling_parser()
                .then(identifier_with_span())
                .then(render_body.clone())
                .validate(
                    |(((spelling, kind_span), (name, name_span)), mut items), extra, emitter| {
                        let kind = render_node_kind_from_spelling(&spelling).unwrap_or_else(|| {
                            emitter.emit(Rich::custom(kind_span, UNKNOWN_NODE_KIND_ERROR));
                            CanonicalRenderNodeKind::Group
                        });
                        assign_render_generator_owners(&mut items, &name);
                        RenderNodeDeclaration {
                            kind,
                            kind_span: source_span(kind_span),
                            name,
                            name_span,
                            items,
                            span: source_span(extra.span()),
                        }
                    },
                );
            choice((
                render_if.map(RenderItem::If),
                generator_parser(render_children_owner_placeholder()).map(RenderItem::Generator),
                node.map(RenderItem::Node),
                entity_expression_parser()
                    .then_ignore(just(semicolon()))
                    .map(RenderItem::Entity),
            ))
        });
        let children = contextual_keyword("children")
            .then(
                render_item
                    .repeated()
                    .collect::<Vec<_>>()
                    .delimited_by(just(left_brace()), just(right_brace())),
            )
            .map_with(|(keyword_span, items), extra| RenderChildrenBlock {
                items,
                span: source_span(extra.span()),
                keyword_span,
            });
        // `children`/`tracks` win over a schema field only via their `{`;
        // `children: ...;` and `generate: ...;` stay ordinary field paths, so
        // the misplaced-generator guard runs after the field alternative.
        let member = choice((
            children.map(|children| Some(RenderBodyItem::Children(children))),
            tracks_block_parser().map(|tracks| Some(RenderBodyItem::Tracks(tracks))),
            render_schema_field_parser().map(|field| Some(RenderBodyItem::Field(Box::new(field)))),
            misplaced_generator_parser().map(|()| None),
        ));
        member
            .repeated()
            .collect::<Vec<Option<RenderBodyItem>>>()
            .map(|items| items.into_iter().flatten().collect::<Vec<RenderBodyItem>>())
            .delimited_by(just(left_brace()), just(right_brace()))
    })
}

/// Recognizes a complete generator production where the grammar forbids one
/// and reports it as misplaced rather than as a plain invalid token
/// (`fcs.md` Appendix B). The emitted item is dropped by the caller.
fn misplaced_generator_parser<'tokens, I>()
-> impl Parser<'tokens, I, (), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    generator_parser(render_children_owner_placeholder()).validate(|generator, _, emitter| {
        let keyword_end = generator.span.start + "generate".len();
        emitter.emit(Rich::custom(
            ChumskySpan::new((), generator.span.start..keyword_end),
            MISPLACED_GENERATOR_ERROR,
        ));
    })
}

/// Matches one Render contextual keyword, which the Core lexer emits as a
/// plain identifier, and yields its span.
fn contextual_keyword<'tokens, I>(
    word: &'static str,
) -> impl Parser<'tokens, I, SourceSpan, ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    any()
        .filter(move |token: &Token| matches!(token, Token::Identifier(name) if name == word))
        .map_with(|_, extra| source_span(extra.span()))
}

/// Matches a token in node-kind position. `line`, `image` and `path` are Core
/// keywords; every other section 2 kind spelling is a plain identifier.
fn node_kind_spelling_parser<'tokens, I>()
-> impl Parser<'tokens, I, (String, ChumskySpan), ParserExtra<'tokens>> + Clone
where
    I: ValueInput<'tokens, Token = Token, Span = ChumskySpan>,
{
    select! {
        Token::Identifier(spelling) => spelling,
        Token::Keyword(Keyword::Line) => "line".to_owned(),
        Token::Keyword(Keyword::Image) => "image".to_owned(),
        Token::Keyword(Keyword::Path) => "path".to_owned(),
    }
    .map_with(|spelling, extra| (spelling, extra.span()))
}

/// Rewrites the placeholder owner of every generator directly or
/// conditionally contained by `items`' children blocks to the enclosing layer
/// or node name. Nested node declarations already own their generators.
fn assign_render_generator_owners(items: &mut Vec<RenderBodyItem>, owner: &str) {
    for item in items {
        if let RenderBodyItem::Children(children) = item {
            assign_render_item_generator_owners(&mut children.items, owner);
        }
    }
}

fn assign_render_item_generator_owners(items: &mut [RenderItem], owner: &str) {
    for item in items {
        match item {
            RenderItem::Generator(generator) => {
                *generator.owner = GeneratorOwner::RenderChildren {
                    name: owner.to_owned(),
                };
            }
            RenderItem::If(branch) => {
                assign_render_item_generator_owners(&mut branch.then_items, owner);
                assign_render_item_generator_owners(&mut branch.else_items, owner);
            }
            RenderItem::Node(_) | RenderItem::Entity(_) => {}
        }
    }
}

fn render_children_owner_placeholder() -> GeneratorOwner {
    GeneratorOwner::RenderChildren {
        name: String::new(),
    }
}

fn left_brace() -> Token {
    Token::Punctuation(Punctuation::LeftBrace)
}
fn right_brace() -> Token {
    Token::Punctuation(Punctuation::RightBrace)
}
fn semicolon() -> Token {
    Token::Punctuation(Punctuation::Semicolon)
}

#[derive(Debug)]
struct ParsedPayload {
    items: Vec<ProfileItem>,
    close_span: SourceSpan,
    span: SourceSpan,
}

#[derive(Debug)]
enum ProfileItem {
    Viewport(RenderViewport),
    Layer(RenderLayerDeclaration),
}

impl ProfileItem {
    const fn span(&self) -> SourceSpan {
        match self {
            Self::Viewport(viewport) => viewport.span,
            Self::Layer(layer) => layer.span,
        }
    }
}
