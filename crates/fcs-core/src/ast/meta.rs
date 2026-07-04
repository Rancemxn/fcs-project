//! Meta block AST (§5.2).
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum MetaValue { String(String), Int(i64), Float(f64), Bool(bool), StringArray(Vec<String>) }

#[derive(Debug, Clone, PartialEq)]
pub struct MetaBlock {
    pub name: String,
    pub artists: Vec<String>,
    pub charters: Vec<String>,
    pub offset: f64,
    pub offset_unit: String,
    pub version: String,
    pub extra: BTreeMap<String, MetaValue>,
}
