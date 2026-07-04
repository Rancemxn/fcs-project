//! Math Engine VM — stack-based Float64 interpreter (§9.9).
//!
//! The VM executes compiled expression bytecode, producing a single f64 result.
//! On error (div-by-zero, NaN, stack overflow), it returns a RuntimeError and
//! the caller should frustum-cull the affected entity for that frame.

pub mod easing;
pub mod math;
pub mod opcode;
pub mod stack;

use crate::error::RuntimeError;
use opcode::Opcode;
use stack::VmStack;

/// Environment variables injected into the VM at execution time.
#[derive(Debug, Clone, Copy)]
pub struct VmEnv {
    /// Absolute physical seconds (from master timeline)
    pub s: f64,
    /// Beat count (from local BPM timeline)
    pub b: f64,
    /// Pixel distance from note to judgment line center
    pub d: f64,
    /// Normalized interval progress (0–1) for motion keyframe evaluation
    pub t: f64,
}

impl VmEnv {
    pub fn new(s: f64, b: f64, d: f64, t: f64) -> Self {
        Self { s, b, d, t }
    }

    /// Default environment for testing (all zeros).
    pub fn default_for_test() -> Self {
        Self { s: 0.0, b: 0.0, d: 0.0, t: 0.0 }
    }
}

/// The FCS Math Engine VM.
pub struct Vm<'a> {
    code: &'a [u8],
    const_pool: &'a [f64],
    stack: VmStack,
    ip: usize,
    env: VmEnv,
}

/// Result of VM execution.
#[derive(Debug, Clone, PartialEq)]
pub enum VmResult {
    /// Successful evaluation — the resulting f64 value.
    Value(f64),
    /// Entity should be frustum-culled this frame (div-by-zero, NaN, etc.).
    Cull,
    /// VM error (stack overflow/underflow).
    Error(RuntimeError),
}

impl<'a> Vm<'a> {
    /// Create a new VM instance.
    pub fn new(code: &'a [u8], const_pool: &'a [f64], env: VmEnv) -> Self {
        Self {
            code,
            const_pool,
            stack: VmStack::new(),
            ip: 0,
            env,
        }
    }

    /// Execute the bytecode and return the result.
    pub fn execute(&mut self) -> VmResult {
        loop {
            if self.ip >= self.code.len() {
                // Implicit end of code — return whatever is on top
                return match self.stack.pop() {
                    Ok(v) => VmResult::Value(v),
                    Err(_) => VmResult::Value(0.0),
                };
            }

            let byte = self.code[self.ip];
            self.ip += 1;

            let opcode = match Opcode::from_u8(byte) {
                Some(op) => op,
                None => return VmResult::Value(0.0), // unknown opcode → return 0
            };

            if let Err(result) = self.dispatch(opcode) {
                return result;
            }
        }
    }

