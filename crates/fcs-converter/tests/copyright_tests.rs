//! Dynamic scan of COPYRIGHT charts: verify all community charts parse.
//! These charts are NOT version-controlled (see .gitignore).

mod common;

use std::path::Path;

/// Try to parse a string with PGR first, then RPE, then PEC.
fn parse_any(src: &str) -> Result<fcs_converter::ir::IrChart, String> {
    fcs_converter::pgr::parse_pgr(src)
        .or_else(|_| fcs_converter::rpe::parse_rpe(src))
        .or_else(|_| fcs_converter::pec::parse_pec(src))
}

/// Count notes across all lines in a chart.
fn total_notes(chart: &fcs_converter::ir::IrChart) -> usize {
    chart
        .lines
        .iter()
        .map(|l| l.notes_above.len() + l.notes_below.len())
        .sum()
}

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

#[test]
fn test_copyright_roundtrip() {
    let dir = Path::new(&common::manifest_path("examples/COPYRIGHT")).to_path_buf();
    let mut passed = 0u32;
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

        // Phase 1: Parse to IR
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                errors.push(format!("{name}: read error: {e}"));
                continue;
            }
        };
        let chart_a = match parse_any(&src) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{name}: parse failed: {e}"));
                continue;
            }
        };
        let lines_a = chart_a.lines.len();
        let notes_a = total_notes(&chart_a);

        // Phase 2: Round-trip through FCS → PGR V3 → parse
        let doc = fcs_converter::to_fcs::ir_to_fcs(&chart_a);
        let pgr_json = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);

        let chart_b = match fcs_converter::pgr::parse_pgr(&pgr_json) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{name}: PGR round-trip failed: {e}"));
                continue;
            }
        };
        let notes_b = total_notes(&chart_b);

        // Phase 3: Structural comparison
        // PGR may repack lines (empty lines dropped, unused lines merged)
        // so only compare total note count, not line count.
        if notes_a != notes_b {
            errors.push(format!(
                "{name}: note count mismatch: {notes_a} (original) vs {notes_b} (round-trip)"
            ));
            continue;
        }
        passed += 1;
    }

    if !errors.is_empty() {
        panic!(
            "{}/{} files failed:\n{}",
            errors.len(),
            passed + errors.len() as u32,
            errors.join("\n")
        );
    }
    assert!(passed > 0, "no copyright chart files were round-tripped");
}
