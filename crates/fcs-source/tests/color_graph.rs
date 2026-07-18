use fcs_source::parser::parse_document;

#[test]
fn canonical_metadata_colors_store_linear_rgb_and_normalized_alpha() {
    let document = parse_document(
        r#"#fcs 5.0.0
format { profile: fragment; }
meta { custom: { "color": #80808080 }; }
"#,
    )
    .into_result()
    .unwrap();
    let metadata = document.canonical_metadata().unwrap();
    let value = metadata
        .meta()
        .unwrap()
        .get("custom")
        .and_then(|value| match value {
            fcs_model::CanonicalValue::Object(object) => object.get("color"),
            _ => None,
        })
        .unwrap();
    let fcs_model::CanonicalValue::Color(color) = value else {
        panic!("expected canonical color")
    };
    assert!((color.red() - 0.21586050011389926).abs() < 1e-15);
    assert!((color.green() - 0.21586050011389926).abs() < 1e-15);
    assert!((color.blue() - 0.21586050011389926).abs() < 1e-15);
    assert_eq!(color.alpha(), 128.0 / 255.0);
}
