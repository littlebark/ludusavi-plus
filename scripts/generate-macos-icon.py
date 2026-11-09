#!/usr/bin/env python3
"""Generate a padded macOS ICNS file from Ludusavi's SVG icon."""

import argparse
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


CANVAS_SIZE = 1024
ART_SIZE = 820
ICON_SIZES = [(16, 16), (32, 32), (64, 64), (128, 128), (256, 256), (512, 512), (1024, 1024)]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="source SVG file")
    parser.add_argument("output", type=Path, help="output ICNS file")
    return parser.parse_args()


def main() -> None:
    args = parse_args()

    if shutil.which("rsvg-convert") is None:
        sys.exit("rsvg-convert is required; install librsvg before generating the macOS icon")

    try:
        from PIL import Image
    except ImportError:
        sys.exit("Pillow is required; install it with: python3 -m pip install pillow")

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temp_dir:
        raster = Path(temp_dir) / "icon.png"
        subprocess.run(
            [
                "rsvg-convert",
                "--width",
                str(CANVAS_SIZE),
                "--height",
                str(CANVAS_SIZE),
                str(args.input),
                "--output",
                str(raster),
            ],
            check=True,
        )

        art = Image.open(raster).convert("RGBA")
        art.thumbnail((ART_SIZE, ART_SIZE), Image.Resampling.LANCZOS)
        canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
        canvas.alpha_composite(art, ((CANVAS_SIZE - art.width) // 2, (CANVAS_SIZE - art.height) // 2))
        canvas.save(args.output, format="ICNS", sizes=ICON_SIZES)


if __name__ == "__main__":
    main()
