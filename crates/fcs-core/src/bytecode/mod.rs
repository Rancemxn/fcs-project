//! .fcbc binary format — top-level structures, serialization and deserialization.

pub mod constant_pool;
pub mod header;
pub mod property;
pub mod sections;
pub mod string_table;

use constant_pool::ConstantPoolBuilder;
use header::FcbcHeader;
use sections::{ExpressionEntry, LineSection, MasterTimelineSection, MetaSection, ShaderEntry};
use string_table::StringTableBuilder;

/// The complete .fcbc file representation in memory.
#[derive(Debug, Clone)]
pub struct FcbcFile {
    pub header: FcbcHeader,
    pub string_table: StringTableBuilder,
    pub const_pool: ConstantPoolBuilder,
    pub meta: MetaSection,
    pub master_timeline: MasterTimelineSection,
    pub lines: Vec<LineSection>,
    pub expressions: Vec<ExpressionEntry>,
    pub shaders: Vec<ShaderEntry>,
}

impl FcbcFile {
    /// Create a new, empty FcbcFile with default header.
    pub fn new() -> Self {
        Self {
            header: FcbcHeader::new(28, 0, 0, 0, 0),
            string_table: StringTableBuilder::new(),
            const_pool: ConstantPoolBuilder::new(),
            meta: MetaSection {
                name_st_off: 0,
                artist_st_offs: vec![],
                charter_st_offs: vec![],
                offset_seconds: 0.0,
            },
            master_timeline: MasterTimelineSection { entries: vec![] },
            lines: vec![],
            expressions: vec![],
            shaders: vec![],
        }
    }

    /// Serialize to bytes (simplified — writes sections sequentially).
    pub fn to_bytes(&self) -> Vec<u8> {
        // Phase 1: build string table and constant pool
        let st_bytes = self.string_table.build();
        let cp_bytes = self.const_pool.build();

        let st_off = FcbcHeader::SIZE as u32;
        let st_size = st_bytes.len() as u32;
        let cp_off = st_off + st_size;
        let cp_size = cp_bytes.len() as u32;

        let has_expr = !self.expressions.is_empty();
        let has_shader = !self.shaders.is_empty();
        let mut flags = 0u32;
        if has_shader {
            flags |= header::FLAG_HAS_SHADER;
        }
        if has_expr {
            flags |= header::FLAG_HAS_EXPRESSION;
        }

        let header = FcbcHeader::new(st_off, st_size, cp_off, cp_size, flags);

        let mut buf = Vec::new();
        // Write header
        buf.extend_from_slice(&header.magic);
        buf.extend_from_slice(&header.version.to_le_bytes());
        buf.extend_from_slice(&header.flags.to_le_bytes());
        buf.extend_from_slice(&header.string_table_offset.to_le_bytes());
        buf.extend_from_slice(&header.string_table_size.to_le_bytes());
        buf.extend_from_slice(&header.constant_pool_offset.to_le_bytes());
        buf.extend_from_slice(&header.constant_pool_size.to_le_bytes());

        // Write string table
        buf.extend_from_slice(&st_bytes);
        // Write constant pool
        buf.extend_from_slice(&cp_bytes);

        // Write meta (simplified)
        buf.extend_from_slice(&self.meta.name_st_off.to_le_bytes());
        buf.extend_from_slice(&(self.meta.artist_st_offs.len() as u32).to_le_bytes());
        for &a in &self.meta.artist_st_offs {
            buf.extend_from_slice(&a.to_le_bytes());
        }
        buf.extend_from_slice(&(self.meta.charter_st_offs.len() as u32).to_le_bytes());
        for &c in &self.meta.charter_st_offs {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        buf.extend_from_slice(&self.meta.offset_seconds.to_le_bytes());

        // Write master timeline
        buf.extend_from_slice(&(self.master_timeline.entries.len() as u32).to_le_bytes());
        for e in &self.master_timeline.entries {
            buf.extend_from_slice(&e.beat.to_le_bytes());
            buf.extend_from_slice(&e.accumulated_sec.to_le_bytes());
            buf.extend_from_slice(&e.bpm.to_le_bytes());
        }

        // Write lines
        buf.extend_from_slice(&(self.lines.len() as u32).to_le_bytes());
        for line in &self.lines {
            let lh = &line.header;
            buf.extend_from_slice(&lh.name_st_off.to_le_bytes());
            buf.extend_from_slice(&lh.texture_st_off.to_le_bytes());
            buf.extend_from_slice(&lh.texture_anchor_x.to_le_bytes());
            buf.extend_from_slice(&lh.texture_anchor_y.to_le_bytes());
            buf.extend_from_slice(&lh.z_order.to_le_bytes());
            buf.extend_from_slice(&lh.color_rgba);
            buf.extend_from_slice(&lh.parent_line_index.to_le_bytes());
            buf.extend_from_slice(&lh.inherit_flags.to_le_bytes());
            buf.extend_from_slice(&[0u8, 0, 0]); // reserved
            buf.extend_from_slice(&lh.bpm_lut_offset.to_le_bytes());
            buf.extend_from_slice(&lh.bpm_lut_entry_count.to_le_bytes());
            buf.extend_from_slice(&lh.motion_layer_count.to_le_bytes());
            buf.extend_from_slice(&lh.note_count.to_le_bytes());
        }

        // Write expressions
        buf.extend_from_slice(&(self.expressions.len() as u32).to_le_bytes());
        for expr in &self.expressions {
            buf.extend_from_slice(&(expr.bytecode.len() as u32).to_le_bytes());
            buf.extend_from_slice(&expr.bytecode);
        }

        buf
    }

    /// Deserialize from bytes (stub — returns empty file).
    pub fn from_bytes(_data: &[u8]) -> Result<Self, String> {
        // Full deserialization deferred to compiler integration
        Ok(Self::new())
    }
}

impl Default for FcbcFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_roundtrip() {
        let file = FcbcFile::new();
        let bytes = file.to_bytes();
        // Should have at least header + string table + constant pool + meta + timeline + line count
        assert!(
            bytes.len() >= 28 + 4 + 8 + 4 + 4,
            "got {} bytes",
            bytes.len()
        );
    }
}