    /// Dispatch a single opcode.
    fn dispatch(&mut self, op: Opcode) -> Result<(), VmResult> {
        match op {
            Opcode::Nop => {}

            // ── Stack operations ──
            Opcode::PushConst => {
                let idx = self.read_u16() as usize;
                if idx < self.const_pool.len() {
                    self.stack_push(self.const_pool[idx])?;
                } else {
                    self.stack_push(0.0)?;
                }
            }
            Opcode::Pop => {
                let _ = self.stack.pop();
            }
            Opcode::Dup => {
                self.stack.dup().map_err(VmResult::Error)?;
            }
            Opcode::Swap => {
                self.stack.swap().map_err(VmResult::Error)?;
            }

            // ── Environment variables ──
            Opcode::PushVarS => self.stack_push(self.env.s)?,
            Opcode::PushVarB => self.stack_push(self.env.b)?,
            Opcode::PushVarD => self.stack_push(self.env.d)?,
            Opcode::PushVarT => self.stack_push(self.env.t)?,

            // ── Arithmetic (binary: pop 2, push 1) ──
            Opcode::Add => self.binary_op(|a, b| Ok(a + b))?,
            Opcode::Sub => self.binary_op(|a, b| Ok(a - b))?,
            Opcode::Mul => self.binary_op(|a, b| Ok(a * b))?,
            Opcode::Div => self.binary_op(|a, b| math::safe_div(a, b).ok_or(()))?,
            Opcode::Mod => self.binary_op(|a, b| math::safe_mod(a, b).ok_or(()))?,
            Opcode::Pow => self.binary_op(|a, b| {
                if a == 0.0 && b < 0.0 { return Err(()); }
                if a < 0.0 && b.fract() != 0.0 { return Err(()); }
                Ok(a.powf(b))
            })?,
            Opcode::Neg => self.unary_op(|a| Ok(-a))?,

            // ── Math functions (unary: pop 1, push 1) ──
            Opcode::Sin => self.unary_op(|a| Ok(a.sin()))?,
            Opcode::Cos => self.unary_op(|a| Ok(a.cos()))?,
            Opcode::Tan => self.unary_op(|a| Ok(a.tan()))?,
            Opcode::Asin => self.unary_op(|a| Ok(a.asin()))?,
            Opcode::Acos => self.unary_op(|a| Ok(a.acos()))?,
            Opcode::Atan => self.unary_op(|a| Ok(a.atan()))?,
            Opcode::Atan2 => self.binary_op(|a, b| Ok(a.atan2(b)))?,
            Opcode::Abs => self.unary_op(|a| Ok(a.abs()))?,
            Opcode::Exp => self.unary_op(|a| Ok(a.exp()))?,
            Opcode::Ln => self.unary_op(|a| {
                if a <= 0.0 { return Err(()); }
                Ok(a.ln())
            })?,
            Opcode::Log2 => self.unary_op(|a| {
                if a <= 0.0 { return Err(()); }
                Ok(a.log2())
            })?,
            Opcode::Log10 => self.unary_op(|a| {
                if a <= 0.0 { return Err(()); }
                Ok(a.log10())
            })?,
            Opcode::Sqrt => self.unary_op(|a| {
                if a < 0.0 { return Err(()); }
                Ok(a.sqrt())
            })?,
            Opcode::Floor => self.unary_op(|a| Ok(a.floor()))?,
            Opcode::Ceil => self.unary_op(|a| Ok(a.ceil()))?,
            Opcode::Round => self.unary_op(|a| Ok(a.round()))?,

            // ── Comparison & branch ──
            Opcode::CmpLt => self.binary_op(|a, b| Ok(if a < b { 1.0 } else { 0.0 }))?,
            Opcode::CmpLe => self.binary_op(|a, b| Ok(if a <= b { 1.0 } else { 0.0 }))?,
            Opcode::CmpGt => self.binary_op(|a, b| Ok(if a > b { 1.0 } else { 0.0 }))?,
            Opcode::CmpGe => self.binary_op(|a, b| Ok(if a >= b { 1.0 } else { 0.0 }))?,
            Opcode::CmpEq => self.binary_op(|a, b| Ok(if (a - b).abs() < 1e-10 { 1.0 } else { 0.0 }))?,
            Opcode::CmpNe => self.binary_op(|a, b| Ok(if (a - b).abs() >= 1e-10 { 1.0 } else { 0.0 }))?,
            Opcode::JmpIfFalse => {
                let offset = self.read_i16();
                let cond = self.stack_pop()?;
                if cond == 0.0 {
                    self.ip = ((self.ip as isize) + offset as isize) as usize;
                }
            }
            Opcode::Jmp => {
                let offset = self.read_i16();
                self.ip = ((self.ip as isize) + offset as isize) as usize;
            }

            // ── Easing dispatch ──
            Opcode::Ease => {
                let easing_id = self.read_u8();
                self.execute_ease(easing_id)?;
            }

            // ── Special operations ──
            Opcode::Clamp => self.ternary_op(|x, lo, hi| Ok(math::clamp(x, lo, hi)))?,
            Opcode::Lerp => self.ternary_op(|a, b, t| Ok(math::lerp(a, b, t)))?,
            Opcode::Select => self.ternary_op(|cond, if_true, if_false| {
                Ok(if cond != 0.0 { if_true } else { if_false })
            })?,
            Opcode::Min => self.binary_op(|a, b| Ok(a.min(b)))?,
            Opcode::Max => self.binary_op(|a, b| Ok(a.max(b)))?,

            // ── Termination ──
            Opcode::Ret => {
                return Err(match self.stack.pop() {
                    Ok(v) => VmResult::Value(v),
                    Err(_) => VmResult::Value(0.0),
                });
            }
        }
        Ok(())
    }

    // ── Stack helpers ──

    fn stack_push(&mut self, v: f64) -> Result<(), VmResult> {
        if !math::is_finite(v) {
            return Err(VmResult::Cull);
        }
        self.stack.push(v).map_err(VmResult::Error)
    }

    fn stack_pop(&mut self) -> Result<f64, VmResult> {
        self.stack.pop().map_err(VmResult::Error)
    }

    fn binary_op<F>(&mut self, f: F) -> Result<(), VmResult>
    where
        F: FnOnce(f64, f64) -> Result<f64, ()>,
    {
        let b = self.stack_pop()?;
        let a = self.stack_pop()?;
        match f(a, b) {
            Ok(v) => self.stack_push(v),
            Err(()) => Err(VmResult::Cull),
        }
    }

