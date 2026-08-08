# Codespaces

Creating a Codespace runs `setup.sh` through `devcontainer.json`. It installs
Rust `1.97.1` with `rustfmt` and `clippy`, `cargo-nextest 0.9.140`, and
`cargo-fuzz 0.13.2`. Re-run `bash .devcontainer/setup.sh` after rebuilding a
container; the version checks make the command repeatable.

Local tools provide development feedback only. The exact-head Full Gate in
GitHub Actions remains the delivery evidence for a commit.
