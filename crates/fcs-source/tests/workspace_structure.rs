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
        "crates/fcs-converter",
        "crates/fcs-source/src/v4",
        "crates/fcs-source/src/v5",
    ] {
        assert!(
            !repository.join(removed).exists(),
            "legacy path remains active: {removed}"
        );
    }
    // I10 product CLI is an intentional unversioned binary crate.
    assert!(
        repository.join("crates/fcs-cli").exists(),
        "product CLI crate must exist for I10"
    );
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

#[test]
fn expression_parser_uses_chumsky_stacker_without_a_fixed_thread() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parser = fs::read_to_string(crate_dir.join("src/parser/expression.rs"))
        .expect("expression parser source must be readable");

    for forbidden in ["std::thread::Builder", "spawn_scoped", ".stack_size("] {
        assert!(
            !parser.contains(forbidden),
            "fixed parser-thread mechanism remains: {forbidden}"
        );
    }
}

#[test]
fn document_parser_uses_bounded_recovery_and_consumes_trailing_input() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parser = fs::read_to_string(crate_dir.join("src/parser/document.rs"))
        .expect("document parser source must be readable");

    assert!(
        parser.contains("skip_then_retry_until"),
        "document parser must use parser-owned Chumsky recovery"
    );
    assert!(
        parser.contains("nested_delimiters"),
        "document recovery must skip balanced groups before one-token fallback"
    );
    assert!(
        parser.contains("then_ignore(end())"),
        "document parser must consume the complete token stream"
    );
    for forbidden in [
        "source.find(",
        "source.as_bytes()",
        "fn scan_top_level",
        "fn rescan_diagnostic",
    ] {
        assert!(
            !parser.contains(forbidden),
            "document parser must not add a raw-source recovery path: {forbidden}"
        );
    }
}
