#!/usr/bin/env python3
"""Materialize per-target fuzz seed corpora from checked-in evidence.

Usage: fcs5-fuzz-seeds.py <repo_root> <dest_root> [target ...]

Seeds come only from files the repository already binds as evidence, so the
fuzz corpus cannot silently drift from the conformance manifests:

- FCS source targets: every fixture in ``docs/conformance/fcs5/manifest.toml``
  plus the public ``examples/fcs/*.fcs`` inputs;
- FCBC/Render targets: every manifest-declared Core and Render binary golden, hex-decoded;
- importer targets: the public conversion fixture sources;
- ``asset_image``: the fixed Render PNG and WebP behind their selector bytes;
- ``asset_font``: the fixed TrueType font and its declared shaping input.

Every requested target directory is created even when it stays empty.
Existing destination files are never overwritten or deleted, so a
coverage-grown corpus restored from cache keeps its inputs.
"""

import shutil
import sys
import tomllib
from pathlib import Path


def place(destination: Path, name: str, data: bytes) -> None:
    path = destination / name
    if not path.exists():
        path.write_bytes(data)


def copy(destination: Path, name: str, source: Path) -> None:
    path = destination / name
    if not path.exists():
        shutil.copyfile(source, path)


def seed_fcs_source(root: Path, destination: Path) -> None:
    manifest_path = root / "docs" / "conformance" / "fcs5" / "manifest.toml"
    manifest = tomllib.loads(manifest_path.read_text())
    for fixture in manifest["fixture"]:
        source = manifest_path.parent / fixture["path"]
        copy(destination, fixture["id"].replace("/", "_") + ".fcs", source)
    for source in sorted((root / "examples" / "fcs").glob("*.fcs")):
        copy(destination, "example-" + source.name, source)


def seed_fcbc_goldens(root: Path, destination: Path) -> None:
    for domain, table in (("fcbc", "fixture"), ("render", "binary_fixture")):
        base = root / "docs" / "conformance" / domain
        manifest = tomllib.loads((base / "manifest.toml").read_text(encoding="utf-8"))
        for fixture in manifest[table]:
            golden = tomllib.loads((base / fixture["manifest"]).read_text(encoding="utf-8"))
            source = base / golden["path"]
            data = bytes.fromhex(source.read_text(encoding="utf-8"))
            place(destination, domain + "-" + source.stem + ".bin", data)


def seed_conversion_sources(root: Path, destination: Path, suffix: str) -> None:
    sources = root / "docs" / "conformance" / "conversion" / "public-fixtures" / "sources"
    for source in sorted(sources.glob("*" + suffix)):
        copy(destination, source.name, source)


def seed_asset_image(root: Path, destination: Path) -> None:
    assets = root / "docs/conformance/render/assets"
    # Bit 0 selects PNG/WebP; the remaining zero bits select sRGB, straight alpha.
    for selector, name in ((0, "fcs-test-rgba8.png"), (1, "fcs-test-lossless.webp")):
        place(destination, name + ".bin", bytes([selector]) + (assets / name).read_bytes())


def seed_asset_font(root: Path, destination: Path) -> None:
    render = root / "docs/conformance/render"
    copy(destination, "fcs-test-font.ttf", render / "assets/fcs-test-font.ttf")
    vector = tomllib.loads((render / "nonempty-render.vector.toml").read_text(encoding="utf-8"))
    place(destination, "shaping-input.txt", vector["shaping"]["input"].encode("utf-8"))


SEEDERS = {
    "document_bytes": seed_fcs_source,
    "document_utf8": seed_fcs_source,
    "expression": seed_fcs_source,
    "fcbc_container": seed_fcbc_goldens,
    "render_section": seed_fcbc_goldens,
    "asset_image": seed_asset_image,
    "asset_font": seed_asset_font,
    "import_pgr": lambda root, dest: seed_conversion_sources(root, dest, ".pgr.json"),
    "import_rpe": lambda root, dest: seed_conversion_sources(root, dest, ".rpe.json"),
    "import_pec": lambda root, dest: seed_conversion_sources(root, dest, ".pec"),
}


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__.strip().splitlines()[2], file=sys.stderr)
        return 2
    root = Path(sys.argv[1]).resolve()
    dest_root = Path(sys.argv[2])
    targets = sys.argv[3:] or sorted(SEEDERS)
    unknown = [target for target in targets if target not in SEEDERS]
    if unknown:
        print(f"unknown fuzz targets: {', '.join(unknown)}", file=sys.stderr)
        return 2
    for target in targets:
        destination = dest_root / target
        destination.mkdir(parents=True, exist_ok=True)
        SEEDERS[target](root, destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
