//! Product FCBC 2.0 container framing, Core section load, and execution queries.
//!
//! I7.1–I7.2 own little-endian encode/decode helpers, CRC-32/ISO-HDLC section
//! checksums, the 128-byte header, and 40-byte section table validation.
//! I7.3 owns product Core section decode via [`load_chart`]. Descriptor
//! evaluation queries for Execution ABI closure are also product surfaces.

mod codec;
mod container;
mod error;
mod evaluator;
mod loader;
mod writer;

pub use codec::{
    decode_f64_le, decode_i64_le, decode_u8, decode_u16_le, decode_u32_le, decode_u64_le,
    encode_f64_le, encode_i64_le, encode_u8, encode_u16_le, encode_u32_le, encode_u64_le,
    section_crc32_iso_hdlc,
};
pub use container::{
    CONTAINER_HEADER_SIZE, ContainerHeader, ContainerProfile, FeatureFlags, MAGIC,
    SECTION_ENTRY_SIZE, SectionEntry, ValidatedContainer, load_container,
    load_container_with_identity,
};
pub use error::{FcbcError, FcbcResult};
pub use evaluator::{
    DescriptorEvaluation, DistanceEvaluation, EvaluationEnvironment, query_descriptor,
    query_distance, query_scroll_coordinate,
};
pub use loader::{
    DecodedChart, DescriptorKind, DistanceClassification, DistanceDescriptor, Domain,
    ExpressionNode, LineRecord, NULL_INDEX, NoteRecord, PropertyDescriptor, RuntimeValue,
    SectionInfo, Segment, TempoPoint, ValueType, load, load_chart,
    validate_descriptor_env_p_context, validate_descriptor_environment_for_target,
};
pub use writer::write_nonempty_execution;
