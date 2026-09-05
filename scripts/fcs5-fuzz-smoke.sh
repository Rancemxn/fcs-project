#!/usr/bin/env bash
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
runs=${FCS_FUZZ_RUNS:-1024}
mode=${1:-bounded}
host_target=$(rustc --print host-tuple)

targets=(
    document_bytes
    document_utf8
    expression
    fcbc_container
    render_section
    asset_image
    asset_font
    import_pgr
    import_rpe
    import_pec
)

case "$mode" in
    bounded)
        fuzz_args=(--target "$host_target" --sanitizer none --dev)
        libfuzzer_args=(-runs="$runs" -max_len=65536)
        ;;
    unbounded)
        fuzz_args=(--target "$host_target")
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

python3 "$root/scripts/test-fcs5-fuzz-seeds.py"
python3 "$root/scripts/fcs5-fuzz-seeds.py" "$root" "$corpus" "${targets[@]}"

cd "$root"
for target in "${targets[@]}"; do
    cargo fuzz run "${fuzz_args[@]}" "$target" "$corpus/$target" -- "${libfuzzer_args[@]}"
done
