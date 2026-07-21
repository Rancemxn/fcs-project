//! Product execution of checked-in FCBC mutation corpora (I7.8).

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::load_chart;
use crate::load_container;

#[derive(Debug, Deserialize)]
struct MutationManifest {
    schema_version: u32,
    base: String,
    mutation: Vec<Mutation>,
}

#[derive(Debug, Deserialize)]
struct Mutation {
    id: String,
    diagnostic: String,
    patch: Vec<MutationPatch>,
}

#[derive(Debug, Deserialize)]
struct MutationPatch {
    offset: u64,
    replace_hex: String,
}

fn suite_base() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/conformance/fcbc")
}

fn decode_hex_file(path: &Path) -> Vec<u8> {
    let text = fs::read_to_string(path).expect("read hex");
    let filtered: String = text
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect();
    assert!(filtered.len().is_multiple_of(2), "odd hex length");
    (0..filtered.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&filtered[index..index + 2], 16).unwrap())
        .collect()
}

fn decode_hex_bytes(hex: &str) -> Vec<u8> {
    let filtered: String = hex.chars().filter(|ch| !ch.is_ascii_whitespace()).collect();
    assert!(filtered.len().is_multiple_of(2), "odd replace_hex");
    (0..filtered.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&filtered[index..index + 2], 16).unwrap())
        .collect()
}

fn apply_patches(base: &[u8], patches: &[MutationPatch]) -> Vec<u8> {
    let mut bytes = base.to_vec();
    for patch in patches {
        let replacement = decode_hex_bytes(&patch.replace_hex);
        let start = usize::try_from(patch.offset).expect("offset");
        let end = start
            .checked_add(replacement.len())
            .expect("patch end overflow");
        assert!(
            end <= bytes.len(),
            "patch out of bounds for {}",
            patch.offset
        );
        bytes[start..end].copy_from_slice(&replacement);
    }
    bytes
}

fn run_mutation_manifest(manifest_name: &str, use_full_load: bool) {
    let base_dir = suite_base();
    let manifest: MutationManifest =
        toml::from_str(&fs::read_to_string(base_dir.join(manifest_name)).unwrap()).unwrap();
    assert_eq!(manifest.schema_version, 2);
    let base_bytes = decode_hex_file(&base_dir.join(&manifest.base));
    assert!(!manifest.mutation.is_empty());
    for mutation in &manifest.mutation {
        let bytes = apply_patches(&base_bytes, &mutation.patch);
        let category = if use_full_load {
            match load_chart(&bytes) {
                Ok(_) => panic!(
                    "mutation {} unexpectedly loaded via load_chart",
                    mutation.id
                ),
                Err(category) => category,
            }
        } else {
            match load_container(&bytes) {
                Ok(_) => panic!(
                    "mutation {} unexpectedly loaded via load_container",
                    mutation.id
                ),
                Err(error) => error.category(),
            }
        };
        assert_eq!(
            category,
            mutation.diagnostic.as_str(),
            "mutation {} diagnostic mismatch",
            mutation.id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_runtime_mutations_reject_via_product_framing() {
        // Framing-level mutations on the empty-tempo golden use load_container.
        run_mutation_manifest("mutations.toml", false);
    }

    #[test]
    fn nonempty_execution_mutations_reject_via_product_core_load() {
        run_mutation_manifest("nonempty-execution-mutations.toml", true);
    }

    #[test]
    fn embedded_resource_mutations_reject_via_product_framing() {
        run_mutation_manifest("embedded-resource-mutations.toml", false);
    }
}
