/// Resolve a path relative to the project root from the converter crate.
pub fn manifest_path(rel: &str) -> String {
    let dir = env!("CARGO_MANIFEST_DIR");
    let full = std::path::Path::new(dir).join("../../").join(rel);
    full.to_string_lossy().to_string()
}
