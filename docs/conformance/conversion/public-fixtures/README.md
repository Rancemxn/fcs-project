# Public importer fixture lane (I6.7)

Executable public fixtures for PGR/RPE/PEC import with expected
`ConversionReport` status keys and canonical shape facts.

## Layout

```text
manifest.toml           fixture index (lane = "public")
sources/                checked-in synthetic chart bytes
expected/               status, line/note counts, provenance keys
copyright/README.md     opt-in private copyright lane policy
```

## Execution

The product harness in `fcs_conversion::fixture_lane` loads `manifest.toml`,
reads source bytes as-is (UTF-8 identity for text formats), runs the real
parse → interpret → lower pipeline, and compares:

- top-level `ConversionReport` status
- line and note counts on the assembled `CanonicalChart`
- required report category presence
- required restricted provenance keys
- empty resource bundle for chart-only fixtures

See `copyright/README.md` for the opt-in private lane that must report
`skipped` when copyrighted charts are not present.
