# Contributing to Trash Browser

Thanks for your interest in contributing. This guide covers getting a build
running, the layout of the code you'll touch, and the conventions the project
follows.

## Prerequisites

| Tool | Why | Notes |
|------|-----|-------|
| **Rust** (edition 2024) | Builds the browser binary and `trash_adblock` | Install via [rustup](https://rustup.rs). Edition 2024 needs a recent stable toolchain. |
| **Python 3** | Runs `build.py` (CEF download + orchestration) | Standard library only; no pip packages required. |
| **Bun** *or* **Node.js/npm** | Builds the React shell in `shell/` | Bun is preferred (the lockfile is `bun.lock`); npm works too. |
| **Ninja** (Windows only) | Required by `cef-dll-sys` to build CEF glue | `winget install Ninja-build.Ninja`, or use the copy bundled with Visual Studio. |
| **A C/C++ toolchain** | Links against the CEF binaries | MSVC on Windows; clang/gcc on Linux. |

CEF itself is **downloaded automatically** by `build.py` (version pinned in that
file) and extracted under `build/`. You do not fetch it manually.

Supported platforms today: **Windows** and **Linux** (X11 — the app forces X11
over Wayland). macOS is not wired up.

## First build

```sh
python build.py
```

This downloads the matching CEF distribution, builds the React shell into
`shell/dist/`, sets `CEF_PATH`, and runs `cargo build`. See
[building.md](building.md) for the full flow and every flag.

Fast iteration:

```sh
python build.py --check         # cargo check, no final executable
python build.py --release       # optimized build
python build.py -- --locked     # pass extra args through to cargo
```

Run the produced binary from `target/debug/trashbrowser-gpui` (or
`target/release/…`).

> If CEF is already configured in your environment (`CEF_PATH` exported and the
> shell already built), you can run `cargo build`/`cargo check` directly. When
> in doubt, go through `build.py` — it scopes `CEF_PATH` to the Cargo process it
> spawns.

## Running tests

The ad-block crate holds most of the unit tests and needs no CEF or GPUI:

```sh
cargo test -p trash_adblock
```

Notes:

- One test is `#[ignore]`d because it hits the live EasyList servers
  (`official_sources_download_and_block_a_known_tracker`). Run it explicitly
  with `cargo test -p trash_adblock -- --ignored` when you're changing the
  subscription downloader.
- `src/cef_host.rs` contains a unit test for the YouTube response filter
  (`youtube_response_filter_tests`). Building the top-level binary requires CEF,
  so run these through the same environment `build.py` sets up.

Shell checks:

```sh
cd shell
bun run lint          # ESLint
bun run build         # tsc -b typecheck + production bundle
```

## What to work on where

| You want to change… | Edit… |
|---------------------|-------|
| Tabs, toolbar, window controls, URL bar behaviour, URL normalization | `src/main.rs` |
| CEF lifecycle, message pump, navigation, focus handoff, request/response hooks | `src/cef_host.rs` |
| How the CEF child window is sized/positioned under the chrome | `src/content.rs` |
| Address-bar text editing/selection | `src/omnibox.rs` |
| Ad-block matching, site policy, YouTube sanitizing, statistics, subscriptions | `crates/trash_adblock/` |
| New Tab / Settings / Downloads / Ad-block UI, overlays, i18n | `shell/src/` |
| Icons embedded into the binary, version metadata | `build.rs`, `assets/` |
| CEF version, build orchestration | `build.py` |

Deeper module maps live in [architecture.md](architecture.md),
[adblock.md](adblock.md), and [shell.md](shell.md).

## Coding conventions

**Rust**

- Format with `cargo fmt`; keep `cargo clippy` clean for code you touch.
- The `trash_adblock` crate **must not depend on CEF.** The CEF layer converts
  its callback values into plain owned types (`AdapterRequest`, `RequestContext`,
  `FrameContext`) and keeps CEF objects out of the crate. Preserve that boundary
  — it's what keeps the ad blocker unit-testable.
- Never call into CEF while holding the `SHARED` mutex in `cef_host.rs`. Win32
  callbacks can synchronously re-enter the module; clone what you need, drop the
  lock, then call. Existing code documents this at each call site — follow it.
- Filtering runs on the CEF IO thread and must stay allocation-light and
  non-blocking. Expensive work (compiling filter lists, SQLite writes, list
  downloads) happens on dedicated worker threads and is published atomically via
  `ArcSwap` / versioned snapshots. Keep it that way.
- Match the surrounding comment style: comments explain *why* (especially the
  platform-specific window/focus quirks), not *what*.

**TypeScript / React**

- Follow the existing ESLint config (`shell/eslint.config.js`); run
  `bun run lint`.
- Routing is **hash-based** (`HashRouter`) so the custom `miko://` scheme and
  `file://` hosts work without a server. Don't introduce path-based routes.
- The shell is bundled to a **single `index.html`** (`vite-plugin-singlefile`)
  because the Rust host embeds and serves exactly one file. Don't add runtime
  external asset fetches that assume a normal web server.
- New user-facing strings go through i18n (`shell/src/i18n/locales/`) for all
  four languages (en, th, zh, ja).

## Commit & PR flow

- Branch off `main`; don't commit directly to it.
- Keep commits focused; write imperative, present-tense messages.
- Before opening a PR: `cargo fmt`, `cargo clippy`, `cargo test -p trash_adblock`,
  and (for shell changes) `bun run lint && bun run build`.
- Describe the change and how you verified it. Call out anything touching the
  CEF↔GPUI focus/threading dance or the ad-block IO-thread path — those are the
  easiest places to introduce subtle regressions.
- Do not edit `crates/gpui/` as part of feature work; it's vendored upstream.

## Gotchas worth knowing early

- **DirectComposition is disabled on Windows** (`GPUI_DISABLE_DIRECT_COMPOSITION=1`
  set in `main`) because GPUI's DirectComposition root visual would cover the
  native CEF child HWND. The app uses GPUI's HWND swap-chain path instead.
- **Wayland is removed on Linux** (`WAYLAND_DISPLAY` unset) — CEF child windows
  need an X11 parent, so the app prefers X11/Xwayland.
- The process is **re-entrant as CEF helpers.** `main` calls `cef_host::init`,
  which returns `None` when the current process is a CEF sub-process (renderer,
  GPU, etc.); in that case the process does its CEF work and exits before any
  window is opened. Keep early-exit paths intact.
- The shell's production bundle is embedded via `include_bytes!` from
  `shell/dist/index.html`, so **the shell must be built before the Rust binary.**
  `build.py` handles the ordering.
