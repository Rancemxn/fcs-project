#![no_main]

use fcs_source::{diagnostic::ParseOutput, parser::parse_document_bytes};
use libfuzzer_sys::fuzz_target;

fn assert_bounded<T>(output: &ParseOutput<T>, source: &[u8]) {
    let valid_utf8 = std::str::from_utf8(source).ok();
    for diagnostic in output.diagnostics() {
        for span in std::iter::once(diagnostic.primary_span())
            .chain(diagnostic.labels().iter().map(|label| label.span()))
            .chain(
                diagnostic
                    .expansion_trace()
                    .iter()
                    .filter_map(|frame| frame.span()),
            )
        {
            assert!(span.start <= span.end, "reversed diagnostic span: {span:?}");
            assert!(
                span.end <= source.len(),
                "out-of-bounds diagnostic span: {span:?}"
            );
            if let Some(text) = valid_utf8 {
                assert!(text.is_char_boundary(span.start));
                assert!(text.is_char_boundary(span.end));
            }
        }
    }
    if !output.diagnostics().is_empty() {
        assert!(output.output().is_none(), "parser returned a partial AST");
    }
}

fuzz_target!(|source: &[u8]| {
    let output = parse_document_bytes(source);
    assert_bounded(&output, source);
});
