use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const NULL_INDEX: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ValueType {
    Bool = 1,
    Int = 2,
    Float = 3,
    Time = 4,
    Beat = 5,
    Length = 6,
    Angle = 7,
    Color = 8,
    Vec2Float = 9,
    Vec2Length = 10,
    Vec2Int = 11,
    Vec2Time = 12,
    Vec2Beat = 13,
    Vec2Angle = 14,
}

impl ValueType {
    fn from_abi(value: u8) -> Result<Self, &'static str> {
        match value {
            1 => Ok(Self::Bool),
            2 => Ok(Self::Int),
            3 => Ok(Self::Float),
            4 => Ok(Self::Time),
            5 => Ok(Self::Beat),
            6 => Ok(Self::Length),
            7 => Ok(Self::Angle),
            8 => Ok(Self::Color),
            9 => Ok(Self::Vec2Float),
            10 => Ok(Self::Vec2Length),
            11 => Ok(Self::Vec2Int),
            12 => Ok(Self::Vec2Time),
            13 => Ok(Self::Vec2Beat),
            14 => Ok(Self::Vec2Angle),
            _ => Err("fcbc.invalid-track"),
        }
    }

    pub fn is_vector(self) -> bool {
        matches!(
            self,
            Self::Vec2Float
                | Self::Vec2Length
                | Self::Vec2Int
                | Self::Vec2Time
                | Self::Vec2Beat
                | Self::Vec2Angle
        )
    }

    pub fn vector_element(self) -> Option<Self> {
        match self {
            Self::Vec2Float => Some(Self::Float),
            Self::Vec2Length => Some(Self::Length),
            Self::Vec2Int => Some(Self::Int),
            Self::Vec2Time => Some(Self::Time),
            Self::Vec2Beat => Some(Self::Beat),
            Self::Vec2Angle => Some(Self::Angle),
            _ => None,
        }
    }

    fn vector_of(element: Self) -> Option<Self> {
        match element {
            Self::Float => Some(Self::Vec2Float),
            Self::Length => Some(Self::Vec2Length),
            Self::Int => Some(Self::Vec2Int),
            Self::Time => Some(Self::Vec2Time),
            Self::Beat => Some(Self::Vec2Beat),
            Self::Angle => Some(Self::Vec2Angle),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Bool(bool),
    Int(i64),
    Scalar { ty: ValueType, value: f64 },
    Color([f64; 4]),
    Vec2 { ty: ValueType, value: [f64; 2] },
    ResourceRef(u64),
    ContributorRef(u64),
}

impl RuntimeValue {
    pub fn value_type(&self) -> ValueType {
        match self {
            Self::Bool(_) => ValueType::Bool,
            Self::Int(_) => ValueType::Int,
            Self::Scalar { ty, .. } => *ty,
            Self::Color(_) => ValueType::Color,
            Self::Vec2 { ty, .. } => *ty,
            Self::ResourceRef(_) | Self::ContributorRef(_) => {
                unreachable!("entity references are not expression ABI values")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    pub start: f64,
    pub end: f64,
    pub unbounded_before: bool,
    pub unbounded_after: bool,
}

impl Domain {
    pub fn contains(self, value: f64) -> bool {
        value.is_finite()
            && (self.unbounded_before || value >= self.start)
            && (self.unbounded_after || value <= self.end)
    }

    pub fn covers(self, other: Self) -> bool {
        (self.unbounded_before || (!other.unbounded_before && self.start <= other.start))
            && (self.unbounded_after || (!other.unbounded_after && self.end >= other.end))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub interpolation: u16,
    pub easing: u16,
    pub flags: u32,
    pub start_constant: u32,
    pub end_constant: u32,
    pub bezier: [f64; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Piece {
    pub start: f64,
    pub end: f64,
    pub descriptor_index: u32,
    pub flags: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DescriptorKind {
    Constant(u32),
    SegmentTrack(Vec<Segment>),
    Piecewise(Vec<Piece>),
    Expression(u32),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDescriptor {
    pub property_type: ValueType,
    pub domain: Domain,
    pub kind: DescriptorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionNode {
    pub opcode: u16,
    pub result_type: ValueType,
    pub operands: [u32; 3],
    pub arity: u8,
    pub immediate: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DistanceClassification {
    PortableAnalytic,
    PortableEvaluable,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DistanceDescriptor {
    pub line_id: u64,
    pub scroll_speed_descriptor: u32,
    pub domain: Domain,
    pub integration_origin: f64,
    pub initial_floor_position: f64,
    pub max_velocity_error: f64,
    pub max_distance_error: f64,
    pub boundaries: Vec<f64>,
    pub classification: DistanceClassification,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TempoPoint {
    pub beat_numerator: i64,
    pub beat_denominator: i64,
    pub chart_time: f64,
    pub bpm: f64,
    pub source_order: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineRecord {
    pub id: u64,
    pub parent_id: u64,
    pub document_order: u32,
    pub line_flags: u32,
    pub position_descriptor: u32,
    pub rotation_descriptor: u32,
    pub scale_descriptor: u32,
    pub alpha_descriptor: u32,
    pub transform_origin_constant: u32,
    pub texture_anchor_constant: u32,
    pub scroll_tempo_descriptor: u32,
    pub scroll_speed_descriptor: u32,
    pub distance_descriptor: u32,
    pub floor_scale: f64,
    pub integration_origin: f64,
    pub initial_floor_position: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct NoteRecord {
    pub id: u64,
    pub line_id: u64,
    pub document_order: u32,
    pub kind: u8,
    pub side: u8,
    pub flags: u16,
    pub time: f64,
    pub end_time: f64,
    pub property_descriptors: [u32; 10],
    pub sound_resource_id: u64,
    pub texture_resource_id: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionInfo {
    pub section_type: u32,
    pub offset: u64,
    pub length: u64,
    pub checksum: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodedChart {
    pub container_profile: u8,
    pub document_profile: u8,
    pub feature_flags: u64,
    pub strings: Vec<String>,
    pub constants: Vec<RuntimeValue>,
    pub tempo_points: Vec<TempoPoint>,
    pub lines: Vec<LineRecord>,
    pub notes: Vec<NoteRecord>,
    pub descriptors: Vec<PropertyDescriptor>,
    pub expressions: Vec<ExpressionNode>,
    pub distances: Vec<DistanceDescriptor>,
    pub sections: Vec<SectionInfo>,
}

#[derive(Clone)]
struct RawSection {
    info: SectionInfo,
    flags: u16,
    version: (u16, u16, u16),
    alignment_log2: u8,
}

#[derive(Clone, Debug)]
struct ParsedValue {
    tag: u8,
    string_ref: Option<u32>,
    fields: Vec<(u32, ParsedValue)>,
}

#[derive(Clone)]
struct ResourceRecord {
    id: u64,
    kind: u16,
    data_offset: u64,
    data_length: u64,
    hash: Vec<u8>,
}

type ParsedContainer = (u8, u64, [Option<u32>; 2], Vec<RawSection>);

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    error: &'static str,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8], error: &'static str) -> Self {
        Self {
            bytes,
            position: 0,
            error,
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.position)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], &'static str> {
        let end = self.position.checked_add(length).ok_or(self.error)?;
        let result = self.bytes.get(self.position..end).ok_or(self.error)?;
        self.position = end;
        Ok(result)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| self.error)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.error)?,
        ))
    }

    fn i32(&mut self) -> Result<i32, &'static str> {
        Ok(i32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| self.error)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| self.error)?,
        ))
    }

    fn i64(&mut self) -> Result<i64, &'static str> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| self.error)?,
        ))
    }

    fn f64(&mut self) -> Result<f64, &'static str> {
        let value = f64::from_bits(self.u64()?);
        if value.is_finite() {
            Ok(value)
        } else {
            Err(self.error)
        }
    }

    fn zeroes(&mut self, length: usize) -> Result<(), &'static str> {
        if self.take(length)?.iter().all(|byte| *byte == 0) {
            Ok(())
        } else {
            Err(self.error)
        }
    }

    fn finish(self) -> Result<(), &'static str> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(self.error)
        }
    }
}

/// Independently decodes and validates an FCBC 2 / Execution ABI 1 byte sequence.
///
/// The cursor and all layout checks in this module are intentionally separate from the reference
/// writer. Failures return the nearest stable `fcbc.*` parent diagnostic.
pub fn load(bytes: &[u8]) -> Result<DecodedChart, &'static str> {
    let (container_profile, feature_flags, compiler_refs, sections) = parse_container(bytes)?;
    let section_map: BTreeMap<u32, &RawSection> = sections
        .iter()
        .map(|section| (section.info.section_type, section))
        .collect();

    let strings = parse_string_table(section_payload(bytes, &section_map, 1)?)?;
    for compiler_ref in compiler_refs.into_iter().flatten() {
        if compiler_ref as usize >= strings.len() {
            return Err("fcbc.dangling-reference");
        }
    }
    let constants = parse_constant_pool(section_payload(bytes, &section_map, 2)?)?;
    let document_profile = parse_meta(section_payload(bytes, &section_map, 3)?, &strings)?;
    let contributor_ids = parse_contributors(section_payload(bytes, &section_map, 4)?, &strings)?;
    parse_credits(
        section_payload(bytes, &section_map, 5)?,
        &strings,
        &contributor_ids,
    )?;
    let resources = parse_resources(section_payload(bytes, &section_map, 6)?, &strings)?;
    validate_resource_data(section_payload(bytes, &section_map, 20)?, &resources)?;
    parse_sync(section_payload(bytes, &section_map, 7)?, &resources)?;
    let tempo_points = parse_tempo(section_payload(bytes, &section_map, 8)?)?;
    let lines = parse_lines(section_payload(bytes, &section_map, 9)?, &strings)?;
    let notes = parse_notes(section_payload(bytes, &section_map, 10)?, &strings)?;
    let descriptors = parse_tracks(section_payload(bytes, &section_map, 11)?)?;
    let expressions = parse_expressions(section_payload(bytes, &section_map, 12)?)?;
    let distances = parse_distances(section_payload(bytes, &section_map, 13)?)?;

    validate_expression_signatures(&expressions, &constants)?;
    validate_descriptors(&descriptors, &constants, &expressions)?;
    validate_lines(&lines, &descriptors, &constants, &distances)?;
    validate_notes(&notes, &lines, &descriptors, &resources)?;
    validate_distances(&distances, &lines, &descriptors, &constants)?;
    validate_canonical_reachability(&descriptors, &expressions, &constants, &lines, &notes)?;

    Ok(DecodedChart {
        container_profile,
        document_profile,
        feature_flags,
        strings,
        constants,
        tempo_points,
        lines,
        notes,
        descriptors,
        expressions,
        distances,
        sections: sections.into_iter().map(|section| section.info).collect(),
    })
}

