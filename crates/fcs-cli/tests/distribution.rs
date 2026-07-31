use std::fs;
use std::path::{Path, PathBuf};

use fcs_fcbc::ContainerProfile;
use toml::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_toml(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    text.parse::<Value>()
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn strings(value: &Value, key: &str) -> Vec<String> {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("{key} must be an array"))
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{key} must contain strings"))
                .to_owned()
        })
        .collect()
}

fn string(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} must be a string"))
        .to_owned()
}

#[test]
fn inventory_matches_product_metadata_and_registries() {
    let root = root();
    let inventory = read_toml(root.join("docs/conformance/fcs5/distribution.toml"));
    assert_eq!(inventory["schema_version"].as_integer(), Some(1));
    assert_eq!(string(&inventory, "publication"), "unpublished");

    let workspace = read_toml(root.join("Cargo.toml"));
    let workspace_package = &workspace["workspace"]["package"];
    assert_eq!(string(workspace_package, "version"), "5.0.0");
    assert_eq!(string(workspace_package, "license"), "MIT");
    assert_eq!(string(&inventory, "workspace_version"), "5.0.0");
    assert_eq!(string(&inventory, "workspace_license"), "MIT");

    let members = strings(&workspace["workspace"], "members");
    let inventory_members = strings(&inventory, "workspace_members");
    assert_eq!(inventory_members, members);
    for member in members {
        let manifest = read_toml(root.join(&member).join("Cargo.toml"));
        assert_eq!(
            manifest["package"]["version"]["workspace"].as_bool(),
            Some(true)
        );
        assert_eq!(
            manifest["package"]["license"]["workspace"].as_bool(),
            Some(true)
        );
    }

    assert_eq!(
        strings(&inventory, "source_profiles"),
        ["fragment", "chart", "playable", "renderable", "publishable"]
    );
    assert_eq!(
        strings(&inventory, "fcbc_container_profiles"),
        [
            ContainerProfile::Runtime.as_str(),
            ContainerProfile::Fidelity.as_str(),
            ContainerProfile::StrictRuntime.as_str(),
        ]
    );

    let inventory_dir = root.join("docs/conformance/fcs5");
    let inventory_path = |key: &str| {
        let relative = string(&inventory, key);
        assert!(
            !Path::new(&relative).is_absolute(),
            "distribution inventory path {key} must be relative: {relative}"
        );
        let path = inventory_dir.join(&relative);
        assert!(
            path.is_file(),
            "distribution inventory path {key} does not resolve to a file: {}",
            path.display()
        );
        path
    };

    let fcbc_manifest = read_toml(inventory_path("fcbc_manifest"));
    assert_eq!(
        string(&inventory, "fcbc_version"),
        string(&fcbc_manifest, "fcbc_version")
    );
    assert_eq!(
        string(&inventory, "execution_abi_version"),
        string(&fcbc_manifest, "execution_abi_version")
    );

    let render_manifest = read_toml(inventory_path("render_manifest"));
    assert_eq!(
        string(&inventory, "render_profile_version"),
        string(&render_manifest, "render_profile_version")
    );

    let root_manifest = read_toml(inventory_path("root_conformance_manifest"));
    let suites = root_manifest["suite"]
        .as_array()
        .expect("root conformance manifest must contain suites");
    let suite_version = |id: &str| {
        suites
            .iter()
            .find(|suite| suite["id"].as_str() == Some(id))
            .map(|suite| string(suite, "version"))
            .unwrap_or_else(|| panic!("missing conformance suite {id}"))
    };
    assert_eq!(suite_version("fcs-core-source"), "5.0.0");
    assert_eq!(suite_version("fcbc-container"), "2.0.0");
    assert_eq!(suite_version("execution-abi"), "1.0.0");
    assert_eq!(suite_version("render-profile"), "1.0.0");
    assert_eq!(suite_version("conversion"), "1.0.0");

    let registry = read_toml(inventory_path("conversion_profile_registry"));
    let registry_profiles: Vec<_> = registry["profile"]
        .as_array()
        .expect("profile registry must contain profiles")
        .iter()
        .map(|profile| format!("{}@{}", string(profile, "id"), string(profile, "version")))
        .collect();
    assert_eq!(
        strings(&inventory, "conversion_profiles"),
        registry_profiles
    );
    assert_eq!(string(&inventory, "conversion_version"), "1.0.0");
}
