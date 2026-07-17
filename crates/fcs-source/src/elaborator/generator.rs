//! Exact compile-time generator range evaluation.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::ast::{Beat, Document, Generator, GeneratorRangeValue, SourceSpan, Type, TypedValue};

use super::eval::evaluate_with_bindings;
use super::{CompileTimeLimits, ElaboratorError as Diagnostic};

/// A checked, typed generator range ready for body/emit expansion.
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratorRange {
    variable_type: Type,
    start: TypedValue,
    end: TypedValue,
    step: TypedValue,
    inclusive_end: bool,
    count: i64,
    span: SourceSpan,
}

impl GeneratorRange {
    /// Returns the generator variable type.
    pub fn variable_type(&self) -> &Type {
        &self.variable_type
    }

    /// Returns the exact evaluated start value.
    pub fn start(&self) -> &TypedValue {
        &self.start
    }

    /// Returns the exact evaluated end value.
    pub fn end(&self) -> &TypedValue {
        &self.end
    }

    /// Returns the exact evaluated step value.
    pub fn step(&self) -> &TypedValue {
        &self.step
    }

    /// Returns whether the source range uses an inclusive end operator.
    pub const fn inclusive_end(&self) -> bool {
        self.inclusive_end
    }

    /// Returns the exact number of reachable generator values.
    pub const fn count(&self) -> i64 {
        self.count
    }

    /// Returns the source span of the complete range production.
    pub const fn span(&self) -> SourceSpan {
        self.span
    }

    pub(super) fn frame_value(&self) -> TypedValue {
        TypedValue::GeneratorRange(Box::new(
            GeneratorRangeValue::new(
                self.start.clone(),
                self.end.clone(),
                self.step.clone(),
                self.count,
            )
            .expect("checked generator range metadata must be representable"),
        ))
    }

    /// Computes one range value from `start + index * step`.
    pub fn value_at(&self, index: i64) -> Result<TypedValue, GeneratorRangeError> {
        if index < 0 || index >= self.count {
            return Err(GeneratorRangeError::IndexOutOfBounds);
        }
        match (&self.start, &self.step) {
            (TypedValue::Int(start), TypedValue::Int(step)) => start
                .checked_add(
                    step.checked_mul(index)
                        .ok_or(GeneratorRangeError::NumericOverflow)?,
                )
                .map(TypedValue::Int)
                .ok_or(GeneratorRangeError::NumericOverflow),
            (TypedValue::Beat(start), TypedValue::Beat(step)) => step
                .checked_mul_i64(index)
                .and_then(|offset| start.checked_add(offset))
                .map(TypedValue::Beat)
                .map_err(|_| GeneratorRangeError::NumericOverflow),
            _ => Err(GeneratorRangeError::NumericOverflow),
        }
    }
}

/// Errors that can occur when a caller asks for one generated range value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorRangeError {
    /// The requested index is outside `[0, count)`.
    IndexOutOfBounds,
    /// Exact integer/rational arithmetic did not fit the source representation.
    NumericOverflow,
}

/// Evaluates one generator's range using the document's compile-time definitions.
pub fn evaluate_generator_range(
    document: &Document,
    generator: &Generator,
    limits: CompileTimeLimits,
) -> Result<GeneratorRange, Vec<crate::diagnostic::Diagnostic>> {
    if let Err(error) = super::preflight_names(document)
        .and_then(|()| super::resolve::check_document(document))
        .and_then(|()| {
            document
                .definitions
                .as_ref()
                .map_or(Ok(()), super::cycle::reject_cycles)
        })
        .and_then(|()| {
            document.definitions.as_ref().map_or(Ok(()), |definitions| {
                super::eval::check_and_evaluate(definitions, limits)
            })
        })
    {
        return Err(vec![error.into_diagnostic()]);
    }
    evaluate_range(document, generator, limits).map_err(|error| vec![error.into_diagnostic()])
}

