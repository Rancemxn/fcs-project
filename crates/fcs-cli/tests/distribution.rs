use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fcs_fcbc::ContainerProfile;
use fcs_model::CanonicalContentSha256;
use toml::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_toml(path: impl AsRef<Path>) -> Value {
    let path = path.as_ref();
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    toml::from_str(&text)
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

fn package_files(output: &[u8], package: &str) -> Vec<String> {
    String::from_utf8(output.to_vec())
        .unwrap_or_else(|error| {
            panic!("cargo package listed non-UTF-8 files for {package}: {error}")
        })
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.replace('\\', "/"))
        .collect()
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
    assert_eq!(
        string(workspace_package, "license"),
        "AGPL-3.0-or-later"
    );
    assert_eq!(string(workspace_package, "license-file"), "LICENSE");
    assert_eq!(string(&inventory, "workspace_version"), "5.0.0");
    assert_eq!(
        string(&inventory, "workspace_license"),
        "AGPL-3.0-or-later"
    );
    assert_eq!(
        string(&inventory, "contribution_policy"),
        "DCO + inbound=outbound"
    );

    let members = strings(&workspace["workspace"], "members");
    let inventory_members = strings(&inventory, "workspace_members");
    assert_eq!(inventory_members, members);
    for member in &members {
        let manifest = read_toml(root.join(member).join("Cargo.toml"));
        assert_eq!(
            manifest["package"]["version"]["workspace"].as_bool(),
            Some(true)
        );
        assert_eq!(
            manifest["package"]["license"]["workspace"].as_bool(),
            Some(true)
        );
        assert_eq!(
            manifest["package"]["license-file"]["workspace"].as_bool(),
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

    let policy_path = inventory_path("contribution_policy_file");
    let policy = fs::read_to_string(&policy_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", policy_path.display()));
    assert!(policy.contains("Contributions are inbound=outbound"));
    assert!(policy.contains("Developer's Certificate of Origin 1.1"));
    assert!(policy.contains("Signed-off-by:"));

    let license_path = inventory_path("license_file");
    let license_bytes = fs::read(&license_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", license_path.display()));
    let license = std::str::from_utf8(&license_bytes)
        .unwrap_or_else(|error| panic!("license is not UTF-8: {}: {error}", license_path.display()));
    let license_sha256 = lower_hex(&CanonicalContentSha256::digest(&license_bytes).as_bytes());
    assert_eq!(license_sha256, string(&inventory, "license_sha256"));
    assert_eq!(
        license_sha256,
        "0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0"
    );
    assert!(license.starts_with(
        "                    GNU AFFERO GENERAL PUBLIC LICENSE\n"
    ));
    assert!(license.contains("Version 3, 19 November 2007"));

    for relative in strings(&inventory, "utf8_paths") {
        let path = inventory_dir.join(&relative);
        let bytes = fs::read(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(
            std::str::from_utf8(&bytes).is_ok(),
            "distribution inventory path is not UTF-8: {}",
            path.display()
        );
    }

    let required_packages = inventory["package_required_files"]
        .as_array()
        .expect("package_required_files must be an array");
    let inventory_members: Vec<_> = required_packages
        .iter()
        .map(|package| string(package, "member"))
        .collect();
    assert_eq!(inventory_members, members);
    for package in required_packages {
        let member = string(package, "member");
        let manifest = read_toml(root.join(&member).join("Cargo.toml"));
        let package_name = string(&manifest["package"], "name");
        let output = Command::new("cargo")
            .arg("package")
            .arg("-p")
            .arg(&package_name)
            .args(["--list", "--allow-dirty", "--no-verify"])
            .current_dir(&root)
            .output()
            .unwrap_or_else(|error| panic!("failed to list package {package_name}: {error}"));
        assert!(
            output.status.success(),
            "cargo package failed for {package_name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let files = package_files(&output.stdout, &package_name);
        for required in strings(package, "files") {
            let required = required.replace('\\', "/");
            assert!(
                files.iter().any(|file| file == &required),
                "package {package_name} is missing required file {required}"
            );
        }
    }

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
