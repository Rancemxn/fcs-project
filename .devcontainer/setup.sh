#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

toolchain=1.97.1
nextest_version=0.9.140
cargo_fuzz_version=0.13.2

rustup toolchain install "$toolchain" --profile minimal
rustup component add rustfmt clippy --toolchain "$toolchain"
rustup default "$toolchain"

install_tool() {
    local binary=$1
    local package=$2
    local version=$3
    if command -v "$binary" >/dev/null 2>&1 && "$binary" --version | grep -Fq "$version"; then
        return
    fi
    cargo +"$toolchain" install "$package" --version "$version" --locked
}

install_tool cargo-nextest cargo-nextest "$nextest_version"
install_tool cargo-fuzz cargo-fuzz "$cargo_fuzz_version"
