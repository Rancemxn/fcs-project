use std::collections::BTreeMap;
use std::fmt;

use crate::{
    CanonicalExpressionDag, CanonicalExpressionEnvironment, CanonicalExpressionType,
    CanonicalExpressionValue,
};

/// The exact query domain owned by one runtime descriptor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalDescriptorDomain {
    start: Option<f64>,
    end: Option<f64>,
    end_inclusive: bool,
}

impl CanonicalDescriptorDomain {
    pub fn new(
        start: Option<f64>,
        end: Option<f64>,
        end_inclusive: bool,
    ) -> Result<Self, CanonicalDescriptorError> {
        validate_domain(start, end, end_inclusive)?;
        Ok(Self {
            start,
            end,
            end_inclusive,
        })
    }

    pub const fn start(self) -> Option<f64> {
        self.start
    }

    pub const fn end(self) -> Option<f64> {
        self.end
    }

    pub const fn end_inclusive(self) -> bool {
        self.end_inclusive
    }

    pub fn contains(self, value: f64) -> bool {
        self.start.is_none_or(|start| value >= start)
            && self.end.is_none_or(|end| {
                value < end || (self.end_inclusive && value.to_bits() == end.to_bits())
            })
    }

    fn covers(self, other: PieceDomain) -> bool {
        let starts = match (self.start, other.start) {
            (None, _) => true,
            (Some(left), Some(right)) => left <= right,
            (Some(_), None) => false,
        };
        let ends = match (self.end, other.end) {
            (None, _) => true,
            (Some(left), Some(right)) => {
                left > right || (left == right && (!other.end_inclusive || self.end_inclusive))
            }
            (Some(_), None) => false,
        };
        starts && ends
    }
}

/// One exact Piecewise interval. `None` denotes the corresponding unbounded side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanonicalPiece {
    start: Option<f64>,
    end: Option<f64>,
    end_inclusive: bool,
    descriptor: usize,
}

impl CanonicalPiece {
    pub fn new(
        start: Option<f64>,
        end: Option<f64>,
        end_inclusive: bool,
        descriptor: usize,
    ) -> Result<Self, CanonicalDescriptorError> {
        validate_domain(start, end, end_inclusive)?;
        Ok(Self {
            start,
            end,
            end_inclusive,
            descriptor,
        })
    }

    pub const fn start(self) -> Option<f64> {
        self.start
    }

    pub const fn end(self) -> Option<f64> {
        self.end
    }

    pub const fn end_inclusive(self) -> bool {
        self.end_inclusive
    }

    pub const fn descriptor(self) -> usize {
        self.descriptor
    }

    fn domain(self) -> PieceDomain {
        PieceDomain {
            start: self.start,
            end: self.end,
            end_inclusive: self.end_inclusive,
        }
    }

    pub fn contains(self, value: f64) -> bool {
        self.start.is_none_or(|start| value >= start)
            && self.end.is_none_or(|end| {
                value < end || (self.end_inclusive && value.to_bits() == end.to_bits())
            })
    }
}

#[derive(Debug, Clone, Copy)]
struct PieceDomain {
    start: Option<f64>,
    end: Option<f64>,
    end_inclusive: bool,
}

/// The exact descriptor kinds available before FCBC serialization.
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalDescriptorKind {
    Constant(CanonicalExpressionValue),
    Expression(CanonicalExpressionDag),
    Piecewise(Vec<CanonicalPiece>),
}

/// A source-free, typed runtime descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalPropertyDescriptor {
    property_type: CanonicalExpressionType,
    domain: CanonicalDescriptorDomain,
    kind: CanonicalDescriptorKind,
}

impl CanonicalPropertyDescriptor {
    pub fn new(
        property_type: CanonicalExpressionType,
        domain: CanonicalDescriptorDomain,
        kind: CanonicalDescriptorKind,
    ) -> Result<Self, CanonicalDescriptorError> {
        if !property_type_is_valid(&property_type) {
            return Err(CanonicalDescriptorError::InvalidType);
        }
        if let CanonicalDescriptorKind::Constant(value) = &kind
            && (value.value_type() != property_type || !value.is_finite())
        {
            return Err(CanonicalDescriptorError::TypeMismatch);
        }
        if let CanonicalDescriptorKind::Expression(expression) = &kind
            && expression.result_type() != &property_type
        {
            return Err(CanonicalDescriptorError::TypeMismatch);
        }
        Ok(Self {
            property_type,
            domain,
            kind,
        })
    }

