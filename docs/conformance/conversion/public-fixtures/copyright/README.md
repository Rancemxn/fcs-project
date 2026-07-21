# Opt-in copyright fixture lane

Private copyrighted charts are **not** shipped in this repository. Conformance
for copyright material is an opt-in lane only (Conversion Specification §16–17).

## Policy

- Default CI and local runs must not require copyrighted chart files.
- When the copyright root is absent or empty, the harness reports
  `skipped` with a stable reason. Skipped is not a pass and is not a failure.
- Opt-in is explicit via environment variable
  `FCS_COPYRIGHT_FIXTURE_ROOT` pointing at a directory that contains a
  `manifest.toml` with the same shape as the public fixture manifest
  (`lane = "copyright"`).
- Hash of copyrighted bytes is never committed here; only policy, harness
  hooks, and public synthetic fixtures live in-repo.

## Manifest shape

```toml
schema_version = 1
conversion_specification_version = "1.0.0"
lane = "copyright"

[[fixture]]
id = "private-example"
lane = "copyright"
class = "feature"
format = "pgr"
parser_dialect = "pgr.json.v3"
profile = "pgr.phira.v3"
profile_version = "1.0.0"
floor_scale_px = "120"
source = "sources/private-example.pgr.json"
expected = "expected/private-example.toml"
producer_evidence = "license-holder-export"
runtime_evidence = "none"
policy = "strict"
```

Each expected file must declare `expected_status`, `expected_lines`,
`expected_notes`, `required_categories`, `required_provenance_keys`, and
`empty_resources` exactly like the public lane.

## Harness entry

Product API:

- `fcs_conversion::fixture_lane::public_fixture_root()` — checked-in public corpus
- `fcs_conversion::fixture_lane::load_fixture_manifest(path)`
- `fcs_conversion::fixture_lane::run_import_fixture(root, fixture)`
- `fcs_conversion::fixture_lane::copyright_lane_status()` — `Active` or `Skipped`

Do not commit chart files under this directory.
