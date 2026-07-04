//! BPM timeline → LUT compilation.
use crate::ast::BpmTimeline;
use crate::bytecode::sections::TimelineLutEntry;
use crate::error::CompileError;

pub fn build_bpm_lut(timeline: &BpmTimeline) -> Result<Vec<TimelineLutEntry>, CompileError> {
    let entries = &timeline.entries;
    if entries.is_empty() { return Err(CompileError::MasterTimelineNonZeroStart); }
    if entries[0].beat != 0.0 { return Err(CompileError::MasterTimelineNonZeroStart); }
    for e in entries { if e.bpm <= 0.0 { return Err(CompileError::MasterTimelineBpmNonPositive); } }

    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());

    let mut lut = Vec::new();
    let mut prev_beat = 0.0;
    let mut prev_bpm = entries[0].bpm;
    let mut accumulated = 0.0;
    let mut i = 0;

    while i < sorted.len() {
        let beat = sorted[i].beat;
        let bpm = sorted[i].bpm;
        let is_step = i + 1 < sorted.len() && (sorted[i + 1].beat - beat).abs() < 1e-10;
        if is_step {
            let dt = (beat - prev_beat) / prev_bpm * 60.0;
            accumulated += dt;
            lut.push(TimelineLutEntry { beat, accumulated_sec: accumulated, bpm });
            let next_bpm = sorted[i + 1].bpm;
            lut.push(TimelineLutEntry { beat, accumulated_sec: accumulated, bpm: next_bpm });
            prev_beat = beat; prev_bpm = next_bpm; i += 2;
        } else {
            let dt = (beat - prev_beat) / prev_bpm * 60.0;
            accumulated += dt;
            lut.push(TimelineLutEntry { beat, accumulated_sec: accumulated, bpm });
            prev_beat = beat; prev_bpm = bpm; i += 1;
        }
    }
    Ok(lut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::BpmEntry;
    #[test]
    fn test_simple() {
        let tl = BpmTimeline { entries: vec![
            BpmEntry { beat:0.0, bpm:120.0, is_step_before:false },
            BpmEntry { beat:4.0, bpm:120.0, is_step_before:false },
        ]};
        let lut = build_bpm_lut(&tl).unwrap();
        assert!((lut[1].accumulated_sec - 2.0).abs() < 0.01);
    }
}