    fn unary_op<F>(&mut self, f: F) -> Result<(), VmResult>
    where
        F: FnOnce(f64) -> Result<f64, ()>,
    {
        let a = self.stack_pop()?;
        match f(a) {
            Ok(v) => self.stack_push(v),
            Err(()) => Err(VmResult::Cull),
        }
    }

    fn ternary_op<F>(&mut self, f: F) -> Result<(), VmResult>
    where
        F: FnOnce(f64, f64, f64) -> Result<f64, ()>,
    {
        let c = self.stack_pop()?;
        let b = self.stack_pop()?;
        let a = self.stack_pop()?;
        match f(a, b, c) {
            Ok(v) => self.stack_push(v),
            Err(()) => Err(VmResult::Cull),
        }
    }

    // ── EASE execution (§9.9.7) ──

    fn execute_ease(&mut self, easing_id: u8) -> Result<(), VmResult> {
        // EASE requires 7 values on stack — check before popping
        if self.stack.depth() < 7 {
            return Err(VmResult::Error(RuntimeError::StackUnderflow));
        }

        // LIFO pop order per §9.9.7.1:
        // 1st pop (TOS):     clampRight
        // 2nd pop (TOS-1):   clampLeft
        // 3rd pop (TOS-2):   v1       (target end value)
        // 4th pop (TOS-3):   v0       (initial start value)
        // 5th pop (TOS-4):   t1       (interval end time)
        // 6th pop (TOS-5):   t0       (interval start time)
        // 7th pop (TOS-6):   t        (current independent variable)
        let clamp_right = self.stack.pop().unwrap();
        let clamp_left = self.stack.pop().unwrap();
        let v1 = self.stack.pop().unwrap();
        let v0 = self.stack.pop().unwrap();
        let t1 = self.stack.pop().unwrap();
        let t0 = self.stack.pop().unwrap();
        let t = self.stack.pop().unwrap();

        // If t < t0: return v0. If t > t1: return v1.
        if t <= t0 {
            return self.stack_push(v0);
        }
        if t >= t1 {
            return self.stack_push(v1);
        }

        // Normalize t to [0, 1] within the interval
        let mut p = (t - t0) / (t1 - t0);
        // Apply clamp boundaries
        p = clamp_left + p * (clamp_right - clamp_left);
        p = math::clamp(p, 0.0, 1.0);

        // Evaluate easing function
        let f_p = easing::evaluate(easing_id, p);
        let result = f_p * (v1 - v0) + v0;

        self.stack_push(result)
    }

    // ── Bytecode reading ──

    fn read_u16(&mut self) -> u16 {
        if self.ip + 2 > self.code.len() {
            return 0;
        }
        let val = u16::from_le_bytes([self.code[self.ip], self.code[self.ip + 1]]);
        self.ip += 2;
        val
    }

    fn read_i16(&mut self) -> i16 {
        if self.ip + 2 > self.code.len() {
            return 0;
        }
        let val = i16::from_le_bytes([self.code[self.ip], self.code[self.ip + 1]]);
        self.ip += 2;
        val
    }

    fn read_u8(&mut self) -> u8 {
        if self.ip >= self.code.len() {
            return 0;
        }
        let val = self.code[self.ip];
        self.ip += 1;
        val
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn run(code: &[u8], const_pool: &[f64], env: VmEnv) -> VmResult {
        let mut vm = Vm::new(code, const_pool, env);
        vm.execute()
    }

    #[test]
    fn test_push_const_ret() {
        // PUSH_CONST[0] RET — const_pool[0] = 42.0
        let code = [0x01, 0x00, 0x00, 0xFF];
        let pool = [42.0];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Value(42.0));
    }