fn parse_container(bytes: &[u8]) -> Result<ParsedContainer, &'static str> {
    if bytes.len() < 128 {
        return Err("fcbc.invalid-header");
    }
    if bytes.get(..4) != Some(b"FCSB") {
        return Err("fcbc.bad-magic");
    }

    let mut header = Cursor::new(&bytes[4..128], "fcbc.invalid-header");
    if header.u16()? != 128 || header.u16()? != 0 {
        return Err("fcbc.invalid-header");
    }
    let source_version = (header.u16()?, header.u16()?, header.u16()?);
    if source_version.0 != 5 {
        return Err("fcbc.unsupported-source-version");
    }
    let fcbc_version = (header.u16()?, header.u16()?, header.u16()?);
    if fcbc_version.0 != 2 {
        return Err("fcbc.unsupported-container-version");
    }
    let abi_version = (header.u16()?, header.u16()?, header.u16()?);
    if abi_version.0 != 1 {
        return Err("fcbc.unsupported-abi-version");
    }
    let profile = header.u8()?;
    if profile != 3 {
        return Err("fcbc.unsupported-profile");
    }
    if header.u8()? != 1 {
        return Err("fcbc.invalid-header");
    }
    let feature_flags = header.u64()?;
    if feature_flags & !0x17f != 0 || feature_flags & (1 << 7) != 0 {
        return Err("fcbc.invalid-header");
    }
    let section_count = header.u32()? as usize;
    if !(14..=1024).contains(&section_count) {
        return Err("fcbc.limit-exceeded");
    }
    let section_table_offset = header.u64()?;
    let declared_file_length = header.u64()?;
    if declared_file_length != bytes.len() as u64 {
        return Err("fcbc.file-length-mismatch");
    }
    let source_hash = header.take(32)?;
    let source_hash_present = feature_flags & 1 != 0;
    if !source_hash_present && source_hash.iter().any(|byte| *byte != 0) {
        return Err("fcbc.invalid-header");
    }
    let compiler_id = optional_index(header.u32()?);
    let compiler_version = optional_index(header.u32()?);
    header.zeroes(32)?;
    header.finish()?;

    let table_length = section_count
        .checked_mul(40)
        .ok_or("fcbc.section-table-bounds")?;
    let table_start =
        usize::try_from(section_table_offset).map_err(|_| "fcbc.section-table-bounds")?;
    let table_end = table_start
        .checked_add(table_length)
        .ok_or("fcbc.section-table-bounds")?;
    if table_start < 128 || table_end > bytes.len() {
        return Err("fcbc.section-table-bounds");
    }

    let mut table = Cursor::new(&bytes[table_start..table_end], "fcbc.section-table-bounds");
    let mut sections = Vec::with_capacity(section_count);
    let mut prior_key = None;
    let mut seen_types = BTreeSet::new();
    for _ in 0..section_count {
        let section_type = table.u32()?;
        let version = (table.u16()?, table.u16()?, table.u16()?);
        let flags = table.u16()?;
        let alignment_log2 = table.u8()?;
        table.zeroes(3)?;
        let offset = table.u64()?;
        let length = table.u64()?;
        let checksum = table.u32()?;
        table.zeroes(4)?;

        if flags & !0b11 != 0 || alignment_log2 > 20 {
            return Err("fcbc.invalid-header");
        }
        if (1..=20).contains(&section_type) && alignment_log2 != 3 {
            return Err("fcbc.section-alignment");
        }
        let alignment = 1u64 << alignment_log2;
        if offset % alignment != 0 {
            return Err("fcbc.section-alignment");
        }
        let key = (section_type, offset);
        if prior_key.is_some_and(|prior| prior >= key) {
            return Err("fcbc.section-order");
        }
        prior_key = Some(key);
        if (1..=20).contains(&section_type) && !seen_types.insert(section_type) {
            return Err("fcbc.invalid-record");
        }
        if (1..=20).contains(&section_type) && version.0 != 1 && flags & 1 != 0 {
            return Err("fcbc.unknown-required-section");
        }

        let end = offset
            .checked_add(length)
            .ok_or("fcbc.section-table-bounds")?;
        if offset < table_end as u64 || end > bytes.len() as u64 {
            return Err("fcbc.section-table-bounds");
        }
        sections.push(RawSection {
            info: SectionInfo {
                section_type,
                offset,
                length,
                checksum,
            },
            flags,
            version,
            alignment_log2,
        });
    }
    table.finish()?;

    let mut layout_cursor = table_end as u64;
    for section in &sections {
        let alignment = 1u64 << section.alignment_log2;
        let expected_offset =
            align_up_u64(layout_cursor, alignment).ok_or("fcbc.section-table-bounds")?;
        if section.info.offset != expected_offset {
            return Err(if section.info.offset < expected_offset {
                "fcbc.section-overlap"
            } else {
                "fcbc.section-order"
            });
        }
        let padding_start =
            usize::try_from(layout_cursor).map_err(|_| "fcbc.section-table-bounds")?;
        let padding_end =
            usize::try_from(expected_offset).map_err(|_| "fcbc.section-table-bounds")?;
        if bytes[padding_start..padding_end]
            .iter()
            .any(|byte| *byte != 0)
        {
            return Err("fcbc.section-order");
        }
        let payload = section_slice(bytes, section)?;
        if crc32_iso_hdlc(payload) != section.info.checksum {
            return Err("fcbc.section-checksum");
        }
        layout_cursor = section
            .info
            .offset
            .checked_add(section.info.length)
            .ok_or("fcbc.section-table-bounds")?;
    }
    if layout_cursor != bytes.len() as u64 {
        return Err("fcbc.section-order");
    }

    let required: BTreeSet<u32> = (1..=13).chain(std::iter::once(20)).collect();
    for section_type in required {
        let Some(section) = sections
            .iter()
            .find(|section| section.info.section_type == section_type)
        else {
            return Err("fcbc.missing-required-section");
        };
        if section.flags & 1 == 0 || section.version != (1, 0, 0) {
            return Err("fcbc.missing-required-section");
        }
    }
    for section in &sections {
        if !(1..=20).contains(&section.info.section_type) && section.flags & 1 != 0 {
            return Err("fcbc.unknown-required-section");
        }
    }
    validate_feature_sections(feature_flags, &sections)?;

    Ok((
        profile,
        feature_flags,
        [compiler_id, compiler_version],
        sections,
    ))
}

fn validate_feature_sections(
    feature_flags: u64,
    sections: &[RawSection],
) -> Result<(), &'static str> {
    let bindings = [(1, 14), (2, 15), (3, 16), (4, 17), (5, 18), (6, 19)];
    for (bit, section_type) in bindings {
        let expected = feature_flags & (1 << bit) != 0;
        let actual = sections
            .iter()
            .find(|section| section.info.section_type == section_type);
        if expected != actual.is_some() {
            return Err("fcbc.profile-requirement-missing");
        }
        if actual.is_some_and(|section| section.flags & 1 == 0) {
            return Err("fcbc.profile-requirement-missing");
        }
    }
    Ok(())
}

fn section_payload<'a>(
    bytes: &'a [u8],
    section_map: &BTreeMap<u32, &RawSection>,
    section_type: u32,
) -> Result<&'a [u8], &'static str> {
    let section = section_map
        .get(&section_type)
        .ok_or("fcbc.missing-required-section")?;
    section_slice(bytes, section)
}

fn section_slice<'a>(bytes: &'a [u8], section: &RawSection) -> Result<&'a [u8], &'static str> {
    let start = usize::try_from(section.info.offset).map_err(|_| "fcbc.section-table-bounds")?;
    let length = usize::try_from(section.info.length).map_err(|_| "fcbc.section-table-bounds")?;
    let end = start
        .checked_add(length)
        .ok_or("fcbc.section-table-bounds")?;
    bytes.get(start..end).ok_or("fcbc.section-table-bounds")
}

fn parse_string_table(bytes: &[u8]) -> Result<Vec<String>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-string-table");
    let count = limited_count(cursor.u32()?)?;
    let mut offsets = Vec::with_capacity(count + 1);
    for _ in 0..=count {
        offsets.push(cursor.u32()? as usize);
    }
    if offsets.first() != Some(&0) || offsets.windows(2).any(|pair| pair[0] > pair[1]) {
        return Err("fcbc.invalid-string-table");
    }
    let utf8_length = *offsets.last().ok_or("fcbc.invalid-string-table")?;
    let utf8 = cursor.take(utf8_length)?;
    let mut strings = Vec::with_capacity(count);
    for pair in offsets.windows(2) {
        let value = std::str::from_utf8(&utf8[pair[0]..pair[1]])
            .map_err(|_| "fcbc.invalid-string-table")?;
        strings.push(value.to_owned());
    }
    let expected_total =
        align_up_usize(4 + (count + 1) * 4 + utf8_length, 8).ok_or("fcbc.invalid-string-table")?;
    if expected_total != bytes.len() {
        return Err("fcbc.invalid-string-table");
    }
    cursor.zeroes(cursor.remaining())?;
    if strings
        .windows(2)
        .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
    {
        return Err("fcbc.invalid-string-table");
    }
    Ok(strings)
}

fn parse_constant_pool(bytes: &[u8]) -> Result<Vec<RuntimeValue>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut constants = Vec::with_capacity(count);
    let mut prior_encoding: Option<Vec<u8>> = None;
    for _ in 0..count {
        let start = cursor.position;
        let value = parse_runtime_constant(&mut cursor)?;
        let encoding = bytes[start..cursor.position].to_vec();
        if prior_encoding
            .as_ref()
            .is_some_and(|prior| (prior[0], &prior[8..]) >= (encoding[0], &encoding[8..]))
        {
            return Err("fcbc.invalid-record");
        }
        prior_encoding = Some(encoding);
        constants.push(value);
    }
    cursor.finish()?;
    Ok(constants)
}