    pub fn property_type(&self) -> &CanonicalExpressionType {
        &self.property_type
    }

    pub const fn domain(&self) -> CanonicalDescriptorDomain {
        self.domain
    }

    pub fn kind(&self) -> &CanonicalDescriptorKind {
        &self.kind
    }
}

/// A direct canonical property root used as deterministic traversal input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalDescriptorRoot {
    target_path: String,
    owner: u64,
    descriptor: usize,
}

impl CanonicalDescriptorRoot {
    pub fn new(
        target_path: impl Into<String>,
        owner: u64,
        descriptor: usize,
    ) -> Result<Self, CanonicalDescriptorError> {
        let target_path = target_path.into();
        if target_path.is_empty() {
            return Err(CanonicalDescriptorError::EmptyTargetPath);
        }
        if owner == 0 {
            return Err(CanonicalDescriptorError::ZeroOwner);
        }
        Ok(Self {
            target_path,
            owner,
            descriptor,
        })
    }

    pub fn target_path(&self) -> &str {
        &self.target_path
    }

    pub const fn owner(&self) -> u64 {
        self.owner
    }

    pub const fn descriptor(&self) -> usize {
        self.descriptor
    }
}

/// Deterministically interned descriptor table.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalDescriptorTable {
    descriptors: Vec<CanonicalPropertyDescriptor>,
    roots: Vec<CanonicalDescriptorRoot>,
}

impl CanonicalDescriptorTable {
    pub fn new(
        descriptors: Vec<CanonicalPropertyDescriptor>,
        roots: Vec<CanonicalDescriptorRoot>,
    ) -> Result<Self, CanonicalDescriptorError> {
        if descriptors.is_empty() {
            return Err(CanonicalDescriptorError::EmptyTable);
        }
        if roots.is_empty() {
            return Err(CanonicalDescriptorError::NoRoots);
        }
        for root in &roots {
            if root.descriptor >= descriptors.len() {
                return Err(CanonicalDescriptorError::DanglingDescriptor {
                    descriptor: root.descriptor,
                });
            }
        }
        let mut root_names = BTreeMap::new();
        for root in &roots {
            if root_names
                .insert((root.target_path.clone(), root.owner), ())
                .is_some()
            {
                return Err(CanonicalDescriptorError::DuplicateRoot {
                    owner: root.owner,
                    target_path: root.target_path.clone(),
                });
            }
        }
        validate_descriptors(&descriptors, &roots)?;

        let keys = descriptors
            .iter()
            .enumerate()
            .map(|(index, _)| descriptor_key(index, &descriptors))
            .collect::<Result<Vec<_>, _>>()?;
        let mut roots = roots;
        roots.sort_by(|left, right| {
            left.target_path
                .as_bytes()
                .cmp(right.target_path.as_bytes())
                .then_with(|| left.owner.cmp(&right.owner))
                .then_with(|| keys[left.descriptor].cmp(&keys[right.descriptor]))
        });

        let mut mapped = vec![None; descriptors.len()];
        let mut interned = BTreeMap::<Vec<u8>, usize>::new();
        let mut canonical = Vec::new();
        for root in &mut roots {
            root.descriptor = emit_descriptor(
                root.descriptor,
                &descriptors,
                &keys,
                &mut mapped,
                &mut interned,
                &mut canonical,
            );
        }
        Ok(Self {
            descriptors: canonical,
            roots,
        })
    }

    pub fn descriptors(&self) -> &[CanonicalPropertyDescriptor] {
        &self.descriptors
    }

    pub fn roots(&self) -> &[CanonicalDescriptorRoot] {
        &self.roots
    }

    pub fn descriptor(&self, index: usize) -> Option<&CanonicalPropertyDescriptor> {
        self.descriptors.get(index)
    }
}

