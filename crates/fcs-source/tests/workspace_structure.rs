use std::{fs, path::Path};

#[test]
fn workspace_has_one_unversioned_source_implementation() {
    assert_eq!(env!("CARGO_PKG_NAME"), "fcs-source");

    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repository = crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate must live under <repo>/crates/<name>");

    for removed in [
        "crates/fcs-core",
        "crates/fcs-cli",
        "crates/fcs-converter",
        "crates/fcs-source/src/v4",
        "crates/fcs-source/src/v5",
    ] {
        assert!(
            !repository.join(removed).exists(),
            "legacy path remains active: {removed}"
        );
    }
}

#[test]
fn lexer_has_no_raw_text_preparser() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lexer = fs::read_to_string(crate_dir.join("src/parser/lexer.rs"))
        .expect("lexer source must be readable");

    for forbidden in [
        "fn header_prefix",
        "fn validate_trivia",
        "fn malformed_exponent_end",
        "fn color_candidate_end",
    ] {
        assert!(
            !lexer.contains(forbidden),
            "raw-text lexer helper remains: {forbidden}"
        );
    }
    assert!(
        lexer.contains("parse_with_state"),
        "Chumsky lexer must enforce recursive limits through parser state"
    );
}