fn parse_runtime_constant(cursor: &mut Cursor<'_>) -> Result<RuntimeValue, &'static str> {
    let start = cursor.position;
    let tag = cursor.u8()?;
    if cursor.u8()? != 0 || cursor.u16()? != 0 {
        return Err("fcbc.invalid-record");
    }
    let payload_length = cursor.u32()? as usize;
    let payload = cursor.take(payload_length)?;
    let mut value = Cursor::new(payload, "fcbc.invalid-record");
    let result = match tag {
        1 => {
            let boolean = value.u8()?;
            if boolean > 1 {
                return Err("fcbc.invalid-record");
            }
            value.zeroes(7)?;
            RuntimeValue::Bool(boolean == 1)
        }
        2 => RuntimeValue::Int(value.i64()?),
        3 => RuntimeValue::Scalar {
            ty: ValueType::Float,
            value: value.f64()?,
        },
        5 => RuntimeValue::Scalar {
            ty: ValueType::Time,
            value: value.f64()?,
        },
        6 => {
            let numerator = value.i64()?;
            let denominator = value.i64()?;
            if denominator <= 0 {
                return Err("fcbc.invalid-record");
            }
            RuntimeValue::Scalar {
                ty: ValueType::Beat,
                value: numerator as f64 / denominator as f64,
            }
        }
        7 => RuntimeValue::Scalar {
            ty: ValueType::Length,
            value: value.f64()?,
        },
        8 => RuntimeValue::Scalar {
            ty: ValueType::Angle,
            value: value.f64()?,
        },
        9 => RuntimeValue::Color([value.f64()?, value.f64()?, value.f64()?, value.f64()?]),
        10 => {
            let element_tag = value.u8()?;
            value.zeroes(7)?;
            let element = scalar_tag_type(element_tag).ok_or("fcbc.invalid-record")?;
            let ty = ValueType::vector_of(element).ok_or("fcbc.invalid-record")?;
            RuntimeValue::Vec2 {
                ty,
                value: [value.f64()?, value.f64()?],
            }
        }
        11 => RuntimeValue::ResourceRef(value.u64()?),
        12 => RuntimeValue::ContributorRef(value.u64()?),
        _ => return Err("fcbc.invalid-record"),
    };
    value.finish()?;
    let consumed = cursor.position - start;
    let padding = (8 - consumed % 8) % 8;
    cursor.zeroes(padding)?;
    Ok(result)
}

fn parse_meta(bytes: &[u8], strings: &[String]) -> Result<u8, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let document_profile = cursor.u8()?;
    if !(1..=5).contains(&document_profile) {
        return Err("fcbc.invalid-record");
    }
    cursor.zeroes(3)?;
    let document_features = cursor.u32()?;
    if document_features & !0b11 != 0 {
        return Err("fcbc.invalid-record");
    }
    let meta = parse_value(&mut cursor, strings.len())?;
    let artwork = parse_value(&mut cursor, strings.len())?;
    if meta.tag != 14 || artwork.tag != 14 {
        return Err("fcbc.invalid-record");
    }
    cursor.finish()?;
    Ok(document_profile)
}

fn parse_contributors(bytes: &[u8], strings: &[String]) -> Result<BTreeSet<u64>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut ids = BTreeSet::new();
    let mut prior_id = None;
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let id = record.u64()?;
        if id == 0 || !ids.insert(id) || prior_id.is_some_and(|prior| prior >= id) {
            return Err("fcbc.duplicate-id");
        }
        prior_id = Some(id);
        let name = record.u32()?;
        check_string_ref(name, strings.len())?;
        if strings[name as usize].is_empty() {
            return Err("fcbc.invalid-record");
        }
        let alias_count = limited_count(record.u32()?)?;
        for _ in 0..alias_count {
            check_string_ref(record.u32()?, strings.len())?;
        }
        if parse_value(&mut record, strings.len())?.tag != 14
            || parse_value(&mut record, strings.len())?.tag != 14
        {
            return Err("fcbc.invalid-record");
        }
    }
    cursor.finish()?;
    Ok(ids)
}

fn parse_credits(
    bytes: &[u8],
    strings: &[String],
    contributor_ids: &BTreeSet<u64>,
) -> Result<(), &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut stable_ids = BTreeSet::new();
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let stable_id = record.u64()?;
        if stable_id == 0 || !stable_ids.insert(stable_id) {
            return Err("fcbc.duplicate-id");
        }
        let role = record.u16()?;
        if record.u16()? != 0 {
            return Err("fcbc.invalid-record");
        }
        let custom_role = optional_index(record.u32()?);
        if (role == 0) != custom_role.is_some() || role > 12 {
            return Err("fcbc.invalid-record");
        }
        if let Some(reference) = custom_role {
            check_string_ref(reference, strings.len())?;
        }
        check_string_ref(record.u32()?, strings.len())?;
        let contributor_count = limited_count(record.u32()?)?;
        for _ in 0..contributor_count {
            if !contributor_ids.contains(&record.u64()?) {
                return Err("fcbc.dangling-reference");
            }
        }
        if parse_value(&mut record, strings.len())?.tag != 14 {
            return Err("fcbc.invalid-record");
        }
    }
    cursor.finish()
}

fn parse_resources(bytes: &[u8], strings: &[String]) -> Result<Vec<ResourceRecord>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut resources = Vec::with_capacity(count);
    let mut prior_id = None;
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let id = record.u64()?;
        if id == 0 || prior_id.is_some_and(|prior| prior >= id) {
            return Err("fcbc.duplicate-id");
        }
        prior_id = Some(id);
        let kind = record.u16()?;
        if !(1..=7).contains(&kind) || record.u16()? != 0 {
            return Err("fcbc.invalid-record");
        }
        check_string_ref(record.u32()?, strings.len())?;
        if record.u16()? != 1 || record.u16()? != 0 {
            return Err("fcbc.invalid-record");
        }
        let data_offset = record.u64()?;
        let data_length = record.u64()?;
        let hash = parse_bytes(&mut record)?;
        if hash.len() != 32 || parse_value(&mut record, strings.len())?.tag != 14 {
            return Err("fcbc.invalid-record");
        }
        resources.push(ResourceRecord {
            id,
            kind,
            data_offset,
            data_length,
            hash,
        });
    }
    cursor.finish()?;
    Ok(resources)
}

fn validate_resource_data(bytes: &[u8], resources: &[ResourceRecord]) -> Result<(), &'static str> {
    let mut cursor = 0usize;
    for resource in resources {
        let expected = align_up_usize(cursor, 8).ok_or("fcbc.invalid-resource-data")?;
        if resource.data_offset != expected as u64 {
            return Err("fcbc.invalid-resource-data");
        }
        if bytes[cursor..expected].iter().any(|byte| *byte != 0) {
            return Err("fcbc.invalid-resource-data");
        }
        let length =
            usize::try_from(resource.data_length).map_err(|_| "fcbc.invalid-resource-data")?;
        let end = expected
            .checked_add(length)
            .ok_or("fcbc.invalid-resource-data")?;
        let payload = bytes
            .get(expected..end)
            .ok_or("fcbc.invalid-resource-data")?;
        let digest = Sha256::digest(payload);
        if digest.as_slice() != resource.hash {
            return Err("fcbc.resource-hash-mismatch");
        }
        cursor = end;
    }
    if cursor != bytes.len() {
        return Err("fcbc.invalid-resource-data");
    }
    Ok(())
}

fn parse_sync(bytes: &[u8], resources: &[ResourceRecord]) -> Result<(), &'static str> {
    let mut outer = Cursor::new(bytes, "fcbc.invalid-record");
    let mut record = take_record(&mut outer)?;
    let primary_audio = record.u64()?;
    record.f64()?;
    let has_preview = record.u8()?;
    if has_preview > 1 {
        return Err("fcbc.invalid-record");
    }
    record.zeroes(7)?;
    let preview_start = record.f64()?;
    let preview_end = record.f64()?;
    if has_preview == 0 && (preview_start.to_bits() != 0 || preview_end.to_bits() != 0) {
        return Err("fcbc.invalid-record");
    }
    if has_preview == 1 && preview_end < preview_start {
        return Err("fcbc.invalid-record");
    }
    if primary_audio != 0
        && !resources
            .iter()
            .any(|resource| resource.id == primary_audio && resource.kind == 1)
    {
        return Err("fcbc.dangling-reference");
    }
    outer.finish()
}

fn parse_tempo(bytes: &[u8]) -> Result<Vec<TempoPoint>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-tempo");
    let count = limited_count(cursor.u32()?)?;
    if count == 0 {
        return Err("fcbc.invalid-tempo");
    }
    let mut points = Vec::with_capacity(count);
    for _ in 0..count {
        let point = TempoPoint {
            beat_numerator: cursor.i64()?,
            beat_denominator: cursor.i64()?,
            chart_time: cursor.f64()?,
            bpm: cursor.f64()?,
            source_order: cursor.u32()?,
        };
        if cursor.u32()? != 0 || point.beat_denominator <= 0 || point.bpm <= 0.0 {
            return Err("fcbc.invalid-tempo");
        }
        points.push(point);
    }
    cursor.finish()?;
    if points[0].beat_numerator != 0 {
        return Err("fcbc.invalid-tempo");
    }
    for pair in points.windows(2) {
        let left = pair[0].beat_numerator as i128 * pair[1].beat_denominator as i128;
        let right = pair[1].beat_numerator as i128 * pair[0].beat_denominator as i128;
        if left > right || (left == right && pair[0].source_order >= pair[1].source_order) {
            return Err("fcbc.invalid-tempo");
        }
    }
    Ok(points)
}

fn parse_lines(bytes: &[u8], strings: &[String]) -> Result<Vec<LineRecord>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut lines = Vec::with_capacity(count);
    let mut prior_id = None;
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let id = record.u64()?;
        if id == 0 || prior_id.is_some_and(|prior| prior >= id) {
            return Err("fcbc.duplicate-id");
        }
        prior_id = Some(id);
        let parent_id = record.u64()?;
        let document_order = record.u32()?;
        record.i32()?; // zOrder
        let inherit_flags = record.u32()?;
        let line_flags = record.u32()?;
        if inherit_flags & !0x1f != 0 || line_flags & !1 != 0 {
            return Err("fcbc.invalid-track");
        }
        let line = LineRecord {
            id,
            parent_id,
            document_order,
            line_flags,
            position_descriptor: record.u32()?,
            rotation_descriptor: record.u32()?,
            scale_descriptor: record.u32()?,
            alpha_descriptor: record.u32()?,
            transform_origin_constant: record.u32()?,
            texture_anchor_constant: record.u32()?,
            scroll_tempo_descriptor: record.u32()?,
            scroll_speed_descriptor: record.u32()?,
            distance_descriptor: record.u32()?,
            floor_scale: record.f64()?,
            integration_origin: record.f64()?,
            initial_floor_position: record.f64()?,
        };
        if parse_value(&mut record, strings.len())?.tag != 14 {
            return Err("fcbc.invalid-track");
        }
        lines.push(line);
    }
    cursor.finish()?;
    Ok(lines)
}

