use super::*;

#[test]
fn note_kind_ordinals_match_section_12() {
    // fcbc.md section 12: kind:u8  1 tap, 2 hold, 3 flick, 4 drag.
    assert_eq!(note_kind_ordinal(CanonicalNoteKind::Tap), 1);
    assert_eq!(note_kind_ordinal(CanonicalNoteKind::Hold), 2);
    assert_eq!(note_kind_ordinal(CanonicalNoteKind::Flick), 3);
    assert_eq!(note_kind_ordinal(CanonicalNoteKind::Drag), 4);
}

#[test]
fn every_note_kind_has_a_distinct_ordinal_in_range() {
    let ordinals: Vec<u8> = [
        CanonicalNoteKind::Tap,
        CanonicalNoteKind::Hold,
        CanonicalNoteKind::Flick,
        CanonicalNoteKind::Drag,
    ]
    .into_iter()
    .map(note_kind_ordinal)
    .collect();
    let mut sorted = ordinals.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted,
        vec![1, 2, 3, 4],
        "ordinals must be 1..=4 and distinct"
    );
}
