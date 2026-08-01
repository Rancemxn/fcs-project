use super::*;

mod custom_value_depth_tests {
    use super::*;

    /// Wraps `payload` in `levels` nested single-element arrays of tag 13.
    ///
    /// Each level costs 12 bytes, which is what makes an unbounded parser cheap
    /// to attack: a few kilobytes reach a depth no native stack survives.
    fn nest(levels: usize, innermost: Vec<u8>) -> Vec<u8> {
        let mut value = innermost;
        for _ in 0..levels {
            let mut level = Vec::new();
            level.push(13u8); // tag: array
            level.push(0);
            level.extend_from_slice(&0u16.to_le_bytes());
            let payload_length = 8 + value.len();
            level.extend_from_slice(&(payload_length as u32).to_le_bytes());
            level.push(13u8); // element tag, must equal the element's own tag
            level.extend_from_slice(&[0, 0, 0]);
            level.extend_from_slice(&1u32.to_le_bytes()); // one element
            level.extend_from_slice(&value);
            value = level;
        }
        value
    }

    /// The innermost value is an empty array, so it terminates the nesting.
    fn empty_array() -> Vec<u8> {
        let mut value = vec![13u8, 0];
        value.extend_from_slice(&0u16.to_le_bytes());
        value.extend_from_slice(&8u32.to_le_bytes());
        value.push(13u8);
        value.extend_from_slice(&[0, 0, 0]);
        value.extend_from_slice(&0u32.to_le_bytes());
        value
    }

    fn parse(bytes: &[u8]) -> Result<ParsedValue, &'static str> {
        let mut cursor = Cursor::new(bytes, "fcbc.invalid-record");
        parse_value(&mut cursor, 0)
    }

    #[test]
    fn nesting_within_the_limit_is_accepted() {
        let bytes = nest(MAX_CUSTOM_VALUE_DEPTH - 1, empty_array());
        assert!(parse(&bytes).is_ok());
    }

    #[test]
    fn nesting_beyond_the_limit_is_a_resource_error_not_a_crash() {
        let bytes = nest(MAX_CUSTOM_VALUE_DEPTH + 1, empty_array());
        assert_eq!(parse(&bytes).unwrap_err(), "fcbc.limit-exceeded");
    }

    #[test]
    fn a_hostile_depth_returns_rather_than_exhausting_the_stack() {
        // Without a depth guard this input aborts the process instead of
        // returning; the payload is only about 24 kB.
        let bytes = nest(2_000, empty_array());
        assert_eq!(parse(&bytes).unwrap_err(), "fcbc.limit-exceeded");
    }
}

mod validator_recursion_tests {
    use super::*;

    /// Longer than any native stack survives under the former recursive
    /// validators, while each record is fixed-size, so building the chain
    /// stays cheap for a hostile container.
    const HOSTILE_CHAIN_LEN: usize = 2_000;

    fn domain() -> Domain {
        Domain {
            start: 0.0,
            end: 1.0,
            unbounded_before: false,
            unbounded_after: false,
        }
    }

    fn descriptor(kind: DescriptorKind) -> PropertyDescriptor {
        PropertyDescriptor {
            property_type: ValueType::Float,
            domain: domain(),
            kind,
        }
    }

    fn piecewise_to(target: u32) -> PropertyDescriptor {
        descriptor(DescriptorKind::Piecewise(vec![Piece {
            start: 0.0,
            end: 1.0,
            descriptor_index: target,
            flags: 0,
        }]))
    }

    /// `descriptors[0]` is a Constant leaf and `descriptors[i]` chains to
    /// `i - 1`, so index order is a valid canonical order.
    fn backward_descriptor_chain(length: usize) -> Vec<PropertyDescriptor> {
        let mut descriptors = vec![descriptor(DescriptorKind::Constant(0))];
        for index in 1..length {
            descriptors.push(piecewise_to(index as u32 - 1));
        }
        descriptors
    }

    /// `descriptors[i]` chains to `i + 1`; the walk from index 0 descends the
    /// whole table before reaching the Constant leaf at the end.
    fn forward_descriptor_chain(length: usize) -> Vec<PropertyDescriptor> {
        let mut descriptors: Vec<PropertyDescriptor> = (1..length)
            .map(|index| piecewise_to(index as u32))
            .collect();
        descriptors.push(descriptor(DescriptorKind::Constant(0)));
        descriptors
    }

    fn node(opcode: u16, operands: [u32; 3], arity: u8) -> ExpressionNode {
        ExpressionNode {
            opcode,
            result_type: ValueType::Time,
            operands,
            arity,
            immediate: 0,
        }
    }

    /// `expressions[0]` is an ENV S leaf and `expressions[i]` wraps `i - 1`,
    /// matching the operand-index ordering the loader enforces.
    fn backward_expression_chain(length: usize, leaf_opcode: u16) -> Vec<ExpressionNode> {
        let mut expressions = vec![node(leaf_opcode, [NULL_INDEX; 3], 0)];
        for index in 1..length {
            expressions.push(node(10, [index as u32 - 1, NULL_INDEX, NULL_INDEX], 1));
        }
        expressions
    }