fn parse_notes(bytes: &[u8], strings: &[String]) -> Result<Vec<NoteRecord>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut notes = Vec::with_capacity(count);
    let mut ids = BTreeSet::new();
    let mut prior_sort_key: Option<(u64, u64, u32, u64)> = None;
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let id = record.u64()?;
        let line_id = record.u64()?;
        let document_order = record.u32()?;
        let kind = record.u8()?;
        let side = record.u8()?;
        let flags = record.u16()?;
        let time = record.f64()?;
        let end_time = record.f64()?;
        if id == 0 || !ids.insert(id) {
            return Err("fcbc.duplicate-id");
        }
        let sort_key = (time.to_bits(), line_id, document_order, id);
        if prior_sort_key.is_some_and(|prior| prior >= sort_key) {
            return Err("fcbc.invalid-note");
        }
        prior_sort_key = Some(sort_key);
        if !(1..=4).contains(&kind) || !(1..=2).contains(&side) || flags & !0b111 != 0 {
            return Err("fcbc.invalid-note");
        }
        let has_end = flags & 0b100 != 0;
        if (kind == 2) != has_end
            || (kind == 2 && end_time <= time)
            || (kind != 2 && end_time.to_bits() != 0)
        {
            return Err("fcbc.invalid-note");
        }
        let judge_shape = parse_value(&mut record, strings.len())?;
        validate_judge_shape(&judge_shape, strings)?;
        let sound_policy = record.u16()?;
        let score_policy = record.u16()?;
        let sound_resource = record.u64()?;
        let score_extension = optional_index(record.u32()?);
        if record.u32()? != 0
            || !(1..=3).contains(&sound_policy)
            || !(1..=3).contains(&score_policy)
        {
            return Err("fcbc.invalid-note");
        }
        if (sound_policy == 3) != (sound_resource != 0) {
            return Err("fcbc.invalid-note");
        }
        if (score_policy == 3) != score_extension.is_some() {
            return Err("fcbc.invalid-note");
        }
        if let Some(reference) = score_extension {
            check_string_ref(reference, strings.len())?;
        }
        if flags & 1 == 0
            && (sound_policy != 2
                || score_policy != 2
                || sound_resource != 0
                || score_extension.is_some())
        {
            return Err("fcbc.invalid-note");
        }
        let mut property_descriptors = [0u32; 10];
        for descriptor in &mut property_descriptors {
            *descriptor = record.u32()?;
        }
        let texture_resource = record.u64()?;
        if parse_value(&mut record, strings.len())?.tag != 14 {
            return Err("fcbc.invalid-note");
        }
        notes.push(NoteRecord {
            id,
            line_id,
            document_order,
            kind,
            side,
            flags,
            time,
            end_time,
            property_descriptors,
            sound_resource_id: sound_resource,
            texture_resource_id: texture_resource,
        });
    }
    cursor.finish()?;
    Ok(notes)
}

fn parse_tracks(bytes: &[u8]) -> Result<Vec<PropertyDescriptor>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut descriptors = Vec::with_capacity(count);
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let property_type = ValueType::from_abi(record.u8()?)?;
        if property_type as u8 > 10 {
            return Err("fcbc.invalid-track");
        }
        let descriptor_kind = record.u8()?;
        let flags = record.u16()?;
        let domain_start = record.f64()?;
        let domain_end = record.f64()?;
        let domain = parse_domain(flags, domain_start, domain_end, "fcbc.invalid-track")?;
        let kind = match descriptor_kind {
            1 => DescriptorKind::Constant(record.u32()?),
            2 => {
                let segment_count = limited_count(record.u32()?)?;
                if segment_count == 0 {
                    return Err("fcbc.invalid-track");
                }
                let mut segments = Vec::with_capacity(segment_count);
                for _ in 0..segment_count {
                    segments.push(Segment {
                        start: record.f64()?,
                        end: record.f64()?,
                        interpolation: record.u16()?,
                        easing: record.u16()?,
                        flags: record.u32()?,
                        start_constant: record.u32()?,
                        end_constant: record.u32()?,
                        bezier: [record.f64()?, record.f64()?, record.f64()?, record.f64()?],
                    });
                }
                DescriptorKind::SegmentTrack(segments)
            }
            3 => {
                let piece_count = limited_count(record.u32()?)?;
                if piece_count == 0 {
                    return Err("fcbc.invalid-track");
                }
                let mut pieces = Vec::with_capacity(piece_count);
                for _ in 0..piece_count {
                    pieces.push(Piece {
                        start: record.f64()?,
                        end: record.f64()?,
                        descriptor_index: record.u32()?,
                        flags: record.u32()?,
                    });
                }
                DescriptorKind::Piecewise(pieces)
            }
            4 => DescriptorKind::Expression(record.u32()?),
            5 => return Err("fcbc.forbidden-descriptor"),
            _ => return Err("fcbc.invalid-track"),
        };
        descriptors.push(PropertyDescriptor {
            property_type,
            domain,
            kind,
        });
    }
    cursor.finish()?;
    Ok(descriptors)
}

fn parse_expressions(bytes: &[u8]) -> Result<Vec<ExpressionNode>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-expression");
    let count = limited_count(cursor.u32()?)?;
    let mut expressions = Vec::with_capacity(count);
    for _ in 0..count {
        expressions.push(ExpressionNode {
            opcode: cursor.u16()?,
            result_type: ValueType::from_abi(cursor.u8()?)
                .map_err(|_| "fcbc.invalid-expression")?,
            arity: cursor.u8()?,
            operands: [cursor.u32()?, cursor.u32()?, cursor.u32()?],
            immediate: cursor.u32()?,
        });
    }
    cursor.finish()?;
    Ok(expressions)
}

fn parse_distances(bytes: &[u8]) -> Result<Vec<DistanceDescriptor>, &'static str> {
    let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
    let count = limited_count(cursor.u32()?)?;
    let mut distances = Vec::with_capacity(count);
    for _ in 0..count {
        let mut record = take_record(&mut cursor)?;
        let line_id = record.u64()?;
        let scroll_speed_descriptor = record.u32()?;
        if record.u32()? != NULL_INDEX {
            return Err("fcbc.invalid-distance");
        }
        let domain_start = record.f64()?;
        let domain_end = record.f64()?;
        let integration_origin = record.f64()?;
        let initial_floor_position = record.f64()?;
        let max_velocity_error = record.f64()?;
        let max_distance_error = record.f64()?;
        let boundary_count = limited_count(record.u32()?)?;
        let classification = match record.u8()? {
            1 => DistanceClassification::PortableAnalytic,
            2 => DistanceClassification::PortableEvaluable,
            3 => return Err("fcbc.invalid-distance"),
            _ => return Err("fcbc.invalid-distance"),
        };
        let flags = record.u8()?;
        if record.u16()? != 0 {
            return Err("fcbc.invalid-distance");
        }
        let domain = parse_domain(
            u16::from(flags),
            domain_start,
            domain_end,
            "fcbc.invalid-distance",
        )?;
        let mut boundaries = Vec::with_capacity(boundary_count);
        for _ in 0..boundary_count {
            boundaries.push(record.f64()?);
        }
        distances.push(DistanceDescriptor {
            line_id,
            scroll_speed_descriptor,
            domain,
            integration_origin,
            initial_floor_position,
            max_velocity_error,
            max_distance_error,
            boundaries,
            classification,
        });
    }
    cursor.finish()?;
    Ok(distances)
}

fn validate_expression_signatures(
    nodes: &[ExpressionNode],
    constants: &[RuntimeValue],
) -> Result<(), &'static str> {
    for (index, node) in nodes.iter().enumerate() {
        if node.arity > 3 {
            return Err("fcbc.invalid-expression");
        }
        for operand in &node.operands[..node.arity as usize] {
            if *operand as usize >= index {
                return Err("fcbc.invalid-expression");
            }
        }
        if node.operands[node.arity as usize..]
            .iter()
            .any(|operand| *operand != NULL_INDEX)
        {
            return Err("fcbc.invalid-expression");
        }
        if !matches!(node.opcode, 1 | 60) && node.immediate != 0 {
            return Err("fcbc.invalid-expression");
        }
        let operands: Vec<ValueType> = node.operands[..node.arity as usize]
            .iter()
            .map(|operand| nodes[*operand as usize].result_type)
            .collect();
        validate_expression_signature(node, &operands, constants)?;
    }
    Ok(())
}

fn validate_expression_signature(
    node: &ExpressionNode,
    operands: &[ValueType],
    constants: &[RuntimeValue],
) -> Result<(), &'static str> {
    let ty = node.result_type;
    let valid = match node.opcode {
        1 => {
            let Some(constant) = constants.get(node.immediate as usize) else {
                return Err("fcbc.invalid-expression");
            };
            node.arity == 0 && runtime_value_type(constant) == Some(ty)
        }
        2 => node.arity == 0 && ty == ValueType::Time,
        3 => node.arity == 0 && ty == ValueType::Beat,
        4 => node.arity == 0 && ty == ValueType::Float,
        5 => node.arity == 0 && ty == ValueType::Length,
        6 => node.arity == 0 && ty == ValueType::Float,
        10 => unary_same(node, operands) && is_numeric_scalar(ty),
        11 => unary(node, operands, ValueType::Bool, ValueType::Bool),
        20 | 21 => binary_same(node, operands) && (is_numeric_scalar(ty) || ty.is_vector()),
        22 => valid_mul(operands, ty),
        23 => valid_div(operands, ty),
        24 => binary(
            node,
            operands,
            ValueType::Int,
            ValueType::Int,
            ValueType::Int,
        ),
        25 => binary_same(node, operands) && matches!(ty, ValueType::Int | ValueType::Float),
        30 | 31 => {
            node.arity == 2
                && operands.len() == 2
                && operands[0] == operands[1]
                && ty == ValueType::Bool
        }
        32..=35 => {
            node.arity == 2
                && operands.len() == 2
                && operands[0] == operands[1]
                && is_numeric_scalar(operands[0])
                && ty == ValueType::Bool
        }
        36 | 37 => binary(
            node,
            operands,
            ValueType::Bool,
            ValueType::Bool,
            ValueType::Bool,
        ),
        38 => {
            node.arity == 3
                && operands == [ValueType::Float, ValueType::Float, ValueType::Float]
                && ty == ValueType::Bool
        }
        40 => unary_same(node, operands) && is_numeric_scalar(ty),
        41 | 42 => binary_same(node, operands) && is_numeric_scalar(ty),
        43 => {
            node.arity == 3
                && operands.len() == 3
                && operands.iter().all(|operand| *operand == ty)
                && is_numeric_scalar(ty)
        }
        44..=55 => unary(node, operands, ValueType::Float, ValueType::Float),
        56 => binary(
            node,
            operands,
            ValueType::Float,
            ValueType::Float,
            ValueType::Float,
        ),
        60 => unary(node, operands, ValueType::Float, ValueType::Float) && node.immediate <= 30,
        61 => unary(node, operands, ValueType::Int, ValueType::Float),
        62 => unary(node, operands, ValueType::Time, ValueType::Float),
        63 => unary(node, operands, ValueType::Angle, ValueType::Float),
        70 => {
            node.arity == 3
                && operands.len() == 3
                && operands[0] == ValueType::Bool
                && operands[1] == operands[2]
                && ty == operands[1]
        }
        80 => {
            node.arity == 2
                && operands.len() == 2
                && operands[0] == operands[1]
                && ValueType::vector_of(operands[0]) == Some(ty)
        }
        81 | 82 => {
            node.arity == 1 && operands.len() == 1 && operands[0].vector_element() == Some(ty)
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err("fcbc.invalid-expression")
    }
}

