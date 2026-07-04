//! String table — null-terminated UTF-8 strings referenced by u32 offset.
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct StringTableBuilder { strings: Vec<String>, offset_map: HashMap<String, u32>, total_size: u32 }

impl StringTableBuilder {
    pub fn new() -> Self { Self::default() }
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&off) = self.offset_map.get(s) { return off; }
        let off = self.total_size;
        self.total_size += s.len() as u32 + 1;
        self.strings.push(s.to_string());
        self.offset_map.insert(s.to_string(), off);
        off
    }
    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.total_size as usize);
        for s in &self.strings { buf.extend_from_slice(s.as_bytes()); buf.push(0); }
        buf
    }
    pub fn size(&self) -> u32 { self.total_size }
}

pub struct StringTableReader<'a> { data: &'a [u8] }
impl<'a> StringTableReader<'a> {
    pub fn new(data: &'a [u8]) -> Self { Self { data } }
    pub fn get(&self, offset: u32) -> Option<&'a str> {
        let start = offset as usize;
        if start >= self.data.len() { return None; }
        let end = self.data[start..].iter().position(|&b| b == 0)?;
        std::str::from_utf8(&self.data[start..start + end]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_intern_dedup() {
        let mut b = StringTableBuilder::new();
        let o1 = b.intern("hello");
        let o2 = b.intern("hello");
        assert_eq!(o1, o2);
    }
    #[test]
    fn test_roundtrip() {
        let mut b = StringTableBuilder::new();
        let o = b.intern("alpha");
        let bytes = b.build();
        let r = StringTableReader::new(&bytes);
        assert_eq!(r.get(o).unwrap(), "alpha");
    }
}
