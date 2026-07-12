//! Dynamic scan of COPYRIGHT charts: verify all community charts parse.
//! These charts are NOT version-controlled (see .gitignore).

mod common;

use std::path::Path;

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// 5MB threshold: files above this only get structural checks.
const SAMPLED_SIZE_LIMIT: u64 = 5 * 1024 * 1024;

/// Try to parse a string with PGR first, then RPE, then PEC.
fn parse_any(src: &str) -> Result<fcs_converter::ir::IrChart, String> {
    fcs_converter::pgr::parse_pgr(src)
        .or_else(|_| fcs_converter::rpe::parse_rpe(src))
        .or_else(|_| fcs_converter::pec::parse_pec(src))
}

/// Parse and return the detected format alongside the chart.
fn detect_and_parse(src: &str) -> Result<(fcs_converter::ir::IrChart, &'static str), String> {
    if let Ok(c) = fcs_converter::pgr::parse_pgr(src) {
        return Ok((c, "PGR"));
    }
    if let Ok(c) = fcs_converter::rpe::parse_rpe(src) {
        return Ok((c, "RPE"));
    }
    if let Ok(c) = fcs_converter::pec::parse_pec(src) {
        return Ok((c, "PEC"));
    }
    Err("unknown format".into())
}

/// Round-trip through PGR: IR → FCS → PGR V3 → parse.
fn roundtrip_pgr(chart: &fcs_converter::ir::IrChart) -> Result<fcs_converter::ir::IrChart, String> {
    let doc = fcs_converter::to_fcs::ir_to_fcs(chart);
    let json = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    fcs_converter::pgr::parse_pgr(&json).map_err(|e| e.to_string())
}

/// Round-trip through RPE: IR → FCS → RPE JSON → parse.
fn roundtrip_rpe(chart: &fcs_converter::ir::IrChart) -> Result<fcs_converter::ir::IrChart, String> {
    let doc = fcs_converter::to_fcs::ir_to_fcs(chart);
    let json = fcs_converter::from_fcs::rpe_writer::fcs_to_rpe_json(&doc);
    fcs_converter::rpe::parse_rpe(&json).map_err(|e| e.to_string())
}

/// Round-trip through PEC: IR → FCS → PEC text → parse.
fn roundtrip_pec(chart: &fcs_converter::ir::IrChart) -> Result<fcs_converter::ir::IrChart, String> {
    let doc = fcs_converter::to_fcs::ir_to_fcs(chart);
    let text = fcs_converter::from_fcs::pec_writer::fcs_to_pec(&doc);
    fcs_converter::pec::parse_pec(&text).map_err(|e| e.to_string())
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

        // Phase 1: Parse to IR + detect source format
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                errors.push(format!("{name}: read error: {e}"));
                continue;
            }
        };
        let (chart_a, fmt) = match detect_and_parse(&src) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!("{name}: parse failed: {e}"));
                continue;
            }
        };
        let notes_a = total_notes(&chart_a);

        // Phase 2: Round-trip through the ORIGINAL format (same-format only)
        let chart_b = match fmt {
            "PGR" => match roundtrip_pgr(&chart_a) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{name}: PGR round-trip failed: {e}"));
                    continue;
                }
            },
            "RPE" => match roundtrip_rpe(&chart_a) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{name}: RPE round-trip failed: {e}"));
                    continue;
                }
            },
            "PEC" => match roundtrip_pec(&chart_a) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(format!("{name}: PEC round-trip failed: {e}"));
                    continue;
                }
            },
            _ => unreachable!(),
        };
        let notes_b = total_notes(&chart_b);

        // Phase 3: Structural comparison
        // Some formats may repack lines, so only compare total note count.
        if notes_a != notes_b {
            errors.push(format!(
                "{name}: note count mismatch: {notes_a} (original) vs {notes_b} (round-trip)"
            ));
            continue;
        }

        // Phase 4: Sampled event precision (small files only)
        if file_size(&path) <= SAMPLED_SIZE_LIMIT {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                common::compare_events_sampled(
                    &chart_a,
                    &chart_b,
                    200,
                    common::EventTolerances {
                        move_x: 0.1,
                        move_y: 0.1,
                        rotate: 0.001,
                        speed: 0.01,
                        alpha: 0.01,
                    },
                );
            }));
            match result {
                Ok(()) => {}
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else {
                        "unknown error".into()
                    };
                    errors.push(format!("{name}: {msg}"));
                    continue;
                }
            }
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
