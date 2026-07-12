//! Dynamic scan of COPYRIGHT charts: verify all community charts parse.
//! These charts are NOT version-controlled (see .gitignore).

mod common;

use std::path::Path;

#[test]
fn test_copyright_files_exist() {
    let dir = Path::new(&common::manifest_path("examples/COPYRIGHT")).to_path_buf();
    assert!(dir.exists(), "COPYRIGHT directory missing");
    let entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .map_or(false, |ext| ext == "json" || ext == "pec")
        })
        .collect();
    assert!(!entries.is_empty(), "no copyright chart files found");
}

#[test]
fn test_copyright_all_parse() {
    let dir = Path::new(&common::manifest_path("examples/COPYRIGHT")).to_path_buf();
    let mut parsed = 0u32;
    let mut errors = Vec::new();

    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if !path
            .extension()
            .map_or(false, |ext| ext == "json" || ext == "pec")
        {
            continue;
        }

        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                errors.push(format!("{name}: read error: {e}"));
                continue;
            }
        };

        let result = match path.extension().and_then(|e| e.to_str()) {
            Some("pec") => fcs_converter::pec::parse_pec(&src).map(|_| ()),
            Some("json") => fcs_converter::pgr::parse_pgr(&src)
                .or_else(|_| fcs_converter::rpe::parse_rpe(&src))
                .or_else(|_| fcs_converter::pec::parse_pec(&src))
                .map(|_| ()),
            _ => unreachable!(),
        };

        match result {
            Ok(()) => parsed += 1,
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }

    if !errors.is_empty() {
        panic!(
            "{}/{} files failed:\n{}",
            errors.len(),
            parsed + errors.len() as u32,
            errors.join("\n")
        );
    }
    assert!(parsed > 0, "no copyright chart files were parsed");
}
