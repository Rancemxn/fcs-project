//! Speed integral tiered processing (§5.5.3).
use crate::ast::Expression;
use crate::bytecode::constant_pool::ConstantPoolBuilder;
use crate::compiler::expr::compile_expression;

pub enum SpeedTier {
    Const(f32),
    Tier1 { velocity_bc: Vec<u8>, integral_bc: Vec<u8> },
    Tier2,
    Tier3 { velocity_bc: Vec<u8> },
}

pub fn classify_speed(expr: &Expression, consts: &mut ConstantPoolBuilder) -> SpeedTier {
    if let Expression::Literal(lit) = expr {
        match lit {
            crate::ast::Literal::Float(f) => return SpeedTier::Const(*f as f32),
            crate::ast::Literal::Integer(n) => return SpeedTier::Const(*n as f32),
            _ => {}
        }
    }
    let depends_runtime = expr_depends_on_runtime(expr);
    let velocity_bc = compile_expression(expr, consts);
    if depends_runtime { SpeedTier::Tier3 { velocity_bc } }
    else { SpeedTier::Tier2 } // MVP: always Tier2 for non-const
}

fn expr_depends_on_runtime(expr: &Expression) -> bool {
    match expr {
        Expression::Variable(name) => matches!(name.as_str(), "s" | "d"),
        Expression::BinaryOp { left, right, .. } => expr_depends_on_runtime(left) || expr_depends_on_runtime(right),
        Expression::UnaryOp { operand, .. } => expr_depends_on_runtime(operand),
        Expression::Call { args, .. } => args.iter().any(|a| expr_depends_on_runtime(a)),
        Expression::Ternary { cond, if_true, if_false } =>
            expr_depends_on_runtime(cond) || expr_depends_on_runtime(if_true) || expr_depends_on_runtime(if_false),
        Expression::ChainCompare { left, ops } =>
            expr_depends_on_runtime(left) || ops.iter().any(|(_, e)| expr_depends_on_runtime(e)),
        _ => false,
    }
}
