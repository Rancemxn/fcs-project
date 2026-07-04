//! Note prototype inheritance flattening (§5.6.5). Zero-cost at runtime.
use crate::ast::{NoteInstance, NotePropertyValue};
use crate::compiler::context::CompileContext;
use crate::error::CompileError;

pub fn flatten_prototype(
    instance: &NoteInstance, line_name: &str, ctx: &CompileContext,
) -> Result<Vec<(String, NotePropertyValue)>, CompileError> {
    let mut props: Vec<(String, NotePropertyValue)> = Vec::new();
    let mut chain = Vec::new();
    let mut current = instance.parent.as_deref();
    while let Some(parent_name) = current {
        if chain.contains(&parent_name) {
            return Err(CompileError::CircularTemplate(parent_name.to_string()));
        }
        chain.push(parent_name);
        let proto = ctx.prototypes.get(line_name).and_then(|pmap| pmap.get(parent_name))
            .ok_or_else(|| CompileError::UndefinedTemplate(parent_name.to_string()))?;
        for (key, val) in &proto.properties {
            if !props.iter().any(|(k,_)| k == key) { props.push((key.clone(), val.clone())); }
        }
        current = proto.parent.as_deref();
    }
    for (key, val) in &instance.properties {
        if let Some(pos) = props.iter().position(|(k,_)| k == key) { props[pos] = (key.clone(), val.clone()); }
        else { props.push((key.clone(), val.clone())); }
    }
    Ok(props)
}

pub fn get_property<'a>(props: &'a [(String, NotePropertyValue)], key: &str) -> Option<&'a NotePropertyValue> {
    props.iter().find(|(k,_)| k == key).map(|(_, v)| v)
}
