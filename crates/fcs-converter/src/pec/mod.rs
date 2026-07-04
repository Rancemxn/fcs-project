//! PEC (PhiEditer text) → IR converter. Ref: phispler-ext pec2rpe.
use crate::ir::*;

pub fn parse_pec(text: &str) -> Result<IrChart, String> {
    let lines_raw: Vec<&str> = text.lines().collect();
    if lines_raw.is_empty() { return Err("empty PEC file".into()); }
    let offset = lines_raw[0].trim().parse::<f64>().map_err(|e| format!("PEC offset: {}", e))?;
    let offset_seconds = (offset - 150.0) / 1000.0;

    // Split note-speed/size pairs: "n1 ... # 1.0 & 1.0" → separate lines
    let tokens: Vec<Vec<String>> = lines_raw[1..].iter()
        .flat_map(|l| l.replace(" #", "\n#").replace(" &", "\n&").split('\n').map(|s| s.to_string()).collect::<Vec<_>>())
        .map(|l| l.split(' ').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect::<Vec<_>>())
        .filter(|v| !v.is_empty())
        .collect();

    let mut bpm_list = Vec::new();
    let mut notes_raw = Vec::new();
    let mut speeds = Vec::new();
    let mut sizes = Vec::new();

    for tok in &tokens {
        if tok.is_empty() { continue; }
        match tok[0].as_str() {
            "bp" if tok.len() >= 3 => {
                // New PEC format: bp <beat_float> <bpm_float> (direct values, not /2048)
                bpm_list.push(IrBpmPoint { beat: tok[1].parse::<f64>().unwrap_or(0.0), bpm: tok[2].parse::<f64>().unwrap_or(120.0) });
            }
            s if s.starts_with('n') && s.len() == 2 && tok.len() >= 5 => notes_raw.push(tok.clone()),
            "#" if tok.len() >= 2 => speeds.push(tok[1].parse::<f64>().unwrap_or(1.0)),
            "&" if tok.len() >= 2 => sizes.push(tok[1].parse::<f64>().unwrap_or(1.0)),
            _ => {}
        }
    }

    let default_bpm = bpm_list.first().map(|b| b.bpm).unwrap_or(120.0);
    let all_notes: Vec<IrNote> = notes_raw.iter().enumerate().map(|(i, raw)| {
        let kind = match raw[0].as_str() { "n1"=>IrNoteKind::Tap, "n2"=>IrNoteKind::Drag, "n3"=>IrNoteKind::Hold, "n4"=>IrNoteKind::Flick, _=>IrNoteKind::Fake };
        let note_speed = speeds.get(i).copied().unwrap_or(1.0);
        let size = sizes.get(i).copied().unwrap_or(1.0);
        let hold_time = if kind == IrNoteKind::Hold && raw.len() >= 6 { raw[5].parse::<f64>().unwrap_or(0.0) } else { 0.0 };
        // New PEC format: direct float values (not /2048 scaled).
        // x is in RPE canvas coords (1350-wide). Map to FCS 1920-wide.
        let x_fcs = raw[3].parse::<f64>().unwrap_or(0.0) / 1350.0 * 1920.0;
        IrNote {
            kind, above: raw[1].parse::<i32>().unwrap_or(1) == 1,
            time_beat: raw[2].parse::<f64>().unwrap_or(0.0), // direct beat value
            position_x: x_fcs, speed: note_speed, hold_beat: hold_time,
            is_fake: false, alpha: 1.0, size,
            y_offset: 0.0, visible_time: 0.0,
        }
    }).collect();

    let line = IrLine { name: "line0".into(), notes_above: all_notes.iter().filter(|n| n.above).cloned().collect(), notes_below: all_notes.iter().filter(|n| !n.above).cloned().collect(), events: IrEventBundle::default(), bpm: default_bpm, z_order: 0, texture: None };

    Ok(IrChart { meta: IrMeta { source_format: "pec".into(), ..Default::default() }, bpm_list, offset_seconds, lines: vec![line] })
}