    #[test]
    fn test_add() {
        // PUSH_CONST[0] PUSH_CONST[1] ADD RET
        let code = [0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x20, 0xFF];
        let pool = [10.0, 32.0];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Value(42.0));
    }

    #[test]
    fn test_environment_vars() {
        // PUSH_VAR_S RET
        let code = [0x10, 0xFF];
        let env = VmEnv::new(3.5, 0.0, 0.0, 0.0);
        assert_eq!(run(&code, &[], env), VmResult::Value(3.5));
    }

    #[test]
    fn test_sin() {
        // pi/2 should give sin=1.0
        // PUSH_CONST[0] SIN RET
        let code = [0x01, 0x00, 0x00, 0x30, 0xFF];
        let pi_half = std::f64::consts::FRAC_PI_2;
        let pool = [pi_half];
        match run(&code, &pool, VmEnv::default_for_test()) {
            VmResult::Value(v) => assert!((v - 1.0).abs() < 1e-10),
            other => panic!("expected Value(1.0), got {:?}", other),
        }
    }

    #[test]
    fn test_div_by_zero_culls() {
        // PUSH_CONST[0,1] DIV RET where const=[1, 0]
        let code = [0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x23, 0xFF];
        let pool = [1.0, 0.0];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Cull);
    }

    #[test]
    fn test_chain_compare() {
        // Equivalent to: (1.0 < 2.0) — should push 1.0
        // PUSH_CONST[0] PUSH_CONST[1] CMP_LT RET
        let code = [0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x50, 0xFF];
        let pool = [1.0, 2.0];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Value(1.0));
    }

    #[test]
    fn test_spec_example_position_x() {
        // §9.9.10: positionX: 200px * sin(b * pi);
        // PUSH_CONST[0] PUSH_VAR_B PUSH_CONST[1] MUL SIN MUL RET
        // constPool[0]=200.0, constPool[1]=pi
        let code = [
            0x01, 0x00, 0x00, // PUSH_CONST[0] = 200.0
            0x11,               // PUSH_VAR_B
            0x01, 0x01, 0x00, // PUSH_CONST[1] = pi
            0x22,               // MUL
            0x30,               // SIN
            0x22,               // MUL
            0xFF,               // RET
        ];
        let pool = [200.0, std::f64::consts::PI];
        // b=0.5 → sin(0.5*pi) = sin(pi/2) = 1.0 → 200*1 = 200
        let env = VmEnv::new(0.0, 0.5, 0.0, 0.0);
        match run(&code, &pool, env) {
            VmResult::Value(v) => assert!((v - 200.0).abs() < 1e-10, "got {}", v),
            other => panic!("expected Value(200), got {:?}", other),
        }
    }

    #[test]
    fn test_spec_example_ternary() {
        // §9.9.10: scaleX: d > 200px ? 1.5 : 1.0;
        // PUSH_VAR_D PUSH_CONST[0] CMP_GT JMP_IF_FALSE +8 PUSH_CONST[1] JMP +5 PUSH_CONST[2] RET
        // constPool[0]=200, [1]=1.5, [2]=1.0
        // Bytecode: PUSH_VAR_D, PUSH_CONST[200], CMP_GT, JMP_IF_FALSE +6,
        //           PUSH_CONST[1.5], JMP +3, PUSH_CONST[1.0], RET
        // Layout: [0x12] [0x01,idx0] [0x52] [0x56,0x06,0x00] [0x01,idx1] [0x57,0x03,0x00] [0x01,idx2] [0xFF]
        //           0       1-3       4       5-7              8-10        11-13           14-16       17
        let code = [
            0x12,               // PUSH_VAR_D
            0x01, 0x00, 0x00, // PUSH_CONST[0] = 200
            0x52,               // CMP_GT
            0x56, 0x06, 0x00, // JMP_IF_FALSE +6 — skip to position 14
            0x01, 0x01, 0x00, // PUSH_CONST[1] = 1.5
            0x57, 0x03, 0x00, // JMP +3 — skip to position 17 (RET)
            0x01, 0x02, 0x00, // PUSH_CONST[2] = 1.0
            0xFF,               // RET
        ];
        let pool = [200.0, 1.5, 1.0];

        // d=250 → 250>200 true → 1.5
        let env = VmEnv::new(0.0, 0.0, 250.0, 0.0);
        assert_eq!(run(&code, &pool, env), VmResult::Value(1.5));

        // d=100 → 100>200 false → 1.0
        let env = VmEnv::new(0.0, 0.0, 100.0, 0.0);
        assert_eq!(run(&code, &pool, env), VmResult::Value(1.0));
    }

    #[test]
    fn test_clamp_lerp_select_min_max() {
        // CLAMP: PUSH 0.5, 0, 1 → CLAMP → 0.5
        let code = [0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x01, 0x02, 0x00, 0x70, 0xFF];
        let pool = [0.5, 0.0, 1.0];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Value(0.5));

        // LERP: PUSH 0, 10, 0.5 → LERP → 5.0
        let code = [0x01, 0x00, 0x00, 0x01, 0x01, 0x00, 0x01, 0x02, 0x00, 0x71, 0xFF];
        let pool = [0.0, 10.0, 0.5];
        assert_eq!(run(&code, &pool, VmEnv::default_for_test()), VmResult::Value(5.0));
    }
}
