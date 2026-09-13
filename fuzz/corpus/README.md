# FCS 5 fuzz corpus

Seed corpora are materialized into a temporary per-target directory tree by
`scripts/fcs5-fuzz-seeds.py` (invoked by `scripts/fcs5-fuzz-smoke.sh` and by
`.github/workflows/weekly-fuzz.yml`). Seeds come only from checked-in
evidence, so no corpus can silently drift from the conformance manifests:

- `document_bytes` / `document_utf8` / `expression`: one file for every entry
  in `docs/conformance/fcs5/manifest.toml` plus the public
  `examples/fcs/*.fcs` inputs;
- `fcbc_container` / `render_section`: every binary golden declared by the
  FCBC `fixture` and Render `binary_fixture` manifests, hex-decoded;
- `import_pgr` / `import_rpe` / `import_pec`: the public conversion fixture
  sources under `docs/conformance/conversion/public-fixtures/sources/`;
- `asset_image`: the fixed Render PNG and lossless WebP behind their
  media-type parameter-selector bytes;
- `asset_font`: the fixed TrueType font and the declared `simple-ltr-1` shaping input.

The weekly workflow additionally grows each target's corpus across runs via
the Actions cache; the seeds script never overwrites or deletes grown inputs.

Do not commit `target/`, `corpus/`, `artifacts/`, or generated libFuzzer output.