fn emit_descriptor(
    index: usize,
    descriptors: &[CanonicalPropertyDescriptor],
    keys: &[Vec<u8>],
    mapped: &mut [Option<usize>],
    interned: &mut BTreeMap<Vec<u8>, usize>,
    canonical: &mut Vec<CanonicalPropertyDescriptor>,
) -> usize {
    if let Some(index) = mapped[index] {
        return index;
    }
    let kind = match descriptors[index].kind() {
        CanonicalDescriptorKind::Piecewise(pieces) => {
            let mut pieces = pieces.clone();
            for piece in &mut pieces {
                piece.descriptor = emit_descriptor(
                    piece.descriptor,
                    descriptors,
                    keys,
                    mapped,
                    interned,
                    canonical,
                );
            }
            CanonicalDescriptorKind::Piecewise(pieces)
        }
        CanonicalDescriptorKind::Constant(value) => {
            CanonicalDescriptorKind::Constant(value.clone())
        }
        CanonicalDescriptorKind::Expression(expression) => {
            CanonicalDescriptorKind::Expression(expression.clone())
        }
    };
    let original_index = index;
    let canonical_index = if let Some(existing) = interned.get(&keys[original_index]) {
        *existing
    } else {
        let descriptor = CanonicalPropertyDescriptor {
            property_type: descriptors[index].property_type.clone(),
            domain: descriptors[index].domain,
            kind,
        };
        let index = canonical.len();
        canonical.push(descriptor);
        interned.insert(keys[original_index].clone(), index);
        index
    };
    mapped[original_index] = Some(canonical_index);
    canonical_index
}

