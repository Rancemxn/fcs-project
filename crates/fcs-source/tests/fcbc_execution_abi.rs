use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

#[path = "support/fcbc_reference_evaluator.rs"]
mod fcbc_reference_evaluator;
#[path = "support/fcbc_reference_loader.rs"]
mod fcbc_reference_loader;
#[path = "support/fcbc_reference_writer.rs"]
mod fcbc_reference_writer;

use fcbc_reference_evaluator::{
    EvaluationEnvironment, query_descriptor, query_distance, query_scroll_coordinate,
};
use fcbc_reference_loader::{
    DescriptorKind, DistanceClassification, Domain, ExpressionNode, Piece, PropertyDescriptor,
    RuntimeValue, ValueType, load, validate_descriptor_env_p_context,
    validate_descriptor_environment_for_target,
};

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteManifest {
    schema_version: u32,
    fcbc_version: String,
    execution_abi_version: String,
    fixture: Vec<SuiteFixture>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteFixture {
    id: String,
    manifest: String,
    mutations: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenManifest {
    schema_version: u32,
    id: String,
    fcbc_version: String,
    execution_abi_version: String,
    source_fcs_version: String,
    container_profile: String,
    document_profile: String,
    chart_count: u32,
    resource_count: usize,
    exact_descriptors_only: bool,
    expect: String,
    path: String,
    decoded_length: u64,
    sha256: String,
    execution: ExecutionExpectation,
    #[serde(default)]
    resource: Vec<toml::Value>,
    section: Vec<SectionExpectation>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionExpectation {
    vector: String,
    constant_count: usize,
    descriptor_count: usize,
    expression_node_count: usize,
    distance_count: usize,
    line_count: usize,
    note_count: usize,
    descriptor_kinds: Vec<String>,
    distance_classifications: Vec<String>,
    lazy_opcodes: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SectionExpectation {
    r#type: u32,
    name: String,
    offset: u64,
    length: u64,
    crc32: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionVector {
    schema_version: u32,
    id: String,
    descriptor_query: Vec<DescriptorQuery>,
    distance_query: Vec<DistanceQuery>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DescriptorQuery {
    id: String,
    descriptor_index: u32,
    time_bits: String,
    s_bits: String,
    b_bits: String,
    q_bits: String,
    d_bits: String,
    p_bits: String,
    expected_type: String,
    expected_bool: Option<bool>,
    expected_int: Option<i64>,
    expected_f64_bits: Option<String>,
    #[serde(default)]
    expected_vec2_bits: Vec<String>,
    expected_trace: Vec<u32>,
    #[serde(default)]
    trace_excludes: Vec<u32>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DistanceQuery {
    id: String,
    distance_index: u32,
    time_bits: String,
    expected_classification: String,
    expected_floor_bits: String,
    max_absolute_error_bits: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationManifest {
    schema_version: u32,
    base: String,
    mutation: Vec<Mutation>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Mutation {
    id: String,
    diagnostic: String,
    patch: Vec<MutationPatch>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MutationPatch {
    offset: u64,
    replace_hex: String,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn suite_base() -> PathBuf {
    repository_root().join("docs/conformance/fcbc")
}

fn load_toml<T: DeserializeOwned>(path: &Path) -> T {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn nonempty_entry(suite: &SuiteManifest) -> &SuiteFixture {
    suite
        .fixture
        .iter()
        .find(|fixture| fixture.id == "nonempty-execution")
        .expect("the S15 ABI blocker requires a bound nonempty-execution fixture")
}

fn load_suite_and_golden() -> (SuiteManifest, GoldenManifest) {
    let base = suite_base();
    let suite: SuiteManifest = load_toml(&base.join("manifest.toml"));
    let entry = nonempty_entry(&suite);
    let golden = load_toml(&base.join(&entry.manifest));
    (suite, golden)
}

fn decode_lower_hex(source: &str, description: &str) -> Vec<u8> {
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(
        !compact.is_empty() && compact.len().is_multiple_of(2),
        "{description} must be nonempty even-length hex"
    );
    assert!(
        compact
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{description} must be lowercase hex"
    );
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digit = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => unreachable!("validated lowercase hex"),
            };
            digit(pair[0]) << 4 | digit(pair[1])
        })
        .collect()
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let source = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    decode_lower_hex(&source, &path.display().to_string())
}

fn encode_hex_lines(bytes: &[u8]) -> String {
    let mut output = String::new();
    for (index, byte) in bytes.iter().enumerate() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
        if index % 48 == 47 || index + 1 == bytes.len() {
            output.push('\n');
        }
    }
    output
}

fn static_golden_or_dump(path: &Path, generated: &[u8]) -> Vec<u8> {
    if path.is_file() {
        return decode_hex_file(path);
    }
    panic!(
        "static golden {} is missing; candidate bytes follow ({} bytes):\n{}",
        path.display(),
        generated.len(),
        encode_hex_lines(generated)
    );
}

fn sha256_lower(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest.finalize() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn bits(source: &str) -> u64 {
    let raw = source.strip_prefix("0x").unwrap_or(source);
    assert_eq!(raw.len(), 16, "binary64 bits must contain 16 hex digits");
    u64::from_str_radix(raw, 16).unwrap_or_else(|error| panic!("invalid bits {source}: {error}"))
}

fn f64_from_bits(source: &str) -> f64 {
    f64::from_bits(bits(source))
}

fn type_name(value_type: ValueType) -> &'static str {
    match value_type {
        ValueType::Bool => "bool",
        ValueType::Int => "int",
        ValueType::Float => "float",
        ValueType::Time => "time",
        ValueType::Beat => "beat",
        ValueType::Length => "length",
        ValueType::Angle => "angle",
        ValueType::Color => "color",
        ValueType::Vec2Float => "vec2-float",
        ValueType::Vec2Length => "vec2-length",
        ValueType::Vec2Int => "vec2-int",
        ValueType::Vec2Time => "vec2-time",
        ValueType::Vec2Beat => "vec2-beat",
        ValueType::Vec2Angle => "vec2-angle",
    }
}

fn descriptor_kind_name(kind: &DescriptorKind) -> &'static str {
    match kind {
        DescriptorKind::Constant(_) => "constant",
        DescriptorKind::SegmentTrack(_) => "segment-track",
        DescriptorKind::Piecewise(_) => "piecewise",
        DescriptorKind::Expression(_) => "expression",
    }
}

fn distance_classification_name(classification: DistanceClassification) -> &'static str {
    match classification {
        DistanceClassification::PortableAnalytic => "portable-analytic",
        DistanceClassification::PortableEvaluable => "portable-evaluable",
    }
}

fn assert_runtime_value(query: &DescriptorQuery, value: &RuntimeValue) {
    assert_eq!(
        type_name(value.value_type()),
        query.expected_type,
        "{}",
        query.id
    );
    match value {
        RuntimeValue::Bool(actual) => {
            assert_eq!(Some(*actual), query.expected_bool, "{}", query.id);
            assert!(query.expected_int.is_none());
            assert!(query.expected_f64_bits.is_none());
            assert!(query.expected_vec2_bits.is_empty());
        }
        RuntimeValue::Int(actual) => {
            assert_eq!(Some(*actual), query.expected_int, "{}", query.id);
            assert!(query.expected_bool.is_none());
            assert!(query.expected_f64_bits.is_none());
            assert!(query.expected_vec2_bits.is_empty());
        }
        RuntimeValue::Scalar { value, .. } => {
            assert_eq!(
                value.to_bits(),
                bits(
                    query
                        .expected_f64_bits
                        .as_deref()
                        .expect("scalar expected bits")
                ),
                "{}",
                query.id
            );
            assert!(query.expected_bool.is_none());
            assert!(query.expected_int.is_none());
            assert!(query.expected_vec2_bits.is_empty());
        }
        RuntimeValue::Vec2 { value, .. } => {
            assert_eq!(query.expected_vec2_bits.len(), 2, "{}", query.id);
            assert_eq!(
                value[0].to_bits(),
                bits(&query.expected_vec2_bits[0]),
                "{}",
                query.id
            );
            assert_eq!(
                value[1].to_bits(),
                bits(&query.expected_vec2_bits[1]),
                "{}",
                query.id
            );
            assert!(query.expected_bool.is_none());
            assert!(query.expected_int.is_none());
            assert!(query.expected_f64_bits.is_none());
        }
        RuntimeValue::Color(_) | RuntimeValue::ResourceRef(_) | RuntimeValue::ContributorRef(_) => {
            panic!(
                "{} uses a vector schema not supported by this execution vector",
                query.id
            )
        }
    }
}

#[test]
fn nonempty_execution_manifest_binds_static_artifacts() {
    let base = suite_base();
    let (suite, golden) = load_suite_and_golden();
    let entry = nonempty_entry(&suite);

    assert_eq!(suite.schema_version, 2);
    assert_eq!(suite.fcbc_version, "2.0.0");
    assert_eq!(suite.execution_abi_version, "1.0.0");
    assert_eq!(golden.schema_version, 2);
    assert_eq!(golden.id, entry.id);
    assert_eq!(golden.fcbc_version, suite.fcbc_version);
    assert_eq!(golden.execution_abi_version, suite.execution_abi_version);
    assert_eq!(golden.source_fcs_version, "5.0.0");
    assert_eq!(golden.container_profile, "strict-runtime");
    assert_eq!(golden.document_profile, "chart");
    assert_eq!(golden.chart_count, 1);
    assert_eq!(golden.resource_count, golden.resource.len());
    assert!(golden.exact_descriptors_only);
    assert_eq!(golden.expect, "success");
    assert!(base.join(&golden.path).is_file());
    assert!(base.join(&golden.execution.vector).is_file());
    assert!(base.join(&entry.mutations).is_file());
    assert_eq!(
        golden.execution.descriptor_kinds,
        ["constant", "segment-track", "piecewise", "expression"]
    );
    assert_eq!(
        golden.execution.distance_classifications,
        ["portable-evaluable", "portable-analytic"]
    );
    assert_eq!(golden.execution.lazy_opcodes, ["and", "or", "choose"]);
    assert_eq!(golden.section.len(), 14);
    for section in &golden.section {
        assert!(!section.name.is_empty());
        assert_eq!(section.offset % 8, 0);
        assert_eq!(section.crc32.len(), 8);
    }
}

#[test]
fn reference_writer_matches_static_golden_byte_for_byte() {
    let generated = fcbc_reference_writer::write_nonempty_execution();
    let path = suite_base().join("nonempty-execution.hex");
    let golden = static_golden_or_dump(&path, &generated);
    assert_eq!(generated, golden);
}

#[test]
fn independent_loader_decodes_nonempty_static_golden() {
    let base = suite_base();
    let (_, golden) = load_suite_and_golden();
    let bytes = decode_hex_file(&base.join(&golden.path));
    assert_eq!(bytes.len() as u64, golden.decoded_length);
    assert_eq!(sha256_lower(&bytes), golden.sha256);

    let chart = load(&bytes).expect("independent loader must accept the static golden");
    assert_eq!(chart.container_profile, 3);
    assert_eq!(chart.document_profile, 2);
    assert_eq!(chart.feature_flags, 0);
    assert_eq!(chart.constants.len(), golden.execution.constant_count);
    assert_eq!(chart.descriptors.len(), golden.execution.descriptor_count);
    assert_eq!(
        chart.expressions.len(),
        golden.execution.expression_node_count
    );
    assert_eq!(chart.distances.len(), golden.execution.distance_count);
    assert_eq!(chart.lines.len(), golden.execution.line_count);
    assert_eq!(chart.notes.len(), golden.execution.note_count);
    assert_eq!(chart.sections.len(), golden.section.len());
    for (actual, expected) in chart.sections.iter().zip(&golden.section) {
        assert_eq!(actual.section_type, expected.r#type, "{}", expected.name);
        assert_eq!(actual.offset, expected.offset, "{}", expected.name);
        assert_eq!(actual.length, expected.length, "{}", expected.name);
        assert_eq!(
            actual.checksum,
            u32::from_str_radix(&expected.crc32, 16).expect("section CRC-32 must be hex"),
            "{}",
            expected.name
        );
    }

    let kinds: BTreeSet<_> = chart
        .descriptors
        .iter()
        .map(|descriptor| descriptor_kind_name(&descriptor.kind))
        .collect();
    assert_eq!(
        kinds,
        golden
            .execution
            .descriptor_kinds
            .iter()
            .map(String::as_str)
            .collect()
    );
    let classifications: Vec<_> = chart
        .distances
        .iter()
        .map(|distance| distance_classification_name(distance.classification))
        .collect();
    assert_eq!(
        classifications,
        golden
            .execution
            .distance_classifications
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
    );
}

#[test]
fn decoded_expression_evaluation_matches_expected_bits_and_lazy_trace() {
    let base = suite_base();
    let (_, golden) = load_suite_and_golden();
    let chart = load(&decode_hex_file(&base.join(&golden.path))).expect("static golden must load");
    let vector: ExecutionVector = load_toml(&base.join(&golden.execution.vector));
    assert_eq!(vector.schema_version, 1);
    assert_eq!(vector.id, golden.id);
    assert!(!vector.descriptor_query.is_empty());

    for query in &vector.descriptor_query {
        let time = f64_from_bits(&query.time_bits);
        let result = query_descriptor(
            &chart,
            query.descriptor_index,
            time,
            EvaluationEnvironment {
                s: f64_from_bits(&query.s_bits),
                b: f64_from_bits(&query.b_bits),
                q: f64_from_bits(&query.q_bits),
                d: f64_from_bits(&query.d_bits),
                p: f64_from_bits(&query.p_bits),
            },
        )
        .unwrap_or_else(|error| panic!("descriptor query {} failed: {error}", query.id));
        assert_runtime_value(query, &result.value);
        assert_eq!(result.visited_nodes, query.expected_trace, "{}", query.id);
        for excluded in &query.trace_excludes {
            assert!(
                !result.visited_nodes.contains(excluded),
                "{} unexpectedly evaluated lazy node {excluded}",
                query.id
            );
        }
    }
}

#[test]
fn decoded_distance_queries_are_direct_seek_and_match_expected_bits() {
    let base = suite_base();
    let (_, golden) = load_suite_and_golden();
    let chart = load(&decode_hex_file(&base.join(&golden.path))).expect("static golden must load");
    let vector: ExecutionVector = load_toml(&base.join(&golden.execution.vector));
    assert!(!vector.distance_query.is_empty());

    let run = |query: &DistanceQuery| {
        let distance = chart
            .distances
            .get(query.distance_index as usize)
            .unwrap_or_else(|| panic!("distance query {} uses an invalid index", query.id));
        assert_eq!(
            distance.max_distance_error.to_bits(),
            bits(&query.max_absolute_error_bits),
            "{}",
            query.id
        );
        let result = query_distance(
            &chart,
            query.distance_index,
            f64_from_bits(&query.time_bits),
        )
        .unwrap_or_else(|error| panic!("distance query {} failed: {error}", query.id));
        assert_eq!(
            distance_classification_name(result.classification),
            query.expected_classification,
            "{}",
            query.id
        );
        let expected = f64_from_bits(&query.expected_floor_bits);
        let tolerance = f64_from_bits(&query.max_absolute_error_bits);
        let error = (result.floor_position - expected).abs();
        assert!(
            error <= tolerance,
            "{}: actual={:?}, expected={expected:?}, error={error:?}, tolerance={tolerance:?}",
            query.id,
            result.floor_position
        );
        result.floor_position.to_bits()
    };

    let first_pass: Vec<_> = vector.distance_query.iter().map(run).collect();
    let second_pass: Vec<_> = vector.distance_query.iter().rev().map(run).collect();
    assert_eq!(
        first_pass,
        second_pass.into_iter().rev().collect::<Vec<_>>(),
        "distance results must not depend on prior query/frame order"
    );
}

#[test]
fn nonempty_execution_mutations_return_stable_categories() {
    let base = suite_base();
    let (suite, golden) = load_suite_and_golden();
    let entry = nonempty_entry(&suite);
    let original = decode_hex_file(&base.join(&golden.path));
    let mutations: MutationManifest = load_toml(&base.join(&entry.mutations));
    assert_eq!(mutations.schema_version, 2);
    assert_eq!(mutations.base, golden.path);
    assert!(!mutations.mutation.is_empty());

    for mutation in &mutations.mutation {
        let mut bytes = original.clone();
        for patch in &mutation.patch {
            let replacement = decode_lower_hex(
                &patch.replace_hex,
                &format!("mutation {} replacement", mutation.id),
            );
            let start = usize::try_from(patch.offset).expect("patch offset must fit usize");
            let end = start + replacement.len();
            assert!(
                end <= bytes.len(),
                "mutation {} patch is out of bounds",
                mutation.id
            );
            bytes[start..end].copy_from_slice(&replacement);
        }
        assert_eq!(
            load(&bytes).expect_err(&format!("mutation {} unexpectedly loaded", mutation.id)),
            mutation.diagnostic,
            "{}",
            mutation.id
        );
    }
}

#[test]
fn piecewise_rebinds_env_p_and_direct_env_p_is_rejected() {
    let mut chart = load(&fcbc_reference_writer::write_nonempty_execution())
        .expect("reviewed Execution fixture");
    let env_p_root = chart.expressions.len() as u32;
    chart.expressions.push(ExpressionNode {
        opcode: 6,
        result_type: ValueType::Float,
        operands: [u32::MAX; 3],
        arity: 0,
        immediate: 0,
    });
    let inner = chart.descriptors.len() as u32;
    chart.descriptors.push(PropertyDescriptor {
        property_type: ValueType::Float,
        domain: Domain {
            start: 0.0,
            end: 4.0,
            unbounded_before: false,
            unbounded_after: false,
        },
        kind: DescriptorKind::Expression(env_p_root),
    });
    let outer = chart.descriptors.len() as u32;
    chart.descriptors.push(PropertyDescriptor {
        property_type: ValueType::Float,
        domain: Domain {
            start: 0.0,
            end: 4.0,
            unbounded_before: false,
            unbounded_after: false,
        },
        kind: DescriptorKind::Piecewise(vec![Piece {
            start: 0.0,
            end: 4.0,
            descriptor_index: inner,
            flags: 1,
        }]),
    });

    assert_eq!(
        validate_descriptor_env_p_context(inner, &chart.descriptors, &chart.expressions),
        Err("fcbc.invalid-expression")
    );
    validate_descriptor_env_p_context(outer, &chart.descriptors, &chart.expressions)
        .expect("EnvP behind Piecewise must be contextual");

    let result = query_descriptor(
        &chart,
        outer,
        1.0,
        EvaluationEnvironment {
            s: 1.0,
            b: 1.0,
            q: 0.0,
            d: 0.0,
            p: 0.875,
        },
    )
    .expect("Piecewise EnvP query");
    assert_eq!(
        result.value,
        RuntimeValue::Scalar {
            ty: ValueType::Float,
            value: 0.25,
        }
    );
    assert_eq!(result.visited_nodes, [env_p_root]);
}

#[test]
fn vec2_int_expression_scales_by_int_operand() {
    let mut chart = load(&fcbc_reference_writer::write_nonempty_execution())
        .expect("reviewed Execution fixture");
    let vector_constant = chart.constants.len() as u32;
    chart.constants.push(RuntimeValue::Vec2 {
        ty: ValueType::Vec2Int,
        value: [1.0, 2.0],
    });
    let factor_constant = chart.constants.len() as u32;
    chart.constants.push(RuntimeValue::Int(3));

    let vector_node = chart.expressions.len() as u32;
    chart.expressions.push(ExpressionNode {
        opcode: 1,
        result_type: ValueType::Vec2Int,
        operands: [u32::MAX; 3],
        arity: 0,
        immediate: vector_constant,
    });
    let factor_node = chart.expressions.len() as u32;
    chart.expressions.push(ExpressionNode {
        opcode: 1,
        result_type: ValueType::Int,
        operands: [u32::MAX; 3],
        arity: 0,
        immediate: factor_constant,
    });

    for operands in [
        [vector_node, factor_node, u32::MAX],
        [factor_node, vector_node, u32::MAX],
    ] {
        let root = chart.expressions.len() as u32;
        chart.expressions.push(ExpressionNode {
            opcode: 22,
            result_type: ValueType::Vec2Int,
            operands,
            arity: 2,
            immediate: 0,
        });
        let descriptor = chart.descriptors.len() as u32;
        chart.descriptors.push(PropertyDescriptor {
            property_type: ValueType::Vec2Int,
            domain: Domain {
                start: 0.0,
                end: 0.0,
                unbounded_before: true,
                unbounded_after: true,
            },
            kind: DescriptorKind::Expression(root),
        });

        assert_eq!(
            query_descriptor(&chart, descriptor, 0.0, EvaluationEnvironment::at_time(0.0))
                .unwrap()
                .value,
            RuntimeValue::Vec2 {
                ty: ValueType::Vec2Int,
                value: [3.0, 6.0],
            }
        );
    }
}

#[test]
fn line_scroll_coordinate_is_direct_seek_and_anchored_at_zero() {
    let chart = load(&fcbc_reference_writer::write_nonempty_execution())
        .expect("reviewed Execution fixture");
    for line in &chart.lines {
        assert_eq!(
            query_scroll_coordinate(&chart, line.scroll_tempo_descriptor, 0.0)
                .expect("q at anchor")
                .to_bits(),
            0.0f64.to_bits()
        );
        assert_eq!(
            query_scroll_coordinate(&chart, line.scroll_tempo_descriptor, 2.0)
                .expect("q after anchor")
                .to_bits(),
            2.0f64.to_bits()
        );
        assert_eq!(
            query_scroll_coordinate(&chart, line.scroll_tempo_descriptor, -1.0)
                .expect("q before anchor")
                .to_bits(),
            (-1.0f64).to_bits()
        );
    }
}

#[test]
fn scroll_tempo_rejects_env_q_while_scroll_speed_accepts_it() {
    let descriptors = [PropertyDescriptor {
        property_type: ValueType::Float,
        domain: Domain {
            start: 0.0,
            end: 0.0,
            unbounded_before: true,
            unbounded_after: true,
        },
        kind: DescriptorKind::Expression(0),
    }];
    let expressions = [ExpressionNode {
        opcode: 4,
        result_type: ValueType::Float,
        operands: [u32::MAX; 3],
        arity: 0,
        immediate: 0,
    }];

    assert_eq!(
        validate_descriptor_environment_for_target(
            "line.scrollTempo",
            0,
            &descriptors,
            &expressions,
        ),
        Err("fcbc.invalid-expression")
    );
    validate_descriptor_environment_for_target("line.scrollSpeed", 0, &descriptors, &expressions)
        .expect("line.scrollSpeed may depend on q");
}