fn valid_mul(operands: &[ValueType], result: ValueType) -> bool {
    if operands.len() != 2 {
        return false;
    }
    let left = operands[0];
    let right = operands[1];
    if left == right && matches!(left, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    if is_unit_scalar(left) && matches!(right, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    if is_unit_scalar(right) && matches!(left, ValueType::Int | ValueType::Float) {
        return result == right;
    }
    if left == ValueType::Vec2Int && right == ValueType::Int {
        return result == left;
    }
    if right == ValueType::Vec2Int && left == ValueType::Int {
        return result == right;
    }
    if left == ValueType::Vec2Float && right == ValueType::Float {
        return result == left;
    }
    if right == ValueType::Vec2Float && left == ValueType::Float {
        return result == right;
    }
    if is_unit_vector(left) && matches!(right, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    if is_unit_vector(right) && matches!(left, ValueType::Int | ValueType::Float) {
        return result == right;
    }
    false
}

fn valid_div(operands: &[ValueType], result: ValueType) -> bool {
    if operands.len() != 2 {
        return false;
    }
    let left = operands[0];
    let right = operands[1];
    if left == right && matches!(left, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    if is_unit_scalar(left) && matches!(right, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    if is_unit_scalar(left) && left == right {
        return result == ValueType::Float;
    }
    if left == ValueType::Vec2Int && right == ValueType::Int {
        return result == left;
    }
    if left == ValueType::Vec2Float && right == ValueType::Float {
        return result == left;
    }
    if is_unit_vector(left) && matches!(right, ValueType::Int | ValueType::Float) {
        return result == left;
    }
    false
}

fn unary_same(node: &ExpressionNode, operands: &[ValueType]) -> bool {
    node.arity == 1 && operands == [node.result_type]
}

fn binary_same(node: &ExpressionNode, operands: &[ValueType]) -> bool {
    node.arity == 2
        && operands.len() == 2
        && operands[0] == node.result_type
        && operands[1] == node.result_type
}

fn unary(
    node: &ExpressionNode,
    operands: &[ValueType],
    operand: ValueType,
    result: ValueType,
) -> bool {
    node.arity == 1 && operands == [operand] && node.result_type == result
}

fn binary(
    node: &ExpressionNode,
    operands: &[ValueType],
    left: ValueType,
    right: ValueType,
    result: ValueType,
) -> bool {
    node.arity == 2 && operands == [left, right] && node.result_type == result
}

fn is_numeric_scalar(ty: ValueType) -> bool {
    matches!(
        ty,
        ValueType::Int
            | ValueType::Float
            | ValueType::Time
            | ValueType::Beat
            | ValueType::Length
            | ValueType::Angle
    )
}

fn is_unit_scalar(ty: ValueType) -> bool {
    matches!(
        ty,
        ValueType::Time | ValueType::Beat | ValueType::Length | ValueType::Angle
    )
}

fn is_unit_vector(ty: ValueType) -> bool {
    matches!(
        ty,
        ValueType::Vec2Time | ValueType::Vec2Beat | ValueType::Vec2Length | ValueType::Vec2Angle
    )
}

fn validate_descriptors(
    descriptors: &[PropertyDescriptor],
    constants: &[RuntimeValue],
    expressions: &[ExpressionNode],
) -> Result<(), &'static str> {
    for descriptor in descriptors {
        match &descriptor.kind {
            DescriptorKind::Constant(index) => {
                let value = constants
                    .get(*index as usize)
                    .ok_or("fcbc.dangling-reference")?;
                if runtime_value_type(value) != Some(descriptor.property_type) {
                    return Err("fcbc.invalid-track");
                }
            }
            DescriptorKind::SegmentTrack(segments) => {
                validate_segments(descriptor, segments, constants)?;
            }
            DescriptorKind::Piecewise(pieces) => {
                validate_pieces(descriptor, pieces, descriptors)?;
            }
            DescriptorKind::Expression(root) => {
                let root = expressions
                    .get(*root as usize)
                    .ok_or("fcbc.invalid-expression")?;
                if root.result_type != descriptor.property_type {
                    return Err("fcbc.invalid-expression");
                }
            }
        }
    }
    validate_piecewise_acyclic(descriptors)
}

fn validate_segments(
    descriptor: &PropertyDescriptor,
    segments: &[Segment],
    constants: &[RuntimeValue],
) -> Result<(), &'static str> {
    if descriptor.domain.unbounded_before
        && segments
            .first()
            .is_none_or(|segment| segment.flags & 1 == 0)
        || descriptor.domain.unbounded_after
            && segments.last().is_none_or(|segment| segment.flags & 1 == 0)
    {
        return Err("fcbc.invalid-track");
    }
    let mut prior: Option<&Segment> = None;
    let mut ordinary: Vec<&Segment> = Vec::new();
    for segment in segments {
        if !segment.start.is_finite()
            || !segment.end.is_finite()
            || segment.bezier.iter().any(|value| !value.is_finite())
            || segment.flags & !1 != 0
        {
            return Err("fcbc.invalid-track");
        }
        let is_point = segment.flags & 1 != 0;
        if let Some(previous) = prior {
            match previous.start.total_cmp(&segment.start) {
                std::cmp::Ordering::Greater => return Err("fcbc.invalid-track"),
                std::cmp::Ordering::Equal if previous.flags & 1 == 0 || is_point => {
                    return Err("fcbc.invalid-track");
                }
                _ => {}
            }
        }
        prior = Some(segment);
        let start = constants
            .get(segment.start_constant as usize)
            .ok_or("fcbc.dangling-reference")?;
        let end = constants
            .get(segment.end_constant as usize)
            .ok_or("fcbc.dangling-reference")?;
        if runtime_value_type(start) != Some(descriptor.property_type)
            || runtime_value_type(end) != Some(descriptor.property_type)
        {
            return Err("fcbc.invalid-track");
        }
        if is_point {
            if segment.start.to_bits() != segment.end.to_bits()
                || segment.interpolation != 1
                || segment.easing != 0
                || segment.bezier.iter().any(|value| value.to_bits() != 0)
                || start != end
            {
                return Err("fcbc.invalid-track");
            }
            if ordinary.iter().any(|ordinary_segment| {
                ordinary_segment.start < segment.start && segment.start < ordinary_segment.end
            }) {
                return Err("fcbc.invalid-track");
            }
            continue;
        }
        if segment.start >= segment.end || !(1..=4).contains(&segment.interpolation) {
            return Err("fcbc.invalid-track");
        }
        if ordinary
            .last()
            .is_some_and(|previous| previous.end > segment.start)
        {
            return Err("fcbc.invalid-track");
        }
        let interpolable = matches!(
            descriptor.property_type,
            ValueType::Float
                | ValueType::Time
                | ValueType::Beat
                | ValueType::Length
                | ValueType::Angle
                | ValueType::Color
                | ValueType::Vec2Float
                | ValueType::Vec2Length
        );
        if segment.interpolation != 1 && !interpolable {
            return Err("fcbc.invalid-track");
        }
        match segment.interpolation {
            3 if !(1..=30).contains(&segment.easing) => {
                return Err("fcbc.invalid-track");
            }
            4 if segment.easing != 0
                || !(0.0..=1.0).contains(&segment.bezier[0])
                || !(0.0..=1.0).contains(&segment.bezier[2]) =>
            {
                return Err("fcbc.invalid-track");
            }
            1 | 2
                if segment.easing != 0
                    || segment.bezier.iter().any(|value| value.to_bits() != 0) =>
            {
                return Err("fcbc.invalid-track");
            }
            3 if segment.bezier.iter().any(|value| value.to_bits() != 0) => {
                return Err("fcbc.invalid-track");
            }
            _ => {}
        }
        ordinary.push(segment);
    }
    if !descriptor.domain.unbounded_before
        && segments
            .first()
            .is_none_or(|segment| segment.start.to_bits() != descriptor.domain.start.to_bits())
        || !descriptor.domain.unbounded_after
            && segments
                .last()
                .is_none_or(|segment| segment.start.to_bits() != descriptor.domain.end.to_bits())
    {
        return Err("fcbc.invalid-track");
    }
    Ok(())
}

fn validate_pieces(
    descriptor: &PropertyDescriptor,
    pieces: &[Piece],
    descriptors: &[PropertyDescriptor],
) -> Result<(), &'static str> {
    let mut cursor = if descriptor.domain.unbounded_before {
        f64::NEG_INFINITY
    } else {
        descriptor.domain.start
    };
    for (index, piece) in pieces.iter().enumerate() {
        if piece.flags & !0b111 != 0 {
            return Err("fcbc.invalid-track");
        }
        let is_first = index == 0;
        let is_last = index + 1 == pieces.len();
        let unbounded_before = piece.flags & 0b010 != 0;
        let unbounded_after = piece.flags & 0b100 != 0;
        if unbounded_before != (is_first && descriptor.domain.unbounded_before)
            || unbounded_after != (is_last && descriptor.domain.unbounded_after)
            || unbounded_before && piece.start.to_bits() != 0
            || unbounded_after && piece.end.to_bits() != 0
            || !unbounded_before && !piece.start.is_finite()
            || !unbounded_after && !piece.end.is_finite()
            || piece.flags & 1 != 0 && (!is_last || unbounded_after)
        {
            return Err("fcbc.invalid-track");
        }
        let interpreted_start = if unbounded_before {
            f64::NEG_INFINITY
        } else {
            piece.start
        };
        let interpreted_end = if unbounded_after {
            f64::INFINITY
        } else {
            piece.end
        };
        if interpreted_start >= interpreted_end || interpreted_start.to_bits() != cursor.to_bits() {
            return Err("fcbc.invalid-track");
        }
        let target = descriptors
            .get(piece.descriptor_index as usize)
            .ok_or("fcbc.dangling-reference")?;
        let piece_domain = Domain {
            start: piece.start,
            end: piece.end,
            unbounded_before,
            unbounded_after,
        };
        if target.property_type != descriptor.property_type || !target.domain.covers(piece_domain) {
            return Err("fcbc.invalid-track");
        }
        cursor = interpreted_end;
    }
    let expected_end = if descriptor.domain.unbounded_after {
        f64::INFINITY
    } else {
        descriptor.domain.end
    };
    if cursor.to_bits() != expected_end.to_bits() {
        return Err("fcbc.invalid-track");
    }
    Ok(())
}

fn validate_piecewise_acyclic(descriptors: &[PropertyDescriptor]) -> Result<(), &'static str> {
    fn visit(
        index: usize,
        descriptors: &[PropertyDescriptor],
        state: &mut [u8],
    ) -> Result<(), &'static str> {
        match state[index] {
            1 => return Err("fcbc.invalid-track"),
            2 => return Ok(()),
            _ => {}
        }
        state[index] = 1;
        if let DescriptorKind::Piecewise(pieces) = &descriptors[index].kind {
            for piece in pieces {
                let target = piece.descriptor_index as usize;
                if target >= descriptors.len() {
                    return Err("fcbc.dangling-reference");
                }
                visit(target, descriptors, state)?;
            }
        }
        state[index] = 2;
        Ok(())
    }

    let mut state = vec![0; descriptors.len()];
    for index in 0..descriptors.len() {
        visit(index, descriptors, &mut state)?;
    }
    Ok(())
}

