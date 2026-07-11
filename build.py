#!/usr/bin/env python3
"""Download the platform CEF distribution and build Trash Browser."""

from __future__ import annotations

import argparse
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.parse
import urllib.request
from pathlib import Path

CEF_VERSION = "150.0.11+gb887805+chromium-150.0.7871.115"
CEF_URLS = {
    "Windows": f"https://cef-builds.spotifycdn.com/cef_binary_{urllib.parse.quote(CEF_VERSION, safe='')}_windows64.tar.bz2",
    "Linux": f"https://cef-builds.spotifycdn.com/cef_binary_{urllib.parse.quote(CEF_VERSION, safe='')}_linux64.tar.bz2",
}

ROOT = Path(__file__).resolve().parent
BUILD_DIR = ROOT / "build"


def download(url: str, destination: Path) -> None:
    if destination.is_file():
        print(f"Using cached archive: {destination}")
        return

    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = destination.with_suffix(destination.suffix + ".part")
    print(f"Downloading {url}")
    try:
        with urllib.request.urlopen(url) as response, temporary.open("wb") as output:
            shutil.copyfileobj(response, output)
        temporary.replace(destination)
    finally:
        temporary.unlink(missing_ok=True)


def extract(archive: Path, destination: Path) -> Path:
    directory_name = archive.name.removesuffix(".tar.bz2")
    cef_dir = destination / directory_name
    marker = cef_dir / ".extracted"
    if marker.is_file():
        print(f"Using extracted CEF: {cef_dir}")
        return cef_dir

    if cef_dir.exists():
        shutil.rmtree(cef_dir)

    print(f"Extracting {archive} to {destination}")
    destination.mkdir(parents=True, exist_ok=True)
    destination_resolved = destination.resolve()
    with tarfile.open(archive, "r:bz2") as bundle:
        for member in bundle.getmembers():
            member_path = (destination / member.name).resolve()
            if destination_resolved not in member_path.parents and member_path != destination_resolved:
                raise RuntimeError(f"Unsafe path in CEF archive: {member.name}")
        bundle.extractall(destination)

    if not cef_dir.is_dir():
        raise RuntimeError(f"CEF archive did not contain the expected directory: {cef_dir}")
    marker.touch()
    return cef_dir


def find_windows_ninja() -> Path | None:
    existing = shutil.which("ninja")
    if existing:
        return Path(existing)

    roots = [
        Path(os.environ.get("ProgramFiles", r"C:\Program Files")),
        Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")),
    ]
    patterns = (
        "Microsoft Visual Studio/*/*/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja.exe",
        "Microsoft Visual Studio/*/*/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja/ninja-build.exe",
    )
    for root in roots:
        for pattern in patterns:
            matches = sorted(root.glob(pattern), reverse=True)
            if matches:
                return matches[0]
    return None


def read_cef_versions(cef_dir: Path) -> tuple[str, str]:
    """Return (cef_version, chromium_version) parsed from CEF's version header."""
    header = cef_dir / "include" / "cef_version.h"
    try:
        text = header.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return "", ""

    def define(name: str) -> str:
        match = re.search(rf'#define\s+{name}\s+"?([^"\s]+)', text)
        return match.group(1).strip() if match else ""

    # CEF_VERSION looks like "150.0.11+gb887805+chromium-150.0.7871.115".
    cef_full = define("CEF_VERSION")
    cef_version = cef_full.split("+", 1)[0] if cef_full else ""
    parts = [define(f"CHROME_VERSION_{p}") for p in ("MAJOR", "MINOR", "BUILD", "PATCH")]
    chromium_version = ".".join(p for p in parts if p)
    return cef_version, chromium_version


def read_app_version() -> str:
    """Read the crate version from the top of Cargo.toml."""
    try:
        text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    except OSError:
        return ""
    match = re.search(r'(?m)^\s*version\s*=\s*"([^"]+)"', text)
    return match.group(1) if match else ""


def shell_env(cef_dir: Path) -> dict[str, str]:
    """Environment for the shell build, carrying version metadata for Vite."""
    cef_version, chromium_version = read_cef_versions(cef_dir)
    env = os.environ.copy()
    env["VITE_APP_VERSION"] = read_app_version()
    env["VITE_CEF_VERSION"] = cef_version
    env["VITE_CHROMIUM_VERSION"] = chromium_version
    print(
        f"Shell versions: app={env['VITE_APP_VERSION'] or '?'} "
        f"cef={cef_version or '?'} chromium={chromium_version or '?'}"
    )
    return env


def build_shell(env: dict[str, str]) -> None:
    shell_dir = ROOT / "shell"
    if shutil.which("bun"):
        command = ["bun", "run", "build"]
    elif shutil.which("npm"):
        command = ["npm", "run", "build"]
    else:
        raise RuntimeError("Building the web shell requires Bun or Node.js/npm")

    print("Building web shell:", " ".join(command))
    result = subprocess.call(command, cwd=shell_dir, env=env)
    if result != 0:
        raise RuntimeError(f"Web shell build failed with exit code {result}")


def build_environment(cef_dir: Path, system: str) -> dict[str, str]:
    env = os.environ.copy()
    env["CEF_PATH"] = str(cef_dir)

    if system == "Windows":
        ninja = find_windows_ninja()
        if ninja is None:
            raise RuntimeError(
                "Ninja is required by cef-dll-sys but was not found. Install it "
                "with `winget install Ninja-build.Ninja`, then rerun this script."
            )
        env["PATH"] = str(ninja.parent) + os.pathsep + env.get("PATH", "")
        env["CMAKE_MAKE_PROGRAM"] = str(ninja)
        print(f"Using Ninja: {ninja}")

    return env


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="run cargo check instead of cargo build",
    )
    parser.add_argument(
        "--release",
        action="store_true",
        help="build in release mode (passes --release to cargo)",
    )
    parser.add_argument(
        "cargo_args",
        nargs=argparse.REMAINDER,
        help="additional Cargo arguments (place them after --)",
    )
    args = parser.parse_args()

    system = platform.system()
    url = CEF_URLS.get(system)
    if url is None:
        parser.error(f"CEF download is not configured for {system}")

    archive = BUILD_DIR / "downloads" / Path(urllib.parse.unquote(url)).name
    cef_dir = extract(download_and_return(url, archive), BUILD_DIR)
    env = build_environment(cef_dir, system)
    build_shell(shell_env(cef_dir))

    command = ["cargo", "check" if args.check else "build"]
    if args.release:
        command.append("--release")
    cargo_args = args.cargo_args
    if cargo_args[:1] == ["--"]:
        cargo_args = cargo_args[1:]
    command.extend(cargo_args)

    print(f"CEF_PATH={cef_dir}")
    print("Running:", " ".join(command))
    return subprocess.call(command, cwd=ROOT, env=env)


def download_and_return(url: str, destination: Path) -> Path:
    download(url, destination)
    return destination


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, RuntimeError) as error:
        print(f"error: {error}", file=sys.stderr)
        raise SystemExit(1)