pub(super) fn evaluate_range(
    document: &Document,
    generator: &Generator,
    limits: CompileTimeLimits,
) -> Result<GeneratorRange, Diagnostic> {
    let definitions = document.definitions.as_ref();
    let start = evaluate_with_bindings(
        &generator.range.start,
        definitions,
        &BTreeMap::new(),
        limits,
    )?;
    let end = evaluate_with_bindings(&generator.range.end, definitions, &BTreeMap::new(), limits)?;
    let step =
        evaluate_with_bindings(&generator.range.step, definitions, &BTreeMap::new(), limits)?;

    if !matches!(generator.variable_type, Type::Int | Type::Beat)
        || start.ty() != generator.variable_type
        || end.ty() != generator.variable_type
        || step.ty() != generator.variable_type
    {
        return Err(Diagnostic::InvalidGeneratorRange {
            span: generator.range.span,
            message: "generator range values must match an int or beat variable",
        });
    }

    if matches!(&step, TypedValue::Int(value) if *value == 0)
        || matches!(&step, TypedValue::Beat(value) if value.is_zero())
    {
        return Err(Diagnostic::ZeroGeneratorStep {
            span: generator.range.step.span(),
        });
    }

    let count = checked_count(
        &start,
        &end,
        &step,
        generator.range.inclusive_end,
        generator.range.span,
    )?;

    Ok(GeneratorRange {
        variable_type: generator.variable_type.clone(),
        start,
        end,
        step,
        inclusive_end: generator.range.inclusive_end,
        count,
        span: generator.range.span,
    })
}

fn checked_count(
    start: &TypedValue,
    end: &TypedValue,
    step: &TypedValue,
    inclusive_end: bool,
    span: SourceSpan,
) -> Result<i64, Diagnostic> {
    match (start, end, step) {
        (TypedValue::Int(start), TypedValue::Int(end), TypedValue::Int(step)) => {
            checked_integer_count(*start, *end, *step, inclusive_end, span)
        }
        (TypedValue::Beat(start), TypedValue::Beat(end), TypedValue::Beat(step)) => {
            checked_beat_count(*start, *end, *step, inclusive_end, span)
        }
        _ => Err(Diagnostic::InvalidGeneratorRange {
            span,
            message: "generator range values must have one exact numeric type",
        }),
    }
}

fn checked_integer_count(
    start: i64,
    end: i64,
    step: i64,
    inclusive_end: bool,
    span: SourceSpan,
) -> Result<i64, Diagnostic> {
    let direction = step.cmp(&0);
    let delta = match direction {
        Ordering::Greater if start > end || (!inclusive_end && start == end) => return Ok(0),
        Ordering::Less if start < end || (!inclusive_end && start == end) => return Ok(0),
        Ordering::Greater => i128::from(end) - i128::from(start),
        Ordering::Less => i128::from(start) - i128::from(end),
        Ordering::Equal => unreachable!("zero step is rejected before count calculation"),
    };
    let magnitude = i128::from(step).abs();
    let count = if inclusive_end {
        delta / magnitude + 1
    } else {
        (delta + magnitude - 1) / magnitude
    };
    i64::try_from(count).map_err(|_| Diagnostic::NumericOverflow { span })
}

fn checked_beat_count(
    start: Beat,
    end: Beat,
    step: Beat,
    inclusive_end: bool,
    span: SourceSpan,
) -> Result<i64, Diagnostic> {
    let ordering = step.cmp(&Beat::new(0, 1).expect("one beat denominator is valid"));
    let comparison = start.cmp(&end);
    match ordering {
        Ordering::Greater if comparison == Ordering::Greater => return Ok(0),
        Ordering::Less if comparison == Ordering::Less => return Ok(0),
        Ordering::Greater if comparison == Ordering::Equal && !inclusive_end => return Ok(0),
        Ordering::Less if comparison == Ordering::Equal && !inclusive_end => return Ok(0),
        _ => {}
    }
    let delta = match ordering {
        Ordering::Greater => end.checked_sub(start),
        Ordering::Less => start.checked_sub(end),
        Ordering::Equal => unreachable!("zero step is rejected before count calculation"),
    }
    .map_err(|_| Diagnostic::NumericOverflow { span })?;
    let ratio_numerator = (delta.numerator() as i128)
        .checked_mul(step.denominator() as i128)
        .ok_or(Diagnostic::NumericOverflow { span })?;
    let ratio_denominator = (delta.denominator() as i128)
        .checked_mul(step.numerator().unsigned_abs() as i128)
        .ok_or(Diagnostic::NumericOverflow { span })?;
    let count = if inclusive_end {
        ratio_numerator / ratio_denominator + 1
    } else {
        (ratio_numerator + ratio_denominator - 1) / ratio_denominator
    };
    i64::try_from(count).map_err(|_| Diagnostic::NumericOverflow { span })
}
