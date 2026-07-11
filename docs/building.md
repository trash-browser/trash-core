# Building

The canonical entry point is `build.py`. It orchestrates three things that must
happen in order:

1. Download and extract the pinned **CEF** distribution.
2. Build the **React shell** into `shell/dist/` (the Rust binary embeds
   `shell/dist/index.html`).
3. Run **cargo** with `CEF_PATH` pointing at the extracted CEF.

```sh
python build.py            # cargo build (debug)
python build.py --check    # cargo check only (fast, no final exe)
python build.py --release  # cargo build --release
python build.py -- --locked --features foo   # pass-through args after --
```

## What `build.py` does, step by step

### 1. CEF download & extraction

- The version is pinned in `build.py`:
  `CEF_VERSION = "150.0.11+gb887805+chromium-150.0.7871.115"`. It matches the
  `cef = "=150.0.0"` crate dependency in `Cargo.toml`. **If you bump one, bump
  both.**
- Archives come from `cef-builds.spotifycdn.com` (Windows64 / Linux64) and are
  cached under `build/downloads/`. Re-runs reuse the cache.
- Extraction is guarded against path-traversal (`Unsafe path in CEF archive`)
  and marked with a `.extracted` sentinel so it only happens once.
- Extracted tree lives at `build/cef_binary_<version>_<platform>64/`.

### 2. Shell build

- Prefers `bun run build`, falls back to `npm run build`; errors if neither is
  installed.
- Runs in `shell/` and injects version metadata as Vite env vars, parsed from
  CEF's `include/cef_version.h` and the crate's `Cargo.toml`:
  - `VITE_APP_VERSION` — crate version (`0.1.0`)
  - `VITE_CEF_VERSION` — e.g. `150.0.11`
  - `VITE_CHROMIUM_VERSION` — e.g. `150.0.7871.115`

  These surface on the About settings page via
  [`shell/src/data/version.ts`](../shell/src/data/version.ts). Building the shell
  standalone (`bun run build` directly) uses fallback strings (`dev`/`unknown`).

### 3. Cargo build

- Sets `CEF_PATH` to the extracted CEF directory. This is scoped to the Cargo
  process `build.py` spawns — it does not pollute your global environment.
- On **Windows**, `cef-dll-sys` needs **Ninja**. `build.py` looks for `ninja` on
  `PATH`, then inside Visual Studio's bundled CMake. If not found it errors with:
  ```
  winget install Ninja-build.Ninja
  ```
  It also sets `CMAKE_MAKE_PROGRAM` to the discovered Ninja.

## Building without `build.py`

Once CEF is extracted and the shell is built, you can run cargo directly if you
export the environment yourself:

```sh
export CEF_PATH=/abs/path/to/build/cef_binary_150.0.11+..._linux64
cd shell && bun run build && cd ..
cargo build
```

On Windows you additionally need Ninja on `PATH` (and `CMAKE_MAKE_PROGRAM`
pointing at it). Prefer `build.py` unless you have a reason not to.

## Toolchain requirements recap

| Platform | Needs |
|----------|-------|
| Windows | Rust (edition 2024), MSVC build tools, Python 3, Bun or npm, Ninja |
| Linux | Rust (edition 2024), clang/gcc + linker, X11 dev libs, Python 3, Bun or npm |

The app targets **X11** on Linux (it unsets `WAYLAND_DISPLAY` / `WAYLAND_SOCKET`
at startup and builds GPUI with the `x11` feature only — do not re-enable the
`wayland` defaults via Cargo feature unification). Under Wayland it runs through
Xwayland.

## Outputs

- Debug binary: `target/debug/trashbrowser-gpui`
- Release binary: `target/release/trashbrowser-gpui`

The binary is self-contained for its own UI (icons and the shell HTML are
embedded), but it still needs the CEF runtime files (libraries, `.pak`
resources, locales) from the CEF distribution alongside it to actually run —
that's standard CEF deployment.

## Runtime cache

At runtime the browser stores its CEF cache and the ad-block data under a
per-user cache directory (`$XDG_CACHE_HOME` / `$HOME/.cache` / temp), in
`trashbrowser-gpui/`:

- `adblock.dat` — SQLite statistics database
- `adblock/lists/` — downloaded EasyList/EasyPrivacy filter lists

Deleting that directory resets stats and cached filter lists; they're rebuilt on
next launch.
