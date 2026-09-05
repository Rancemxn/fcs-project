#!/usr/bin/env python3
"""The binary fuzz lane must include the static Render golden and preserve cached inputs."""

import runpy
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
seeders = runpy.run_path(str(root / "scripts/fcs5-fuzz-seeds.py"))
seed = seeders["seed_fcbc_goldens"]
with tempfile.TemporaryDirectory(prefix="fcs-fuzz-seed-check-") as temporary:
    destination = Path(temporary)
    seed(root, destination)
    render = destination / "render-nonempty-render.bin"
    golden = root / "docs/conformance/render/nonempty-render.hex"
    assert render.read_bytes() == bytes.fromhex(golden.read_text(encoding="utf-8"))
    assert all(path.read_bytes().startswith(b"FCSB") for path in destination.iterdir())
    assert any(path.name.startswith("fcbc-") for path in destination.iterdir())
    render.write_bytes(b"cached input")
    seed(root, destination)
    assert render.read_bytes() == b"cached input"
    seeders["seed_asset_image"](root, destination)
    assets = root / "docs/conformance/render/assets"
    assert (destination / "fcs-test-rgba8.png.bin").read_bytes() == b"\x00" + (assets / "fcs-test-rgba8.png").read_bytes()
    assert (destination / "fcs-test-lossless.webp.bin").read_bytes() == b"\x01" + (assets / "fcs-test-lossless.webp").read_bytes()
    seeders["seed_asset_font"](root, destination)
    assert (destination / "fcs-test-font.ttf").read_bytes() == (assets / "fcs-test-font.ttf").read_bytes()
    assert (destination / "shaping-input.txt").read_bytes() == b"A"
