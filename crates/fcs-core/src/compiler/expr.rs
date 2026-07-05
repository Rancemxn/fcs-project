//! Expression AST → VM bytecode compilation.
use crate::ast::{BinaryOp, CompareOp, Expression, Literal, UnaryOp};
use crate::bytecode::constant_pool::ConstantPoolBuilder;
use crate::vm::opcode::Opcode;

pub fn compile_expression(expr: &Expression, consts: &mut ConstantPoolBuilder) -> Vec<u8> {
    let mut code = Vec::new();
    compile_expr(expr, consts, &mut code);
    code.push(Opcode::Ret as u8);
    code
}

fn compile_expr(expr: &Expression, consts: &mut ConstantPoolBuilder, code: &mut Vec<u8>) {
    match expr {
        Expression::Literal(lit) => match lit {
            Literal::Integer(n) => push_const(*n as f64, consts, code),
            Literal::Float(f) => push_const(*f, consts, code),
            Literal::Quantified { value, .. } => push_const(*value, consts, code),
            Literal::Boolean(b) => push_const(if *b { 1.0 } else { 0.0 }, consts, code),
            _ => push_const(0.0, consts, code),
        },
        Expression::Variable(name) => {
            code.push(match name.as_str() {
                "s" => Opcode::PushVarS,
                "b" => Opcode::PushVarB,
                "d" => Opcode::PushVarD,
                _ => Opcode::PushVarB,
            } as u8);
        }
        Expression::BinaryOp { op, left, right } => {
            compile_expr(left, consts, code);
            compile_expr(right, consts, code);
            code.push(match op {
                BinaryOp::Add => Opcode::Add,
                BinaryOp::Sub => Opcode::Sub,
                BinaryOp::Mul => Opcode::Mul,
                BinaryOp::Div => Opcode::Div,
                BinaryOp::Mod => Opcode::Mod,
                BinaryOp::Pow => Opcode::Pow,
            } as u8);
        }
        Expression::UnaryOp {
            op: UnaryOp::Neg,
            operand,
        } => {
            compile_expr(operand, consts, code);
            code.push(Opcode::Neg as u8);
        }
        Expression::Call { name, args } => compile_call(name, args, consts, code),
        Expression::Ternary {
            cond,
            if_true,
            if_false,
        } => {
            compile_expr(cond, consts, code);
            let jmp_false_pos = code.len();
            code.push(Opcode::JmpIfFalse as u8);
            code.extend_from_slice(&[0u8; 2]);
            compile_expr(if_true, consts, code);
            let jmp_end_pos = code.len();
            code.push(Opcode::Jmp as u8);
            code.extend_from_slice(&[0u8; 2]);
            let ft = code.len();
            let off = (ft as i32 - jmp_false_pos as i32 - 3) as i16;
            code[jmp_false_pos + 1..jmp_false_pos + 3].copy_from_slice(&off.to_le_bytes());
            compile_expr(if_false, consts, code);
            let et = code.len();
            let off2 = (et as i32 - jmp_end_pos as i32 - 3) as i16;
            code[jmp_end_pos + 1..jmp_end_pos + 3].copy_from_slice(&off2.to_le_bytes());
        }
        Expression::ChainCompare { left, ops } => {
            let mut prev = left.as_ref().clone();
            let mut first = true;
            for (cmp, right) in ops {
                if !first {
                    code.push(Opcode::Swap as u8);
                }
                compile_expr(&prev, consts, code);
                compile_expr(right, consts, code);
                code.push(match cmp {
                    CompareOp::Lt => Opcode::CmpLt,
                    CompareOp::Le => Opcode::CmpLe,
                    CompareOp::Gt => Opcode::CmpGt,
                    CompareOp::Ge => Opcode::CmpGe,
                    CompareOp::Eq => Opcode::CmpEq,
                    CompareOp::Ne => Opcode::CmpNe,
                } as u8);
                if !first {
                    code.push(Opcode::Mul as u8);
                }
                prev = right.as_ref().clone();
                first = false;
            }
        }
    }
}

fn push_const(v: f64, consts: &mut ConstantPoolBuilder, code: &mut Vec<u8>) {
    code.push(Opcode::PushConst as u8);
    let idx = consts.intern(v);
    code.extend_from_slice(&idx.to_le_bytes());
}

fn compile_call(
    name: &str,
    args: &[Expression],
    consts: &mut ConstantPoolBuilder,
    code: &mut Vec<u8>,
) {
    if let Some(op) = builtin_opcode(name) {
        for a in args {
            compile_expr(a, consts, code);
        }
        code.push(op as u8);
    } else if is_easing(name) {
        for a in args {
            compile_expr(a, consts, code);
        }
        code.push(Opcode::Ease as u8);
        code.push(easing_id(name));
    } else {
        push_const(0.0, consts, code);
    }
}

fn builtin_opcode(name: &str) -> Option<Opcode> {
    Some(match name {
        "sin" => Opcode::Sin,
        "cos" => Opcode::Cos,
        "tan" => Opcode::Tan,
        "asin" => Opcode::Asin,
        "acos" => Opcode::Acos,
        "atan" => Opcode::Atan,
        "atan2" => Opcode::Atan2,
        "abs" => Opcode::Abs,
        "exp" => Opcode::Exp,
        "ln" => Opcode::Ln,
        "log2" => Opcode::Log2,
        "log10" => Opcode::Log10,
        "sqrt" => Opcode::Sqrt,
        "floor" => Opcode::Floor,
        "ceil" => Opcode::Ceil,
        "round" => Opcode::Round,
        "clamp" => Opcode::Clamp,
        "lerp" => Opcode::Lerp,
        "min" => Opcode::Min,
        "max" => Opcode::Max,
        _ => return None,
    })
}

fn is_easing(s: &str) -> bool {
    s.starts_with("ease")
}

fn easing_id(name: &str) -> u8 {
    match name {
        "easeLinear" => 1,
        "easeOutSine" => 2,
        "easeInSine" => 3,
        "easeOutQuad" => 4,
        "easeInQuad" => 5,
        "easeInOutSine" => 6,
        "easeInOutQuad" => 7,
        "easeOutCubic" => 8,
        "easeInCubic" => 9,
        "easeOutQuart" => 10,
        "easeInQuart" => 11,
        "easeInOutCubic" => 12,
        "easeInOutQuart" => 13,
        "easeOutQuint" => 14,
        "easeInQuint" => 15,
        "easeOutExpo" => 16,
        "easeInExpo" => 17,
        "easeOutCirc" => 18,
        "easeInCirc" => 19,
        "easeOutBack" => 20,
        "easeInBack" => 21,
        "easeInOutCirc" => 22,
        "easeInOutBack" => 23,
        "easeOutElastic" => 24,
        "easeInElastic" => 25,
        "easeOutBounce" => 26,
        "easeInBounce" => 27,
        "easeInOutBounce" => 28,
        "easeInOutElastic" => 29,
        "easeBezier" => 30,
        _ => 0,
    }
}
