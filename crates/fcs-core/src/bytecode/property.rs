//! Property descriptor (§9.5) — 8-byte dual-path evaluation: Kind(u8)|Reserved(u24)|Payload(u32).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropertyDescriptor { pub kind: u8, pub payload: u32 }

impl PropertyDescriptor {
    pub fn new_const(value: f32) -> Self { Self { kind: 0, payload: value.to_bits() } }
    pub fn new_easing(byte_offset: u32) -> Self { Self { kind: 1, payload: byte_offset } }
    pub fn new_expr(byte_offset: u32) -> Self { Self { kind: 2, payload: byte_offset } }
    pub fn pack(&self) -> u64 { (self.kind as u64) | ((self.payload as u64) << 32) }
    pub fn unpack(raw: u64) -> Self { Self { kind: (raw & 0xFF) as u8, payload: (raw >> 32) as u32 } }
    pub fn as_f32(&self) -> f32 { f32::from_bits(self.payload) }
}

pub const TIER3_SENTINEL: u32 = 0xFFFFFFFF;
