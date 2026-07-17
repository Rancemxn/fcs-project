# FCS 5 fuzz corpus

The checked-in corpus is materialized into a temporary directory by
`scripts/fcs5-fuzz-smoke.sh`. It contains one file for every entry in
`conformance/fcs5/manifest.toml` plus the public `examples/fcs/*.fcs` inputs, so
the source corpus cannot silently drift from the manifest. The same corpus is
passed to the byte-document, UTF-8-document, and expression targets; each
target owns its generated corpus outside the repository.

Do not commit `target/`, `corpus/`, `artifacts/`, or generated libFuzzer output.
