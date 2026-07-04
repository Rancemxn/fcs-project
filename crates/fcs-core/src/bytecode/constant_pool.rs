//! Constant pool — deduplicated f64 values referenced by u16 index.
use std::collections::BTreeMap;

#[derive(Debug, Default, Clone)]
pub struct ConstantPoolBuilder { values: Vec<f64>, indices: BTreeMap<u64, u16> }

impl ConstantPoolBuilder {
    pub fn new() -> Self { Self::default() }
    pub fn intern(&mut self, value: f64) -> u16 {
        let bits = value.to_bits();
        if let Some(&idx) = self.indices.get(&bits) { return idx; }
        let idx = self.values.len() as u16;
        self.values.push(value);
        self.indices.insert(bits, idx);
        idx
    }
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.values.len() * 8);
        for &v in &self.values { buf.extend_from_slice(&v.to_le_bytes()); }
        buf
    }
    pub fn size(&self) -> u32 { self.values.len() as u32 * 8 }
    pub fn len(&self) -> usize { self.values.len() }
    pub fn is_empty(&self) -> bool { self.values.is_empty() }
    pub fn as_slice(&self) -> &[f64] { &self.values }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_dedup() {
        let mut p = ConstantPoolBuilder::new();
        assert_eq!(p.intern(3.14), p.intern(3.14));
        assert_eq!(p.len(), 1);
    }
}