    #[test]
    fn deep_acyclic_piecewise_chain_is_accepted() {
        let descriptors = forward_descriptor_chain(HOSTILE_CHAIN_LEN);
        assert_eq!(validate_piecewise_acyclic(&descriptors), Ok(()));
    }

    #[test]
    fn cycle_at_the_end_of_a_deep_piecewise_chain_is_still_rejected() {
        let mut descriptors = forward_descriptor_chain(HOSTILE_CHAIN_LEN);
        let last = descriptors.len() as u32 - 1;
        *descriptors.last_mut().unwrap() = piecewise_to(last);
        assert_eq!(
            validate_piecewise_acyclic(&descriptors),
            Err("fcbc.invalid-track")
        );
    }

    #[test]
    fn deep_descriptor_chain_environment_dependencies_is_limited() {
        let descriptors = backward_descriptor_chain(HOSTILE_CHAIN_LEN);
        let root = descriptors.len() as u32 - 1;
        assert_eq!(
            descriptor_environment_dependencies(root, &descriptors, &[], 0),
            Err("fcbc.limit-exceeded")
        );
    }

    #[test]
    fn deep_expression_chain_environment_dependencies_is_limited() {
        let expressions = backward_expression_chain(HOSTILE_CHAIN_LEN, 2);
        let root = expressions.len() as u32 - 1;
        assert_eq!(
            expression_environment_dependencies(root, &expressions, 0),
            Err("fcbc.limit-exceeded")
        );
    }

    #[test]
    fn deep_descriptor_chain_env_p_context_is_limited() {
        let descriptors = backward_descriptor_chain(HOSTILE_CHAIN_LEN);
        let root = descriptors.len() as u32 - 1;
        assert_eq!(
            validate_descriptor_env_p_context(root, &descriptors, &[]),
            Err("fcbc.limit-exceeded")
        );
    }

    #[test]
    fn env_p_at_the_end_of_a_deep_expression_chain_is_still_rejected() {
        // The ENV P leaf sits below a piece-less Expression root, so the
        // context rule must reject it even at hostile depth.
        let expressions = backward_expression_chain(MAX_VALIDATOR_DEPTH - 2, 6);
        let root = expressions.len() as u32 - 1;
        let descriptors = vec![descriptor(DescriptorKind::Expression(root))];
        assert_eq!(
            validate_descriptor_env_p_context(0, &descriptors, &expressions),
            Err("fcbc.invalid-expression")
        );
    }

    #[test]
    fn deep_descriptor_chain_reachability_walk_terminates() {
        let descriptors = backward_descriptor_chain(HOSTILE_CHAIN_LEN);
        let root = descriptors.len() as u32 - 1;
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        assert_eq!(
            reachability_visit_descriptor(root, &descriptors, &mut visited, &mut order),
            Ok(())
        );
        assert_eq!(order.len(), descriptors.len());
    }

    #[test]
    fn deep_expression_chain_reachability_walk_terminates() {
        let expressions = backward_expression_chain(HOSTILE_CHAIN_LEN, 2);
        let root = expressions.len() as u32 - 1;
        let mut visited = BTreeSet::new();
        let mut order = Vec::new();
        assert_eq!(
            reachability_visit_node(root, &expressions, &mut visited, &mut order),
            Ok(())
        );
        assert_eq!(order.len(), expressions.len());
    }
}

mod tempo_revalidation_tests {
    use super::parse_tempo;

