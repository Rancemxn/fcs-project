//! .fcbc file header (§9.2) — 28 bytes, little-endian.
pub const MAGIC: [u8; 4] = [0x46, 0x43, 0x53, 0x42];
pub const VERSION: u32 = 1;
pub const FLAG_HAS_SHADER: u32 = 1 << 0;
pub const FLAG_HAS_EXPRESSION: u32 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct FcbcHeader {
    pub magic: [u8; 4], pub version: u32, pub flags: u32,
    pub string_table_offset: u32, pub string_table_size: u32,
    pub constant_pool_offset: u32, pub constant_pool_size: u32,
}

impl FcbcHeader {
    pub fn new(st_off: u32, st_size: u32, cp_off: u32, cp_size: u32, flags: u32) -> Self {
        Self { magic: MAGIC, version: VERSION, flags, string_table_offset: st_off,
            string_table_size: st_size, constant_pool_offset: if cp_size > 0 { cp_off } else { 0 },
            constant_pool_size: cp_size }
    }
    pub const SIZE: usize = 28;
}
