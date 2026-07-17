#![no_main]

use fcs_source::{diagnostic::ParseOutput, parser::parse_expression};
use libfuzzer_sys::fuzz_target;

fn assert_bounded<T>(output: &ParseOutput<T>, source: &str) {
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
            assert!(source.is_char_boundary(span.start));
            assert!(source.is_char_boundary(span.end));
        }
    }
    if !output.diagnostics().is_empty() {
        assert!(
            output.output().is_none(),
            "parser returned a partial expression"
        );
    }
}

fuzz_target!(|bytes: &[u8]| {
    if let Ok(source) = std::str::from_utf8(bytes) {
        let output = parse_expression(source);
        assert_bounded(&output, source);
    }
});