fn validate_descriptors(
    descriptors: &[CanonicalPropertyDescriptor],
    roots: &[CanonicalDescriptorRoot],
) -> Result<(), CanonicalDescriptorError> {
    let mut marks = vec![Visit::Unvisited; descriptors.len()];
    let mut reachable = vec![false; descriptors.len()];
    for root in roots {
        visit_descriptor(root.descriptor, descriptors, &mut marks, &mut reachable)?;
    }
    if let Some(descriptor) = reachable.iter().position(|reachable| !reachable) {
        return Err(CanonicalDescriptorError::UnreachableDescriptor { descriptor });
    }
    let mut seen = vec![(usize::MAX, false); descriptors.len()];
    for root in roots {
        validate_environment(root.descriptor, false, descriptors, &mut seen)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Visit {
    Unvisited,
    Visiting,
    Done,
}

fn visit_descriptor(
    index: usize,
    descriptors: &[CanonicalPropertyDescriptor],
    marks: &mut [Visit],
    reachable: &mut [bool],
) -> Result<(), CanonicalDescriptorError> {
    if index >= descriptors.len() {
        return Err(CanonicalDescriptorError::DanglingDescriptor { descriptor: index });
    }
    if matches!(marks[index], Visit::Visiting) {
        return Err(CanonicalDescriptorError::DescriptorCycle { descriptor: index });
    }
    if matches!(marks[index], Visit::Done) {
        return Ok(());
    }
    marks[index] = Visit::Visiting;
    reachable[index] = true;
    if let CanonicalDescriptorKind::Piecewise(pieces) = descriptors[index].kind() {
        validate_partition(index, descriptors, pieces)?;
        for piece in pieces {
            visit_descriptor(piece.descriptor, descriptors, marks, reachable)?;
        }
    }
    marks[index] = Visit::Done;
    Ok(())
}

fn validate_partition(
    descriptor: usize,
    descriptors: &[CanonicalPropertyDescriptor],
    pieces: &[CanonicalPiece],
) -> Result<(), CanonicalDescriptorError> {
    if pieces.is_empty() {
        return Err(CanonicalDescriptorError::EmptyPiecewise { descriptor });
    }
    let domain = descriptors[descriptor].domain;
    let first = pieces[0];
    let last = pieces[pieces.len() - 1];
    if !same_endpoint(first.start, domain.start)
        || !same_endpoint(last.end, domain.end)
        || last.end_inclusive != domain.end_inclusive
        || first.start.is_none() != domain.start.is_none()
        || last.end.is_none() != domain.end.is_none()
    {
        return Err(CanonicalDescriptorError::PartitionBoundary { descriptor });
    }
    for (index, piece) in pieces.iter().copied().enumerate() {
        if piece.end_inclusive && index + 1 != pieces.len() {
            return Err(CanonicalDescriptorError::InvalidEndpointFlags {
                descriptor,
                piece: index,
            });
        }
        if piece.start.is_none() && index != 0 {
            return Err(CanonicalDescriptorError::InvalidEndpointFlags {
                descriptor,
                piece: index,
            });
        }
        if piece.end.is_none() && index + 1 != pieces.len() {
            return Err(CanonicalDescriptorError::InvalidEndpointFlags {
                descriptor,
                piece: index,
            });
        }
        if index > 0 && !same_endpoint(pieces[index - 1].end, piece.start) {
            return Err(CanonicalDescriptorError::PartitionGap {
                descriptor,
                piece: index,
            });
        }
        let child = descriptors.get(piece.descriptor).ok_or(
            CanonicalDescriptorError::DanglingDescriptor {
                descriptor: piece.descriptor,
            },
        )?;
        if child.property_type != descriptors[descriptor].property_type
            || !child.domain.covers(piece.domain())
        {
            return Err(CanonicalDescriptorError::PieceDomain {
                descriptor,
                piece: index,
            });
        }
    }
    Ok(())
}

fn validate_environment(
    index: usize,
    in_piece: bool,
    descriptors: &[CanonicalPropertyDescriptor],
    seen: &mut [(usize, bool)],
) -> Result<(), CanonicalDescriptorError> {
    if seen[index] == (index, in_piece) {
        return Ok(());
    }
    seen[index] = (index, in_piece);
    match descriptors[index].kind() {
        CanonicalDescriptorKind::Expression(expression) => {
            if !in_piece
                && expression
                    .required_environment()
                    .contains(&CanonicalExpressionEnvironment::P)
            {
                return Err(CanonicalDescriptorError::EnvPWithoutPiece { descriptor: index });
            }
        }
        CanonicalDescriptorKind::Piecewise(pieces) => {
            for piece in pieces {
                validate_environment(piece.descriptor, true, descriptors, seen)?;
            }
        }
        CanonicalDescriptorKind::Constant(_) => {}
    }
    Ok(())
}

fn descriptor_key(
    index: usize,
    descriptors: &[CanonicalPropertyDescriptor],
) -> Result<Vec<u8>, CanonicalDescriptorError> {
    fn visit(
        index: usize,
        descriptors: &[CanonicalPropertyDescriptor],
        marks: &mut [Visit],
        keys: &mut [Option<Vec<u8>>],
    ) -> Result<Vec<u8>, CanonicalDescriptorError> {
        if let Some(key) = &keys[index] {
            return Ok(key.clone());
        }
        if matches!(marks[index], Visit::Visiting) {
            return Err(CanonicalDescriptorError::DescriptorCycle { descriptor: index });
        }
        marks[index] = Visit::Visiting;
        let descriptor = &descriptors[index];
        let mut key = Vec::new();
        descriptor.property_type.append_structural_key(&mut key);
        append_domain_key(&mut key, descriptor.domain);
        match descriptor.kind() {
            CanonicalDescriptorKind::Constant(value) => {
                key.push(0);
                value.append_structural_key(&mut key);
            }
            CanonicalDescriptorKind::Expression(expression) => {
                key.push(1);
                expression.append_structural_key(&mut key);
            }
            CanonicalDescriptorKind::Piecewise(pieces) => {
                key.push(2);
                key.extend_from_slice(&(pieces.len() as u32).to_le_bytes());
                for piece in pieces {
                    append_piece_key(&mut key, *piece);
                    let child = visit(piece.descriptor, descriptors, marks, keys)?;
                    append_bytes(&mut key, &child);
                }
            }
        }
        marks[index] = Visit::Done;
        keys[index] = Some(key.clone());
        Ok(key)
    }

    let mut marks = vec![Visit::Unvisited; descriptors.len()];
    let mut keys = vec![None; descriptors.len()];
    visit(index, descriptors, &mut marks, &mut keys)
}

fn append_domain_key(output: &mut Vec<u8>, domain: CanonicalDescriptorDomain) {
    append_endpoint(output, domain.start);
    append_endpoint(output, domain.end);
    output.push(u8::from(domain.end_inclusive));
}

fn append_piece_key(output: &mut Vec<u8>, piece: CanonicalPiece) {
    append_endpoint(output, piece.start);
    append_endpoint(output, piece.end);
    output.push(u8::from(piece.end_inclusive));
}

fn append_endpoint(output: &mut Vec<u8>, endpoint: Option<f64>) {
    match endpoint {
        Some(value) => {
            output.push(1);
            output.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        None => output.push(0),
    }
}

fn append_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn validate_domain(
    start: Option<f64>,
    end: Option<f64>,
    end_inclusive: bool,
) -> Result<(), CanonicalDescriptorError> {
    if start.is_some_and(|value| !value.is_finite()) || end.is_some_and(|value| !value.is_finite())
    {
        return Err(CanonicalDescriptorError::NonFiniteEndpoint);
    }
    if end.is_none() && end_inclusive {
        return Err(CanonicalDescriptorError::InvalidDomainFlags);
    }
    if let (Some(start), Some(end)) = (start, end)
        && start >= end
    {
        return Err(CanonicalDescriptorError::InvalidInterval);
    }
    Ok(())
}

fn same_endpoint(left: Option<f64>, right: Option<f64>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.to_bits() == right.to_bits(),
        (None, None) => true,
        _ => false,
    }
}

fn property_type_is_valid(value: &CanonicalExpressionType) -> bool {
    match value {
        CanonicalExpressionType::Vec2(element) => element.is_numeric(),
        _ => true,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalDescriptorError {
    EmptyTable,
    NoRoots,
    EmptyTargetPath,
    ZeroOwner,
    EmptyPiecewise { descriptor: usize },
    InvalidType,
    TypeMismatch,
    NonFiniteEndpoint,
    InvalidInterval,
    InvalidDomainFlags,
    InvalidEndpointFlags { descriptor: usize, piece: usize },
    PartitionBoundary { descriptor: usize },
    PartitionGap { descriptor: usize, piece: usize },
    PieceDomain { descriptor: usize, piece: usize },
    DanglingDescriptor { descriptor: usize },
    DescriptorCycle { descriptor: usize },
    UnreachableDescriptor { descriptor: usize },
    DuplicateRoot { owner: u64, target_path: String },
    EnvPWithoutPiece { descriptor: usize },
}

impl fmt::Display for CanonicalDescriptorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTable => formatter.write_str("descriptor table must not be empty"),
            Self::NoRoots => formatter.write_str("descriptor table requires a direct root"),
            Self::EmptyTargetPath => {
                formatter.write_str("descriptor root target path must not be empty")
            }
            Self::ZeroOwner => formatter.write_str("descriptor root owner must be non-zero"),
            Self::EmptyPiecewise { descriptor } => {
                write!(formatter, "descriptor {descriptor} has no Piece entries")
            }
            Self::InvalidType => formatter.write_str("descriptor property type is invalid"),
            Self::TypeMismatch => {
                formatter.write_str("descriptor value type does not match its property type")
            }
            Self::NonFiniteEndpoint => {
                formatter.write_str("descriptor endpoint must be finite when present")
            }
            Self::InvalidInterval => {
                formatter.write_str("descriptor interval must have start before end")
            }
            Self::InvalidDomainFlags => {
                formatter.write_str("an unbounded descriptor end cannot be inclusive")
            }
            Self::InvalidEndpointFlags { descriptor, piece } => write!(
                formatter,
                "descriptor {descriptor} Piece {piece} has invalid endpoint flags"
            ),
            Self::PartitionBoundary { descriptor } => write!(
                formatter,
                "descriptor {descriptor} Piecewise partition does not match its domain"
            ),
            Self::PartitionGap { descriptor, piece } => write!(
                formatter,
                "descriptor {descriptor} Piece {piece} leaves a gap or overlaps the previous Piece"
            ),
            Self::PieceDomain { descriptor, piece } => write!(
                formatter,
                "descriptor {descriptor} Piece {piece} child domain is insufficient"
            ),
            Self::DanglingDescriptor { descriptor } => {
                write!(formatter, "descriptor reference {descriptor} is dangling")
            }
            Self::DescriptorCycle { descriptor } => {
                write!(formatter, "descriptor cycle reaches {descriptor}")
            }
            Self::UnreachableDescriptor { descriptor } => write!(
                formatter,
                "descriptor {descriptor} is unreachable from a direct root"
            ),
            Self::DuplicateRoot { owner, target_path } => write!(
                formatter,
                "owner {owner} has duplicate descriptor root {target_path}"
            ),
            Self::EnvPWithoutPiece { descriptor } => write!(
                formatter,
                "descriptor {descriptor} requires EnvP without Piece context"
            ),
        }
    }
}

impl std::error::Error for CanonicalDescriptorError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalExpressionNode, CanonicalExpressionOpcode};

    fn domain(start: Option<f64>, end: Option<f64>, inclusive: bool) -> CanonicalDescriptorDomain {
        CanonicalDescriptorDomain::new(start, end, inclusive).unwrap()
    }

    fn constant(value: f64, domain: CanonicalDescriptorDomain) -> CanonicalPropertyDescriptor {
        CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain,
            CanonicalDescriptorKind::Constant(CanonicalExpressionValue::Float(value)),
        )
        .unwrap()
    }

    fn root(path: &str, descriptor: usize) -> CanonicalDescriptorRoot {
        CanonicalDescriptorRoot::new(path, 1, descriptor).unwrap()
    }

    #[test]
    fn partition_rejects_gaps_and_cycles() {
        let child = constant(1.0, domain(Some(0.0), Some(10.0), true));
        let pieces = vec![
            CanonicalPiece::new(Some(0.0), Some(4.0), false, 0).unwrap(),
            CanonicalPiece::new(Some(5.0), Some(10.0), true, 0).unwrap(),
        ];
        let parent = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(10.0), true),
            CanonicalDescriptorKind::Piecewise(pieces),
        )
        .unwrap();
        assert!(matches!(
            CanonicalDescriptorTable::new(vec![child, parent], vec![root("line.alpha", 1)]),
            Err(CanonicalDescriptorError::PartitionGap {
                descriptor: 1,
                piece: 1
            })
        ));

        let cycle_piece = CanonicalPiece::new(Some(0.0), Some(1.0), true, 0).unwrap();
        let cycle = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(Some(0.0), Some(1.0), true),
            CanonicalDescriptorKind::Piecewise(vec![cycle_piece]),
        )
        .unwrap();
        assert!(matches!(
            CanonicalDescriptorTable::new(vec![cycle], vec![root("line.alpha", 0)]),
            Err(CanonicalDescriptorError::DescriptorCycle { descriptor: 0 })
        ));
    }

    #[test]
    fn structural_interning_and_root_order_are_stable() {
        let descriptor_a = constant(1.0, domain(None, None, false));
        let descriptor_b = constant(1.0, domain(None, None, false));
        let table = CanonicalDescriptorTable::new(
            vec![descriptor_a, descriptor_b],
            vec![root("note.alpha", 1), root("line.alpha", 0)],
        )
        .unwrap();
        assert_eq!(table.descriptors().len(), 1);
        assert_eq!(table.roots()[0].target_path(), "line.alpha");
        assert_eq!(table.roots()[1].target_path(), "note.alpha");
        assert_eq!(table.roots()[0].descriptor(), table.roots()[1].descriptor());

        let reversed = CanonicalDescriptorTable::new(
            vec![
                constant(1.0, domain(None, None, false)),
                constant(1.0, domain(None, None, false)),
            ],
            vec![root("line.alpha", 1), root("note.alpha", 0)],
        )
        .unwrap();
        assert_eq!(table, reversed);

        let signed_zero = CanonicalDescriptorTable::new(
            vec![
                constant(0.0, domain(None, None, false)),
                constant(-0.0, domain(None, None, false)),
            ],
            vec![root("line.alpha", 0), root("note.alpha", 1)],
        )
        .unwrap();
        assert_eq!(signed_zero.descriptors().len(), 2);
    }

    #[test]
    fn direct_env_p_requires_piece_context() {
        let expression = CanonicalExpressionDag::new(
            vec![CanonicalExpressionNode::new(
                CanonicalExpressionOpcode::EnvP,
                CanonicalExpressionType::Float,
                [None; 3],
                None,
                0,
            )],
            0,
        )
        .unwrap();
        let descriptor = CanonicalPropertyDescriptor::new(
            CanonicalExpressionType::Float,
            domain(None, None, false),
            CanonicalDescriptorKind::Expression(expression),
        )
        .unwrap();
        assert!(matches!(
            CanonicalDescriptorTable::new(vec![descriptor], vec![root("line.alpha", 0)]),
            Err(CanonicalDescriptorError::EnvPWithoutPiece { descriptor: 0 })
        ));
    }
}
