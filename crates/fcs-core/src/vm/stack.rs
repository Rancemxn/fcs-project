//! VM stack — fixed [f64; 256] with overflow/underflow checks (§9.9.1).

use crate::error::RuntimeError;

pub const MAX_STACK_DEPTH: usize = 256;

#[derive(Debug, Clone)]
pub struct VmStack { data: [f64; MAX_STACK_DEPTH], sp: usize }

impl VmStack {
    pub fn new() -> Self { Self { data: [0.0; MAX_STACK_DEPTH], sp: 0 } }

    pub fn push(&mut self, value: f64) -> Result<(), RuntimeError> {
        if self.sp >= MAX_STACK_DEPTH { return Err(RuntimeError::StackOverflow); }
        self.data[self.sp] = value;
        self.sp += 1;
        Ok(())
    }

    pub fn pop(&mut self) -> Result<f64, RuntimeError> {
        if self.sp == 0 { return Err(RuntimeError::StackUnderflow); }
        self.sp -= 1;
        Ok(self.data[self.sp])
    }

    pub fn peek(&self) -> Result<f64, RuntimeError> {
        if self.sp == 0 { return Err(RuntimeError::StackUnderflow); }
        Ok(self.data[self.sp - 1])
    }

    pub fn dup(&mut self) -> Result<(), RuntimeError> { let v = self.peek()?; self.push(v) }

    pub fn swap(&mut self) -> Result<(), RuntimeError> {
        if self.sp < 2 { return Err(RuntimeError::StackUnderflow); }
        self.data.swap(self.sp - 1, self.sp - 2);
        Ok(())
    }

    pub fn depth(&self) -> usize { self.sp }
    pub fn has(&self, n: usize) -> bool { self.sp >= n }
    pub fn clear(&mut self) { self.sp = 0; }
}

impl Default for VmStack { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_pop() {
        let mut s = VmStack::new();
        s.push(42.0).unwrap();
        assert_eq!(s.pop().unwrap(), 42.0);
    }

    #[test]
    fn test_underflow() { assert!(VmStack::new().pop().is_err()); }

    #[test]
    fn test_overflow() {
        let mut s = VmStack::new();
        for _ in 0..256 { s.push(1.0).unwrap(); }
        assert!(s.push(1.0).is_err());
    }

    #[test]
    fn test_swap() {
        let mut s = VmStack::new();
        s.push(1.0).unwrap(); s.push(2.0).unwrap();
        s.swap().unwrap();
        assert_eq!(s.pop().unwrap(), 1.0);
        assert_eq!(s.pop().unwrap(), 2.0);
    }
}