    /// Encodes a TempoMap section payload: `count:u32` then the section 10 points as
    /// `beatNumerator, beatDenominator, chartTime, bpm, sourceOrder, reserved=0`.
    fn tempo_section(points: &[(i64, i64, f64, f64, u32)]) -> Vec<u8> {
        let mut bytes = (points.len() as u32).to_le_bytes().to_vec();
        for (numerator, denominator, chart_time, bpm, source_order) in points {
            bytes.extend_from_slice(&numerator.to_le_bytes());
            bytes.extend_from_slice(&denominator.to_le_bytes());
            bytes.extend_from_slice(&chart_time.to_bits().to_le_bytes());
            bytes.extend_from_slice(&bpm.to_bits().to_le_bytes());
            bytes.extend_from_slice(&source_order.to_le_bytes());
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn adjacent_mapping_within_two_ulp_is_accepted() {
        // 3/2 beat at 120bpm is 0.75s exactly, and ULP(0.75) is one step of the
        // binade, so two steps in either direction is the tolerance boundary.
        for chart_time in [
            0.75,
            0.75f64.next_up().next_up(),
            0.75f64.next_down().next_down(),
        ] {
            let bytes = tempo_section(&[(0, 1, 0.0, 120.0, 0), (3, 2, chart_time, 240.0, 1)]);
            assert!(parse_tempo(&bytes).is_ok(), "{chart_time} is within 2 ULP");
        }
    }

    #[test]
    fn overflowing_tempo_denominator_uses_a_scaled_reference() {
        let chart_time = (1.0 / 2.0) * 60.0 / f64::MAX;
        let bytes = tempo_section(&[(0, 1, 0.0, f64::MAX, 0), (1, 2, chart_time, 120.0, 1)]);
        assert!(parse_tempo(&bytes).is_ok());
    }

    #[test]
    fn adjacent_mapping_beyond_two_ulp_is_rejected() {
        let chart_time = 0.75f64.next_up().next_up().next_up();
        let bytes = tempo_section(&[(0, 1, 0.0, 120.0, 0), (3, 2, chart_time, 240.0, 1)]);
        assert_eq!(parse_tempo(&bytes).unwrap_err(), "fcbc.invalid-tempo");
    }

    #[test]
    fn a_floored_exact_beat_is_rejected() {
        // The writer defect this guards: the 3/2 beat is emitted as 1/1 while the
        // chartTime still denotes 3/2, so the stored mapping is a quarter second off.
        let bytes = tempo_section(&[(0, 1, 0.0, 120.0, 0), (1, 1, 0.75, 240.0, 1)]);
        assert_eq!(parse_tempo(&bytes).unwrap_err(), "fcbc.invalid-tempo");
    }

    #[test]
    fn a_same_beat_step_must_repeat_the_chart_time_bitwise() {
        let step = |chart_time: f64| {
            tempo_section(&[
                (0, 1, 0.0, 120.0, 0),
                (1, 1, 0.5, 120.0, 1),
                (1, 1, chart_time, 240.0, 2),
            ])
        };
        assert!(parse_tempo(&step(0.5)).is_ok());
        assert_eq!(
            parse_tempo(&step(0.5f64.next_up())).unwrap_err(),
            "fcbc.invalid-tempo"
        );
    }

    #[test]
    fn a_same_beat_step_rejects_the_opposite_signed_zero() {
        // Bitwise, not numeric: -0.0 == 0.0 but the stored bits differ.
        let bytes = tempo_section(&[(0, 1, 0.0, 120.0, 0), (0, 1, -0.0, 240.0, 1)]);
        assert_eq!(parse_tempo(&bytes).unwrap_err(), "fcbc.invalid-tempo");
    }

    #[test]
    fn the_first_point_must_carry_the_canonical_time_of_beat_zero() {
        let bytes = tempo_section(&[(0, 1, 1.0, 120.0, 0), (1, 1, 1.5, 120.0, 1)]);
        assert_eq!(parse_tempo(&bytes).unwrap_err(), "fcbc.invalid-tempo");
    }
}

mod extension_tests {
    use super::*;

    fn extension_record(
        namespace: u32,
        version: (u16, u16, u16),
        flags: u16,
        value_tag: u8,
        tail: &[u8],
    ) -> Vec<u8> {
        let value_payload = if value_tag == 14 {
            0u32.to_le_bytes().to_vec()
        } else {
            Vec::new()
        };
        let mut payload = Vec::new();
        payload.extend_from_slice(&namespace.to_le_bytes());
        payload.extend_from_slice(&version.0.to_le_bytes());
        payload.extend_from_slice(&version.1.to_le_bytes());
        payload.extend_from_slice(&version.2.to_le_bytes());
        payload.extend_from_slice(&flags.to_le_bytes());
        payload.extend_from_slice(&[value_tag, 0, 0, 0]);
        payload.extend_from_slice(&(value_payload.len() as u32).to_le_bytes());
        payload.extend_from_slice(&value_payload);
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(tail);

        let mut record = Vec::new();
        record.extend_from_slice(&((payload.len() + 8) as u32).to_le_bytes());
        record.extend_from_slice(&1u16.to_le_bytes());
        record.extend_from_slice(&0u16.to_le_bytes());
        record.extend_from_slice(&payload);
        record
    }

    fn extension_section(records: &[Vec<u8>]) -> Vec<u8> {
        let mut section = Vec::new();
        section.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for record in records {
            section.extend_from_slice(record);
        }
        section
    }

    #[test]
    fn extension_record_tail_is_skipped_within_its_boundary() {
        let record = extension_record(0, (1, 2, 3), 1, 14, &[0xaa, 0xbb, 0xcc, 0xdd]);
        let extensions = parse_extensions(&extension_section(&[record]), &["score.ext".into()])
            .expect("extension record tail must be skippable");
        assert_eq!(
            extensions,
            vec![ExtensionRecord {
                namespace: "score.ext".into(),
                version: (1, 2, 3),
                flags: 1,
            }]
        );
    }

    #[test]
    fn extension_validation_uses_the_stable_record_category() {
        let strings = vec!["score.ext".into()];
        let invalid_flags = extension_record(0, (1, 2, 3), 4, 14, &[]);
        assert_eq!(
            parse_extensions(&extension_section(&[invalid_flags]), &strings),
            Err("fcbc.invalid-record")
        );

        let invalid_payload = extension_record(0, (1, 2, 3), 1, 0, &[]);
        assert_eq!(
            parse_extensions(&extension_section(&[invalid_payload]), &strings),
            Err("fcbc.invalid-record")
        );

        let duplicate = extension_record(0, (1, 2, 3), 1, 14, &[]);
        assert_eq!(
            parse_extensions(
                &extension_section(&[duplicate.clone(), duplicate]),
                &strings
            ),
            Err("fcbc.invalid-record")
        );
    }
}
