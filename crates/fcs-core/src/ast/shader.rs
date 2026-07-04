//! Shader block AST (§5.7).
use super::expr::Expression;

#[derive(Debug, Clone, PartialEq)]
pub struct UniformBind { pub name: String, pub expression: Expression }

#[derive(Debug, Clone, PartialEq)]
pub struct ShaderDef {
    pub name: String, pub vertex_path: String, pub fragment_path: String,
    pub binds: Vec<UniformBind>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ShaderBlock { pub shaders: Vec<ShaderDef> }
