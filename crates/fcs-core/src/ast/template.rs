//! Template block AST (§5.4).
use super::expr::{Expression, ValueType};
use super::note::NotePropertyValue;

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateParam {
    pub name: String,
    pub ty: ValueType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TemplateStatement {
    If {
        condition: Expression,
        body: Vec<TemplateStatement>,
        else_body: Option<Vec<TemplateStatement>>,
    },
    Return(Expression),
    Assign {
        property: String,
        value: NotePropertyValue,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDef {
    pub name: String,
    pub params: Vec<TemplateParam>,
    pub body: Vec<TemplateStatement>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TemplateBlock {
    pub definitions: Vec<TemplateDef>,
}