fn validate_canonical_reachability(
    descriptors: &[PropertyDescriptor],
    expressions: &[ExpressionNode],
    constants: &[RuntimeValue],
    lines: &[LineRecord],
    notes: &[NoteRecord],
) -> Result<(), &'static str> {
    let mut roots = Vec::new();
    for line in lines {
        roots.extend([
            ("line.alpha", line.id, line.alpha_descriptor),
            ("line.position", line.id, line.position_descriptor),
            ("line.rotation", line.id, line.rotation_descriptor),
            ("line.scale", line.id, line.scale_descriptor),
            ("line.scrollSpeed", line.id, line.scroll_speed_descriptor),
            ("line.scrollTempo", line.id, line.scroll_tempo_descriptor),
        ]);
    }
    for note in notes {
        let indices = note.property_descriptors;
        roots.extend([
            ("note.presentation.positionX", note.id, indices[0]),
            ("note.presentation.scrollFactor", note.id, indices[1]),
            ("note.presentation.xOffset", note.id, indices[2]),
            ("note.presentation.yOffset", note.id, indices[3]),
            ("note.presentation.alpha", note.id, indices[4]),
            ("note.presentation.scaleX", note.id, indices[5]),
            ("note.presentation.scaleY", note.id, indices[6]),
            ("note.presentation.rotation", note.id, indices[7]),
            ("note.presentation.color", note.id, indices[8]),
            ("note.presentation.visibility", note.id, indices[9]),
        ]);
    }
    roots.sort_by(|left, right| {
        left.0
            .as_bytes()
            .cmp(right.0.as_bytes())
            .then_with(|| left.1.cmp(&right.1))
    });
    for (path, _, root) in &roots {
        if (path.starts_with("line.") || *path == "note.presentation.scrollFactor")
            && descriptor_uses_env_d(*root, descriptors, expressions, 0)?
        {
            return Err("fcbc.invalid-expression");
        }
    }

    fn visit_descriptor(
        index: u32,
        descriptors: &[PropertyDescriptor],
        visited: &mut BTreeSet<u32>,
        order: &mut Vec<u32>,
    ) -> Result<(), &'static str> {
        if visited.contains(&index) {
            return Ok(());
        }
        let descriptor = descriptors
            .get(index as usize)
            .ok_or("fcbc.dangling-reference")?;
        if let DescriptorKind::Piecewise(pieces) = &descriptor.kind {
            for piece in pieces {
                visit_descriptor(piece.descriptor_index, descriptors, visited, order)?;
            }
        }
        if index != order.len() as u32 {
            return Err("fcbc.invalid-track");
        }
        visited.insert(index);
        order.push(index);
        Ok(())
    }

    let mut visited_descriptors = BTreeSet::new();
    let mut descriptor_order = Vec::new();
    for (_, _, root) in roots {
        visit_descriptor(
            root,
            descriptors,
            &mut visited_descriptors,
            &mut descriptor_order,
        )?;
    }
    if descriptor_order.len() != descriptors.len() {
        return Err("fcbc.invalid-track");
    }

    let mut node_keys = Vec::with_capacity(expressions.len());
    let mut unique_node_keys = BTreeSet::new();
    for index in 0..expressions.len() {
        let key = expression_structural_key(index as u32, expressions, &mut node_keys)?;
        if !unique_node_keys.insert(key) {
            return Err("fcbc.invalid-expression");
        }
    }
    let mut descriptor_keys = Vec::with_capacity(descriptors.len());
    let mut unique_descriptor_keys = BTreeSet::new();
    for index in 0..descriptors.len() {
        let key = descriptor_structural_key(
            index as u32,
            descriptors,
            expressions,
            constants,
            &mut descriptor_keys,
            &mut node_keys,
        )?;
        if !unique_descriptor_keys.insert(key) {
            return Err("fcbc.invalid-track");
        }
    }

    fn visit_node(
        index: u32,
        expressions: &[ExpressionNode],
        visited: &mut BTreeSet<u32>,
        order: &mut Vec<u32>,
    ) -> Result<(), &'static str> {
        if visited.contains(&index) {
            return Ok(());
        }
        let node = expressions
            .get(index as usize)
            .ok_or("fcbc.invalid-expression")?;
        for operand in &node.operands[..node.arity as usize] {
            visit_node(*operand, expressions, visited, order)?;
        }
        if index != order.len() as u32 {
            return Err("fcbc.invalid-expression");
        }
        visited.insert(index);
        order.push(index);
        Ok(())
    }

    let mut visited_nodes = BTreeSet::new();
    let mut node_order = Vec::new();
    for descriptor in descriptors {
        if let DescriptorKind::Expression(root) = descriptor.kind {
            visit_node(root, expressions, &mut visited_nodes, &mut node_order)?;
        }
    }
    if node_order.len() != expressions.len() {
        return Err("fcbc.invalid-expression");
    }
    Ok(())
}

fn descriptor_uses_env_d(
    index: u32,
    descriptors: &[PropertyDescriptor],
    expressions: &[ExpressionNode],
    depth: usize,
) -> Result<bool, &'static str> {
    if depth > descriptors.len() + expressions.len() {
        return Err("fcbc.invalid-expression");
    }
    let descriptor = descriptors
        .get(index as usize)
        .ok_or("fcbc.dangling-reference")?;
    match &descriptor.kind {
        DescriptorKind::Constant(_) | DescriptorKind::SegmentTrack(_) => Ok(false),
        DescriptorKind::Piecewise(pieces) => {
            for piece in pieces {
                if descriptor_uses_env_d(
                    piece.descriptor_index,
                    descriptors,
                    expressions,
                    depth + 1,
                )? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        DescriptorKind::Expression(root) => expression_uses_env_d(*root, expressions, depth + 1),
    }
}

fn expression_uses_env_d(
    index: u32,
    expressions: &[ExpressionNode],
    depth: usize,
) -> Result<bool, &'static str> {
    if depth > expressions.len() {
        return Err("fcbc.invalid-expression");
    }
    let node = expressions
        .get(index as usize)
        .ok_or("fcbc.invalid-expression")?;
    if node.opcode == 5 {
        return Ok(true);
    }
    for operand in &node.operands[..node.arity as usize] {
        if expression_uses_env_d(*operand, expressions, depth + 1)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn expression_structural_key(
    index: u32,
    expressions: &[ExpressionNode],
    memo: &mut Vec<Vec<u8>>,
) -> Result<Vec<u8>, &'static str> {
    if let Some(key) = memo.get(index as usize) {
        return Ok(key.clone());
    }
    let node = expressions
        .get(index as usize)
        .ok_or("fcbc.invalid-expression")?;
    let mut key = Vec::new();
    key.extend_from_slice(&node.opcode.to_le_bytes());
    key.push(node.result_type as u8);
    key.push(node.arity);
    key.extend_from_slice(&node.immediate.to_le_bytes());
    for operand in &node.operands[..node.arity as usize] {
        let operand_key = expression_structural_key(*operand, expressions, memo)?;
        push_key_bytes(&mut key, &operand_key);
    }
    if memo.len() != index as usize {
        return Err("fcbc.invalid-expression");
    }
    memo.push(key.clone());
    Ok(key)
}

fn descriptor_structural_key(
    index: u32,
    descriptors: &[PropertyDescriptor],
    expressions: &[ExpressionNode],
    constants: &[RuntimeValue],
    memo: &mut Vec<Vec<u8>>,
    node_memo: &mut Vec<Vec<u8>>,
) -> Result<Vec<u8>, &'static str> {
    if let Some(key) = memo.get(index as usize) {
        return Ok(key.clone());
    }
    let descriptor = descriptors
        .get(index as usize)
        .ok_or("fcbc.invalid-track")?;
    let mut key = vec![descriptor.property_type as u8];
    let flags = u16::from(descriptor.domain.unbounded_before)
        | (u16::from(descriptor.domain.unbounded_after) << 1);
    key.extend_from_slice(&flags.to_le_bytes());
    key.extend_from_slice(&descriptor.domain.start.to_bits().to_le_bytes());
    key.extend_from_slice(&descriptor.domain.end.to_bits().to_le_bytes());
    match &descriptor.kind {
        DescriptorKind::Constant(constant) => {
            key.push(1);
            let value = constants
                .get(*constant as usize)
                .ok_or("fcbc.dangling-reference")?;
            push_key_bytes(&mut key, &runtime_value_key(value));
        }
        DescriptorKind::SegmentTrack(segments) => {
            key.push(2);
            key.extend_from_slice(&(segments.len() as u32).to_le_bytes());
            for segment in segments {
                key.extend_from_slice(&segment.start.to_bits().to_le_bytes());
                key.extend_from_slice(&segment.end.to_bits().to_le_bytes());
                key.extend_from_slice(&segment.interpolation.to_le_bytes());
                key.extend_from_slice(&segment.easing.to_le_bytes());
                key.extend_from_slice(&segment.flags.to_le_bytes());
                let start = constants
                    .get(segment.start_constant as usize)
                    .ok_or("fcbc.dangling-reference")?;
                let end = constants
                    .get(segment.end_constant as usize)
                    .ok_or("fcbc.dangling-reference")?;
                push_key_bytes(&mut key, &runtime_value_key(start));
                push_key_bytes(&mut key, &runtime_value_key(end));
                for bezier in segment.bezier {
                    key.extend_from_slice(&bezier.to_bits().to_le_bytes());
                }
            }
        }
        DescriptorKind::Piecewise(pieces) => {
            key.push(3);
            key.extend_from_slice(&(pieces.len() as u32).to_le_bytes());
            for piece in pieces {
                key.extend_from_slice(&piece.start.to_bits().to_le_bytes());
                key.extend_from_slice(&piece.end.to_bits().to_le_bytes());
                key.extend_from_slice(&piece.flags.to_le_bytes());
                let child_key = descriptor_structural_key(
                    piece.descriptor_index,
                    descriptors,
                    expressions,
                    constants,
                    memo,
                    node_memo,
                )?;
                push_key_bytes(&mut key, &child_key);
            }
        }
        DescriptorKind::Expression(root) => {
            key.push(4);
            let root_key = expression_structural_key(*root, expressions, node_memo)?;
            push_key_bytes(&mut key, &root_key);
        }
    }
    if memo.len() != index as usize {
        return Err("fcbc.invalid-track");
    }
    memo.push(key.clone());
    Ok(key)
}

