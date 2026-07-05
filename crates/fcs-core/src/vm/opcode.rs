//! VM opcode definitions (§9.9.2–§9.9.9). 50+ instructions total.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    Nop = 0x00,
    PushConst = 0x01,
    Pop = 0x02,
    Dup = 0x03,
    Swap = 0x04,
    PushVarS = 0x10,
    PushVarB = 0x11,
    PushVarD = 0x12,
    PushVarT = 0x13,
    Add = 0x20,
    Sub = 0x21,
    Mul = 0x22,
    Div = 0x23,
    Mod = 0x24,
    Neg = 0x25,
    Pow = 0x26,
    Sin = 0x30,
    Cos = 0x31,
    Tan = 0x32,
    Asin = 0x33,
    Acos = 0x34,
    Atan = 0x35,
    Atan2 = 0x36,
    Abs = 0x37,
    Exp = 0x38,
    Ln = 0x39,
    Log2 = 0x3A,
    Log10 = 0x3B,
    Sqrt = 0x3C,
    Floor = 0x3D,
    Ceil = 0x3E,
    Round = 0x3F,
    CmpLt = 0x50,
    CmpLe = 0x51,
    CmpGt = 0x52,
    CmpGe = 0x53,
    CmpEq = 0x54,
    CmpNe = 0x55,
    JmpIfFalse = 0x56,
    Jmp = 0x57,
    Ease = 0x60,
    Clamp = 0x70,
    Lerp = 0x71,
    Select = 0x72,
    Min = 0x73,
    Max = 0x74,
    Ret = 0xFF,
}

impl Opcode {
    pub fn from_u8(byte: u8) -> Option<Self> {
        Some(match byte {
            0x00 => Self::Nop,
            0x01 => Self::PushConst,
            0x02 => Self::Pop,
            0x03 => Self::Dup,
            0x04 => Self::Swap,
            0x10 => Self::PushVarS,
            0x11 => Self::PushVarB,
            0x12 => Self::PushVarD,
            0x13 => Self::PushVarT,
            0x20 => Self::Add,
            0x21 => Self::Sub,
            0x22 => Self::Mul,
            0x23 => Self::Div,
            0x24 => Self::Mod,
            0x25 => Self::Neg,
            0x26 => Self::Pow,
            0x30 => Self::Sin,
            0x31 => Self::Cos,
            0x32 => Self::Tan,
            0x33 => Self::Asin,
            0x34 => Self::Acos,
            0x35 => Self::Atan,
            0x36 => Self::Atan2,
            0x37 => Self::Abs,
            0x38 => Self::Exp,
            0x39 => Self::Ln,
            0x3A => Self::Log2,
            0x3B => Self::Log10,
            0x3C => Self::Sqrt,
            0x3D => Self::Floor,
            0x3E => Self::Ceil,
            0x3F => Self::Round,
            0x50 => Self::CmpLt,
            0x51 => Self::CmpLe,
            0x52 => Self::CmpGt,
            0x53 => Self::CmpGe,
            0x54 => Self::CmpEq,
            0x55 => Self::CmpNe,
            0x56 => Self::JmpIfFalse,
            0x57 => Self::Jmp,
            0x60 => Self::Ease,
            0x70 => Self::Clamp,
            0x71 => Self::Lerp,
            0x72 => Self::Select,
            0x73 => Self::Min,
            0x74 => Self::Max,
            0xFF => Self::Ret,
            _ => return None,
        })
    }

    pub fn to_u8(self) -> u8 {
        self as u8
    }
    pub fn has_u16_operand(self) -> bool {
        matches!(self, Self::PushConst)
    }
    pub fn has_i16_operand(self) -> bool {
        matches!(self, Self::JmpIfFalse | Self::Jmp)
    }
    pub fn has_u8_operand(self) -> bool {
        matches!(self, Self::Ease)
    }
}
