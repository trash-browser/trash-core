#!/usr/bin/env python3
"""Convert app.png into a multi-resolution app.ico for the Windows build.

The Rust build script (build.rs) embeds the resulting app.ico as resource id 1,
which GPUI loads for the window / taskbar icon and Explorer uses for the .exe.

Usage:
    python assets/appicon/make_ico.py
"""

from __future__ import annotations

from pathlib import Path

from PIL import Image

HERE = Path(__file__).resolve().parent
# Sizes Windows requests for the taskbar, Alt+Tab, title bar, and Explorer.
SIZES = [16, 24, 32, 48, 64, 128, 256]


def main() -> None:
    source = HERE / "app.png"
    target = HERE / "app.ico"

    image = Image.open(source).convert("RGBA")
    image.save(target, format="ICO", sizes=[(size, size) for size in SIZES])

    labels = ", ".join(f"{size}x{size}" for size in SIZES)
    print(f"Wrote {target} ({labels})")


if __name__ == "__main__":
    main()
