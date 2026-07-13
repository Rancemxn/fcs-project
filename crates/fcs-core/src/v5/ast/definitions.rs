//! Source declarations for the FCS 5 compile-time language.

use super::{SourceExpression, SourceSpan, Type};

#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionsBlock {
    pub declarations: Vec<Definition>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Definition {
    Const(ConstDeclaration),
    Function(FunctionDeclaration),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConstDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub initializer: SourceExpression,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParameter {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDeclaration {
    pub name: String,
    pub name_span: SourceSpan,
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Type,
    pub body: Vec<FunctionStatement>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionStatement {
    Let(LetStatement),
    Return(ReturnStatement),
    If(IfStatement),
}

impl FunctionStatement {
    pub const fn span(&self) -> SourceSpan {
        match self {
            Self::Let(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::If(statement) => statement.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetStatement {
    pub name: String,
    pub name_span: SourceSpan,
    pub ty: Type,
    pub initializer: SourceExpression,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReturnStatement {
    pub value: SourceExpression,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IfStatement {
    pub condition: SourceExpression,
    pub then_branch: Vec<FunctionStatement>,
    pub else_branch: Vec<FunctionStatement>,
    pub span: SourceSpan,
}