fn runtime_value_key(value: &RuntimeValue) -> Vec<u8> {
    let mut key = Vec::new();
    match value {
        RuntimeValue::Bool(value) => {
            key.push(ValueType::Bool as u8);
            key.push(u8::from(*value));
        }
        RuntimeValue::Int(value) => {
            key.push(ValueType::Int as u8);
            key.extend_from_slice(&value.to_le_bytes());
        }
        RuntimeValue::Scalar { ty, value } => {
            key.push(*ty as u8);
            key.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        RuntimeValue::Color(value) => {
            key.push(ValueType::Color as u8);
            for component in value {
                key.extend_from_slice(&component.to_bits().to_le_bytes());
            }
        }
        RuntimeValue::Vec2 { ty, value } => {
            key.push(*ty as u8);
            for component in value {
                key.extend_from_slice(&component.to_bits().to_le_bytes());
            }
        }
        RuntimeValue::ResourceRef(value) => {
            key.push(15);
            key.extend_from_slice(&value.to_le_bytes());
        }
        RuntimeValue::ContributorRef(value) => {
            key.push(16);
            key.extend_from_slice(&value.to_le_bytes());
        }
    }
    key
}

fn push_key_bytes(target: &mut Vec<u8>, bytes: &[u8]) {
    target.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    target.extend_from_slice(bytes);
}

fn validate_lines(
    lines: &[LineRecord],
    descriptors: &[PropertyDescriptor],
    constants: &[RuntimeValue],
    distances: &[DistanceDescriptor],
) -> Result<(), &'static str> {
    let line_ids: BTreeSet<u64> = lines.iter().map(|line| line.id).collect();
    for line in lines {
        if line.parent_id != 0 && !line_ids.contains(&line.parent_id) {
            return Err("fcbc.dangling-reference");
        }
        check_descriptor_type(descriptors, line.position_descriptor, ValueType::Vec2Length)?;
        check_descriptor_type(descriptors, line.rotation_descriptor, ValueType::Angle)?;
        check_descriptor_type(descriptors, line.scale_descriptor, ValueType::Vec2Float)?;
        check_descriptor_type(descriptors, line.alpha_descriptor, ValueType::Float)?;
        check_constant_type(
            constants,
            line.transform_origin_constant,
            ValueType::Vec2Length,
        )?;
        check_constant_type(
            constants,
            line.texture_anchor_constant,
            ValueType::Vec2Float,
        )?;
        check_descriptor_type(descriptors, line.scroll_tempo_descriptor, ValueType::Float)?;
        check_descriptor_type(descriptors, line.scroll_speed_descriptor, ValueType::Float)?;
        for root in [
            line.position_descriptor,
            line.rotation_descriptor,
            line.scale_descriptor,
            line.alpha_descriptor,
            line.scroll_tempo_descriptor,
            line.scroll_speed_descriptor,
        ] {
            require_unbounded_root(descriptors, root, "fcbc.invalid-track")?;
        }
        let distance = distances
            .get(line.distance_descriptor as usize)
            .ok_or("fcbc.dangling-reference")?;
        if !line_ids.contains(&distance.line_id) {
            return Err("fcbc.dangling-reference");
        }
        if distance.line_id != line.id {
            return Err("fcbc.invalid-distance");
        }
    }
    for line in lines {
        let mut seen = BTreeSet::new();
        let mut current = line.parent_id;
        while current != 0 {
            if !seen.insert(current) {
                return Err("fcbc.parent-cycle");
            }
            current = lines
                .iter()
                .find(|candidate| candidate.id == current)
                .ok_or("fcbc.dangling-reference")?
                .parent_id;
        }
    }
    Ok(())
}

fn validate_notes(
    notes: &[NoteRecord],
    lines: &[LineRecord],
    descriptors: &[PropertyDescriptor],
    resources: &[ResourceRecord],
) -> Result<(), &'static str> {
    let expected_types = [
        ValueType::Length,
        ValueType::Float,
        ValueType::Length,
        ValueType::Length,
        ValueType::Float,
        ValueType::Float,
        ValueType::Float,
        ValueType::Angle,
        ValueType::Color,
        ValueType::Bool,
    ];
    for note in notes {
        if !lines.iter().any(|line| line.id == note.line_id) {
            return Err("fcbc.dangling-reference");
        }
        for (index, expected_type) in note.property_descriptors.iter().zip(expected_types.iter()) {
            let descriptor = descriptors
                .get(*index as usize)
                .ok_or("fcbc.dangling-reference")?;
            if descriptor.property_type != *expected_type {
                return Err("fcbc.invalid-note");
            }
            require_unbounded_root(descriptors, *index, "fcbc.invalid-note")?;
        }
        if note.sound_resource_id != 0
            && !resources
                .iter()
                .any(|resource| resource.id == note.sound_resource_id && resource.kind == 1)
        {
            return Err("fcbc.invalid-note");
        }
        if note.texture_resource_id != 0
            && !resources.iter().any(|resource| {
                resource.id == note.texture_resource_id && matches!(resource.kind, 2 | 4)
            })
        {
            return Err("fcbc.invalid-note");
        }
    }
    Ok(())
}

fn validate_distances(
    distances: &[DistanceDescriptor],
    lines: &[LineRecord],
    descriptors: &[PropertyDescriptor],
    constants: &[RuntimeValue],
) -> Result<(), &'static str> {
    let mut line_ids = BTreeSet::new();
    let mut prior_line_id = None;
    for distance in distances {
        if distance.line_id == 0
            || !line_ids.insert(distance.line_id)
            || prior_line_id.is_some_and(|prior| prior >= distance.line_id)
        {
            return Err("fcbc.invalid-distance");
        }
        prior_line_id = Some(distance.line_id);
        if !distance.domain.unbounded_before
            || !distance.domain.unbounded_after
            || distance.domain.start.to_bits() != 0
            || distance.domain.end.to_bits() != 0
            || !distance.domain.contains(distance.integration_origin)
            || !distance.initial_floor_position.is_finite()
            || distance.max_velocity_error.to_bits() != 0
        {
            return Err("fcbc.invalid-distance");
        }
        if distance
            .boundaries
            .windows(2)
            .any(|pair| pair[0].total_cmp(&pair[1]).is_ge())
            || distance
                .boundaries
                .iter()
                .any(|boundary| !boundary.is_finite())
            || !distance
                .boundaries
                .iter()
                .any(|boundary| boundary.to_bits() == distance.integration_origin.to_bits())
        {
            return Err("fcbc.invalid-distance");
        }
        let matching_lines: Vec<&LineRecord> = lines
            .iter()
            .filter(|line| line.id == distance.line_id)
            .collect();
        if matching_lines.is_empty() {
            return Err("fcbc.dangling-reference");
        }
        if matching_lines.len() != 1 {
            return Err("fcbc.invalid-distance");
        }
        let line = matching_lines[0];
        if distance.scroll_speed_descriptor != line.scroll_speed_descriptor
            || line.integration_origin.to_bits() != distance.integration_origin.to_bits()
            || line.initial_floor_position.to_bits() != distance.initial_floor_position.to_bits()
        {
            return Err("fcbc.invalid-distance");
        }
        let tempo = descriptors
            .get(line.scroll_tempo_descriptor as usize)
            .ok_or("fcbc.dangling-reference")?;
        let speed = descriptors
            .get(line.scroll_speed_descriptor as usize)
            .ok_or("fcbc.dangling-reference")?;
        if tempo.property_type != ValueType::Float
            || speed.property_type != ValueType::Float
            || !tempo.domain.unbounded_before
            || !tempo.domain.unbounded_after
            || !speed.domain.unbounded_before
            || !speed.domain.unbounded_after
        {
            return Err("fcbc.invalid-distance");
        }
        let expected_boundaries = expected_distance_boundaries(
            distance.integration_origin,
            line.scroll_speed_descriptor,
            line.scroll_tempo_descriptor,
            descriptors,
        )?;
        if distance.boundaries.len() != expected_boundaries.len()
            || distance
                .boundaries
                .iter()
                .zip(&expected_boundaries)
                .any(|(actual, expected)| actual.to_bits() != expected.to_bits())
        {
            return Err("fcbc.invalid-distance");
        }
        match distance.classification {
            DistanceClassification::PortableAnalytic => {
                if distance.max_distance_error.to_bits() != 0
                    || !matches!(&tempo.kind, DescriptorKind::Constant(_))
                    || !matches!(&speed.kind, DescriptorKind::Constant(_))
                {
                    return Err("fcbc.invalid-distance");
                }
                let tempo_value = descriptor_constant_scalar(tempo, constants)?;
                if tempo_value <= 0.0 {
                    return Err("fcbc.invalid-distance");
                }
            }
            DistanceClassification::PortableEvaluable => {
                if distance.max_distance_error.to_bits() != 2.328_306_436_538_696_3e-10f64.to_bits()
                    || matches!(&tempo.kind, DescriptorKind::Constant(_))
                        && matches!(&speed.kind, DescriptorKind::Constant(_))
                {
                    return Err("fcbc.invalid-distance");
                }
            }
        }
    }
    if distances.len() != lines.len()
        || lines.iter().any(|line| {
            distances
                .get(line.distance_descriptor as usize)
                .is_none_or(|distance| distance.line_id != line.id)
        })
    {
        return Err("fcbc.invalid-distance");
    }
    Ok(())
}

