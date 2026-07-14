use std::path::Path;

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
