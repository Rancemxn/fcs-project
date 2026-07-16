use fcs_source::{
    ast::{SourceSpan, SourceTypeKind, Type},
    parser::parse_type,
};

#[test]
fn source_types_preserve_nested_generic_spans_and_static_shape() {
    let source = "Track<array<vec2<bool>>>";
    let source_type = parse_type(source).into_result().unwrap();

    assert_eq!(source_type.span(), SourceSpan::new(0, 24));
    assert_eq!(
        source_type.to_type(),
        Type::Track(Box::new(Type::Array(Box::new(Type::Vec2(Box::new(
            Type::Bool
        )),))))
    );
    assert!(!source_type.is_constructible());

    let SourceTypeKind::Track(array) = source_type.kind() else {
        panic!("expected Track source type");
    };
    assert_eq!(array.span(), SourceSpan::new(6, 23));
    let SourceTypeKind::Array(vector) = array.kind() else {
        panic!("expected array source type");
    };
    assert_eq!(vector.span(), SourceSpan::new(12, 22));
    let SourceTypeKind::Vec2(element) = vector.kind() else {
        panic!("expected vec2 source type");
    };
    assert_eq!(element.span(), SourceSpan::new(17, 21));
    assert!(matches!(element.kind(), SourceTypeKind::Bool));

    let constructible = parse_type("TrackSegment<array<int>>")
        .into_result()
        .unwrap();
    assert!(constructible.is_constructible());

    let statically_invalid = parse_type("array<Note>").into_result().unwrap();
    assert_eq!(
        statically_invalid.to_type(),
        Type::Array(Box::new(Type::Note))
    );
    assert!(!statically_invalid.is_constructible());
}
