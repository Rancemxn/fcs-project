use std::fmt::Write as _;
use std::fs;

use fcs_model::CanonicalValue;
use fcs_source::ResourceLimits;
use fcs_source::diagnostic::DiagnosticCode;
use fcs_source::parser::parse_document;
use sha2::{Digest, Sha256};
use tempfile::tempdir;

fn parse(source: &str) -> fcs_source::ast::Document {
    parse_document(source)
        .into_result()
        .unwrap_or_else(|diagnostics| panic!("source must parse: {diagnostics:?}"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn one_binary(path: &str, hash: Option<&str>) -> String {
    let hash = hash.map_or_else(String::new, |hash| format!("hash: \"sha256:{hash}\";"));
    format!(
        r#"#fcs 5.0.0
format {{ profile: fragment; }}
resources {{
    binary payload {{
        source: "{path}";
        {hash}
        mediaType: "application/octet-stream";
    }}
}}
"#
    )
}

#[test]
fn builds_deterministic_opaque_bundle_without_path_or_content_deduplication() {
    let workspace = tempdir().expect("temporary workspace");
    fs::create_dir_all(workspace.path().join("nested")).expect("nested fixture directory");
    let opaque = b"not a png, texture, or font\0\xff";
    fs::write(workspace.path().join("nested/opaque.bin"), opaque).expect("fixture payload");
    fs::write(workspace.path().join("unused.bin"), b"unused").expect("unused payload");

    let document = parse(
        r#"#fcs 5.0.0
format { profile: fragment; }
resources {
    texture z_texture {
        source: "nested/opaque.bin";
        mediaType: "image/png";
        colorSpace: "linear-srgb";
        alpha: "premultiplied";
        sampling: "nearest";
    }
    image a_image { source: "nested/opaque.bin"; mediaType: "image/png"; }
    font m_font { source: "nested/opaque.bin"; mediaType: "font/ttf"; }
    binary unused { source: "unused.bin"; mediaType: "application/octet-stream"; }
}
"#,
    );
    let bundle = document
        .canonical_resource_bundle(workspace.path(), ResourceLimits::default())
        .unwrap_or_else(|diagnostics| panic!("bundle must resolve: {diagnostics:?}"));

    assert_eq!(bundle.len(), 4);
    assert_eq!(
        bundle
            .resources()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["a_image", "m_font", "unused", "z_texture"]
    );
    let debug = format!("{bundle:?}");
    assert!(!debug.contains("nested/opaque.bin"));
    assert!(!debug.contains(&workspace.path().display().to_string()));
    for id in ["a_image", "m_font", "z_texture"] {
        let resource = bundle.get(id).expect("opaque resource retained");
        assert_eq!(resource.bytes(), opaque);
        let mut expected = [0; 32];
        expected.copy_from_slice(&Sha256::digest(opaque));
        assert_eq!(resource.content_sha256().as_bytes(), expected);
    }
    assert_eq!(
        bundle
            .get("unused")
            .expect("unused declaration retained")
            .bytes(),
        b"unused"
    );

    let image = bundle.get("a_image").expect("image").resource();
    assert_eq!(
        image
            .metadata()
            .entries()
            .iter()
            .map(|entry| entry.key())
            .collect::<Vec<_>>(),
        ["colorSpace", "alpha", "sampling"]
    );
    assert_eq!(
        image.metadata().get("colorSpace"),
        Some(&CanonicalValue::String("srgb".into()))
    );
    assert_eq!(
        image.metadata().get("alpha"),
        Some(&CanonicalValue::String("straight".into()))
    );
    assert_eq!(
        image.metadata().get("sampling"),
        Some(&CanonicalValue::String("linear".into()))
    );

    let texture = bundle.get("z_texture").expect("texture").resource();
    assert_eq!(
        texture
            .metadata()
            .entries()
            .iter()
            .map(|entry| entry.key())
            .collect::<Vec<_>>(),
        ["colorSpace", "alpha", "sampling"]
    );
    assert_eq!(
        texture.metadata().get("colorSpace"),
        Some(&CanonicalValue::String("linear-srgb".into()))
    );
    assert_eq!(
        texture.metadata().get("alpha"),
        Some(&CanonicalValue::String("premultiplied".into()))
    );
    assert_eq!(
        texture.metadata().get("sampling"),
        Some(&CanonicalValue::String("nearest".into()))
    );

    let font = bundle.get("m_font").expect("font").resource();
    assert_eq!(
        font.metadata()
            .entries()
            .iter()
            .map(|entry| entry.key())
            .collect::<Vec<_>>(),
        ["fontProfile", "shapingProfile", "faceCount"]
    );
    assert_eq!(
        font.metadata().get("fontProfile"),
        Some(&CanonicalValue::String("truetype-glyf-1".into()))
    );
    assert_eq!(
        font.metadata().get("faceCount"),
        Some(&CanonicalValue::Int(1))
    );
}

#[test]
fn verifies_declared_sha256_only_after_reading_safe_exact_bytes() {
    let workspace = tempdir().expect("temporary workspace");
    let bytes = b"opaque exact bytes";
    fs::write(workspace.path().join("payload.bin"), bytes).expect("fixture payload");
    let matching = parse(&one_binary("payload.bin", Some(&sha256_hex(bytes))));
    let bundle = matching
        .canonical_resource_bundle(workspace.path(), ResourceLimits::default())
        .expect("matching digest must pass");
    assert_eq!(bundle.get("payload").expect("payload").bytes(), bytes);

    let zero_hash = "0".repeat(64);
    let mismatch = parse(&one_binary("payload.bin", Some(&zero_hash)));
    let diagnostics = mismatch
        .canonical_resource_bundle(workspace.path(), ResourceLimits::default())
        .expect_err("mismatched digest must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::RESOURCE_HASH_MISMATCH
    );
}

#[test]
fn rejects_missing_directory_and_non_regular_workspace_members() {
    let workspace = tempdir().expect("temporary workspace");
    fs::create_dir(workspace.path().join("directory")).expect("fixture directory");
    for path in ["missing.bin", "directory"] {
        let diagnostics = parse(&one_binary(path, None))
            .canonical_resource_bundle(workspace.path(), ResourceLimits::default())
            .expect_err("non-file workspace member must fail");
        assert_eq!(diagnostics.len(), 1, "{path}");
        assert_eq!(
            diagnostics[0].code(),
            DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE,
            "{path}"
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::net::UnixListener;

        let socket_path = workspace.path().join("socket");
        let _listener = UnixListener::bind(&socket_path).expect("fixture socket");
        let diagnostics = parse(&one_binary("socket", None))
            .canonical_resource_bundle(workspace.path(), ResourceLimits::default())
            .expect_err("non-regular socket must fail");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code(),
            DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE
        );
    }
}

#[cfg(unix)]
#[test]
fn accepts_in_root_symlink_and_rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let container = tempdir().expect("temporary container");
    let workspace = container.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace directory");
    fs::write(workspace.join("inside.bin"), b"inside").expect("inside payload");
    symlink("inside.bin", workspace.join("inside-link.bin")).expect("inside symlink");
    let inside = parse(&one_binary("inside-link.bin", None))
        .canonical_resource_bundle(&workspace, ResourceLimits::default())
        .expect("in-root symlink must resolve");
    assert_eq!(inside.get("payload").expect("payload").bytes(), b"inside");

    let outside = container.path().join("outside.bin");
    fs::write(&outside, b"outside").expect("outside payload");
    symlink(&outside, workspace.join("escape.bin")).expect("escaping symlink");
    let diagnostics = parse(&one_binary("escape.bin", None))
        .canonical_resource_bundle(&workspace, ResourceLimits::default())
        .expect_err("escaping symlink must fail");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::RESOURCE_UNKNOWN_REFERENCE
    );
}

#[test]
fn enforces_public_count_single_and_total_byte_budgets() {
    let workspace = tempdir().expect("temporary workspace");
    fs::write(workspace.path().join("one.bin"), b"one").expect("first payload");
    fs::write(workspace.path().join("two.bin"), b"two").expect("second payload");
    let document = parse(
        r#"#fcs 5.0.0
format { profile: fragment; }
resources {
    binary one { source: "one.bin"; mediaType: "application/octet-stream"; }
    binary two { source: "two.bin"; mediaType: "application/octet-stream"; }
}
"#,
    );

    let count = document
        .canonical_resource_bundle(workspace.path(), ResourceLimits::new(1, 3, 6))
        .expect_err("count budget must fail");
    assert_budget(&count, "resource-count", 1, 2);

    let single = document
        .canonical_resource_bundle(workspace.path(), ResourceLimits::new(2, 2, 6))
        .expect_err("single-resource budget must fail");
    assert_eq!(single.len(), 2);
    for diagnostic in &single {
        assert_eq!(diagnostic.code(), DiagnosticCode::RESOURCE_LIMIT_EXCEEDED);
        let budget = diagnostic.budget().expect("budget details");
        assert_eq!(budget.kind(), "single-resource-bytes");
        assert_eq!(budget.limit(), 2);
        assert_eq!(budget.observed(), 3);
    }

    let total = document
        .canonical_resource_bundle(workspace.path(), ResourceLimits::new(2, 3, 5))
        .expect_err("total-resource budget must fail");
    assert_budget(&total, "total-resource-bytes", 5, 6);
}

fn assert_budget(
    diagnostics: &[fcs_source::Diagnostic],
    kind: &str,
    limit: usize,
    observed: usize,
) {
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code(),
        DiagnosticCode::RESOURCE_LIMIT_EXCEEDED
    );
    let budget = diagnostics[0].budget().expect("budget details");
    assert_eq!(budget.kind(), kind);
    assert_eq!(budget.limit(), limit);
    assert_eq!(budget.observed(), observed);
}

#[test]
fn metadata_lowering_remains_filesystem_free_and_validates_core_contract_values() {
    let document = parse(&one_binary("does-not-exist.bin", None));
    assert!(document.canonical_metadata().is_ok());

    let invalid = parse(
        r#"#fcs 5.0.0
format { profile: fragment; }
resources {
    image image { source: "missing.png"; mediaType: "image/png"; sampling: "cubic"; }
    font font { source: "missing.ttf"; mediaType: "font/ttf"; faceCount: 2; }
}
"#,
    );
    let diagnostics = invalid
        .canonical_metadata()
        .expect_err("invalid canonical media contracts must fail before filesystem access");
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code() == DiagnosticCode::TYPE_INVALID_OPERATION)
    );
}
