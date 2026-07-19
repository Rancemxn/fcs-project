#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
runs=${FCS_FUZZ_RUNS:-32}
mode=${1:-bounded}

case "$mode" in
    bounded)
        fuzz_args=(--sanitizer none --dev)
        libfuzzer_args=(-runs="$runs" -max_len=65536)
        ;;
    unbounded)
        fuzz_args=()
        libfuzzer_args=()
        ;;
    *)
        printf 'usage: %s [bounded|unbounded]\n' "$0" >&2
        exit 2
        ;;
esac

if ! command -v cargo-fuzz >/dev/null 2>&1 && ! cargo fuzz --help >/dev/null 2>&1; then
    printf 'cargo-fuzz 0.13.2 is required; install it with: cargo install cargo-fuzz --version 0.13.2\n' >&2
    exit 127
fi

corpus=$(mktemp -d "${TMPDIR:-/tmp}/fcs5-fuzz-corpus.XXXXXX")
trap 'rm -rf "$corpus"' EXIT

python3 - "$root/docs/conformance/fcs5/manifest.toml" "$root/examples/fcs" "$corpus" <<'PY'
import shutil
import sys
import tomllib
from pathlib import Path

manifest_path, examples_dir, destination = map(Path, sys.argv[1:])
manifest_root = manifest_path.parent
destination.mkdir(parents=True, exist_ok=True)
manifest = tomllib.loads(manifest_path.read_text())

for fixture in manifest["fixture"]:
    source = manifest_root / fixture["path"]
    name = fixture["id"].replace("/", "_") + ".fcs"
    shutil.copyfile(source, destination / name)

for source in sorted(Path(examples_dir).glob("*.fcs")):
    shutil.copyfile(source, destination / ("example-" + source.name))
PY

cd "$root"
for target in document_bytes document_utf8 expression; do
    cargo fuzz run "${fuzz_args[@]}" "$target" "$corpus" -- "${libfuzzer_args[@]}"
done