fn expected_distance_boundaries(
    integration_origin: f64,
    speed_descriptor: u32,
    tempo_descriptor: u32,
    descriptors: &[PropertyDescriptor],
) -> Result<Vec<f64>, &'static str> {
    fn collect(
        index: u32,
        descriptors: &[PropertyDescriptor],
        seen: &mut BTreeSet<u32>,
        boundaries: &mut Vec<f64>,
    ) -> Result<(), &'static str> {
        if !seen.insert(index) {
            return Ok(());
        }
        let descriptor = descriptors
            .get(index as usize)
            .ok_or("fcbc.dangling-reference")?;
        match &descriptor.kind {
            DescriptorKind::Constant(_) | DescriptorKind::Expression(_) => {}
            DescriptorKind::SegmentTrack(segments) => {
                for segment in segments {
                    boundaries.push(segment.start);
                    boundaries.push(segment.end);
                }
            }
            DescriptorKind::Piecewise(pieces) => {
                for piece in pieces {
                    if piece.flags & 0b010 == 0 {
                        boundaries.push(piece.start);
                    }
                    if piece.flags & 0b100 == 0 {
                        boundaries.push(piece.end);
                    }
                    collect(piece.descriptor_index, descriptors, seen, boundaries)?;
                }
            }
        }
        Ok(())
    }

    let mut seen = BTreeSet::new();
    let mut boundaries = vec![integration_origin];
    collect(speed_descriptor, descriptors, &mut seen, &mut boundaries)?;
    collect(tempo_descriptor, descriptors, &mut seen, &mut boundaries)?;
    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|left, right| left.to_bits() == right.to_bits());
    Ok(boundaries)
}

fn take_record<'a>(outer: &mut Cursor<'a>) -> Result<Cursor<'a>, &'static str> {
    let byte_length = outer.u32()? as usize;
    if byte_length < 8 || !byte_length.is_multiple_of(4) {
        return Err("fcbc.invalid-record");
    }
    if outer.u16()? != 1 || outer.u16()? != 0 {
        return Err("fcbc.invalid-record");
    }
    let payload = outer
        .take(byte_length - 8)
        .map_err(|_| "fcbc.invalid-record")?;
    Ok(Cursor::new(payload, "fcbc.invalid-record"))
}

fn parse_value(cursor: &mut Cursor<'_>, string_count: usize) -> Result<ParsedValue, &'static str> {
    let start = cursor.position;
    let tag = cursor.u8()?;
    if cursor.u8()? != 0 || cursor.u16()? != 0 {
        return Err("fcbc.invalid-record");
    }
    let payload_length = cursor.u32()? as usize;
    let payload = cursor.take(payload_length)?;
    let mut value = Cursor::new(payload, "fcbc.invalid-record");
    let mut parsed = ParsedValue {
        tag,
        string_ref: None,
        fields: Vec::new(),
    };
    match tag {
        0 => {}
        1 => {
            if value.u8()? > 1 {
                return Err("fcbc.invalid-record");
            }
            value.zeroes(7)?;
        }
        2 => {
            value.i64()?;
        }
        3 | 5 | 7 | 8 => {
            value.f64()?;
        }
        4 => {
            let reference = value.u32()?;
            check_string_ref(reference, string_count)?;
            value.zeroes(4)?;
            parsed.string_ref = Some(reference);
        }
        6 => {
            value.i64()?;
            if value.i64()? <= 0 {
                return Err("fcbc.invalid-record");
            }
        }
        9 => {
            for _ in 0..4 {
                value.f64()?;
            }
        }
        10 => {
            let element_tag = value.u8()?;
            value.zeroes(7)?;
            parse_scalar_payload(&mut value, element_tag)?;
            parse_scalar_payload(&mut value, element_tag)?;
        }
        11 | 12 => {
            value.u64()?;
        }
        13 => {
            let element_tag = value.u8()?;
            if element_tag == 0 {
                return Err("fcbc.invalid-record");
            }
            value.zeroes(3)?;
            let count = limited_count(value.u32()?)?;
            for _ in 0..count {
                let element = parse_value(&mut value, string_count)?;
                if element.tag != element_tag {
                    return Err("fcbc.invalid-record");
                }
            }
        }
        14 => {
            let count = limited_count(value.u32()?)?;
            let mut keys = BTreeSet::new();
            for _ in 0..count {
                let key = value.u32()?;
                check_string_ref(key, string_count)?;
                if !keys.insert(key) {
                    return Err("fcbc.invalid-record");
                }
                parsed
                    .fields
                    .push((key, parse_value(&mut value, string_count)?));
            }
        }
        _ => return Err("fcbc.invalid-record"),
    }
    value.finish()?;
    let consumed = cursor.position - start;
    cursor.zeroes((8 - consumed % 8) % 8)?;
    Ok(parsed)
}

fn parse_scalar_payload(cursor: &mut Cursor<'_>, tag: u8) -> Result<(), &'static str> {
    match tag {
        2 => {
            cursor.i64()?;
        }
        3 | 5 | 7 | 8 => {
            cursor.f64()?;
        }
        6 => {
            cursor.i64()?;
            if cursor.i64()? <= 0 {
                return Err("fcbc.invalid-record");
            }
        }
        _ => return Err("fcbc.invalid-record"),
    }
    Ok(())
}

fn parse_bytes(cursor: &mut Cursor<'_>) -> Result<Vec<u8>, &'static str> {
    let length = cursor.u32()? as usize;
    let bytes = cursor.take(length)?.to_vec();
    cursor.zeroes((4 - length % 4) % 4)?;
    Ok(bytes)
}

fn validate_judge_shape(judge_shape: &ParsedValue, strings: &[String]) -> Result<(), &'static str> {
    if judge_shape.tag != 14 {
        return Err("fcbc.invalid-note");
    }
    let kind_key = strings
        .iter()
        .position(|value| value == "kind")
        .ok_or("fcbc.invalid-note")? as u32;
    let kind_value = judge_shape
        .fields
        .iter()
        .find(|(key, _)| *key == kind_key)
        .map(|(_, value)| value)
        .ok_or("fcbc.invalid-note")?;
    if kind_value.tag != 4 {
        return Err("fcbc.invalid-note");
    }
    let kind_ref = kind_value.string_ref.ok_or("fcbc.invalid-note")?;
    match strings.get(kind_ref as usize).map(String::as_str) {
        Some("lineDefault") if judge_shape.fields.len() == 1 => Ok(()),
        // The non-empty reference fixture deliberately uses lineDefault. Other canonical shapes
        // belong in their own vectors rather than being guessed by this fixture-specific oracle.
        _ => Err("fcbc.invalid-note"),
    }
}

fn parse_domain(
    flags: u16,
    start: f64,
    end: f64,
    error: &'static str,
) -> Result<Domain, &'static str> {
    if flags & !0b11 != 0 {
        return Err(error);
    }
    let unbounded_before = flags & 1 != 0;
    let unbounded_after = flags & 2 != 0;
    if (unbounded_before && start.to_bits() != 0) || (unbounded_after && end.to_bits() != 0) {
        return Err(error);
    }
    if !unbounded_before && !start.is_finite() || !unbounded_after && !end.is_finite() {
        return Err(error);
    }
    if !unbounded_before && !unbounded_after && start > end {
        return Err(error);
    }
    Ok(Domain {
        start,
        end,
        unbounded_before,
        unbounded_after,
    })
}

fn check_descriptor_type(
    descriptors: &[PropertyDescriptor],
    index: u32,
    expected: ValueType,
) -> Result<(), &'static str> {
    let descriptor = descriptors
        .get(index as usize)
        .ok_or("fcbc.dangling-reference")?;
    if descriptor.property_type == expected {
        Ok(())
    } else {
        Err("fcbc.invalid-track")
    }
}

fn check_constant_type(
    constants: &[RuntimeValue],
    index: u32,
    expected: ValueType,
) -> Result<(), &'static str> {
    let value = constants
        .get(index as usize)
        .ok_or("fcbc.dangling-reference")?;
    if runtime_value_type(value) == Some(expected) {
        Ok(())
    } else {
        Err("fcbc.invalid-track")
    }
}

fn descriptor_constant_scalar(
    descriptor: &PropertyDescriptor,
    constants: &[RuntimeValue],
) -> Result<f64, &'static str> {
    let DescriptorKind::Constant(index) = &descriptor.kind else {
        return Err("fcbc.invalid-distance");
    };
    match constants.get(*index as usize) {
        Some(RuntimeValue::Scalar { value, .. }) => Ok(*value),
        _ => Err("fcbc.invalid-distance"),
    }
}

fn require_unbounded_root(
    descriptors: &[PropertyDescriptor],
    index: u32,
    error: &'static str,
) -> Result<(), &'static str> {
    let descriptor = descriptors
        .get(index as usize)
        .ok_or("fcbc.dangling-reference")?;
    if descriptor.domain.unbounded_before
        && descriptor.domain.unbounded_after
        && descriptor.domain.start.to_bits() == 0
        && descriptor.domain.end.to_bits() == 0
    {
        Ok(())
    } else {
        Err(error)
    }
}

fn runtime_value_type(value: &RuntimeValue) -> Option<ValueType> {
    match value {
        RuntimeValue::Bool(_) => Some(ValueType::Bool),
        RuntimeValue::Int(_) => Some(ValueType::Int),
        RuntimeValue::Scalar { ty, .. } => Some(*ty),
        RuntimeValue::Color(_) => Some(ValueType::Color),
        RuntimeValue::Vec2 { ty, .. } => Some(*ty),
        RuntimeValue::ResourceRef(_) | RuntimeValue::ContributorRef(_) => None,
    }
}

fn scalar_tag_type(tag: u8) -> Option<ValueType> {
    match tag {
        2 => Some(ValueType::Int),
        3 => Some(ValueType::Float),
        5 => Some(ValueType::Time),
        6 => Some(ValueType::Beat),
        7 => Some(ValueType::Length),
        8 => Some(ValueType::Angle),
        _ => None,
    }
}

fn optional_index(value: u32) -> Option<u32> {
    (value != NULL_INDEX).then_some(value)
}

fn check_string_ref(reference: u32, string_count: usize) -> Result<(), &'static str> {
    if reference as usize >= string_count {
        Err("fcbc.dangling-reference")
    } else {
        Ok(())
    }
}

fn limited_count(value: u32) -> Result<usize, &'static str> {
    if value > 1_000_000 {
        Err("fcbc.limit-exceeded")
    } else {
        Ok(value as usize)
    }
}

fn align_up_u64(value: u64, alignment: u64) -> Option<u64> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
}

fn align_up_usize(value: usize, alignment: usize) -> Option<usize> {
    value
        .checked_add(alignment - 1)
        .map(|sum| sum & !(alignment - 1))
}

fn crc32_iso_hdlc(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}
