# GPUI — Standalone Edition

> A hybrid immediate- and retained-mode, GPU-accelerated UI framework for Rust.

**GPUI Standalone Edition** is a fork of [Zed's GPUI](https://github.com/zed-industries/zed)
framework, extracted and maintained as an independent, standalone project. Where
upstream GPUI ships as a component of the Zed code editor, this edition is
organized into self-contained workspace crates so it can be consumed on its own —
without pulling in the rest of the Zed tree.

It keeps the same design goals as upstream: a fast, GPU-accelerated toolkit for
building a wide variety of desktop (and experimental web) applications in Rust.

> [!NOTE]
> This is a community fork and is **not** affiliated with or endorsed by
> Zed Industries, Inc. See [`NOTICE`](NOTICE) for attribution.

---

## Why a Standalone Edition?

- **Independent crates** — the framework is split into a workspace of focused
  crates (`crates/*`) that build on their own.
- **Fork-friendly** — versioned and published independently so you can depend on
  it directly.
- **Multi-target** — platform backends for Windows, macOS, and Linux, plus
  experimental `wgpu` and web renderers, and language-binding crates.

## Workspace layout

| Crate | Description |
| --- | --- |
| `gpui` (root) | Core framework: entities, views, elements, layout, styling. |
| `crates/gpui_platform` | Platform abstraction layer. |
| `crates/gpui_windows` | Windows backend. |
| `crates/gpui_macos` | macOS (Metal) backend. |
| `crates/gpui_linux` | Linux (Wayland / X11) backend. |
| `crates/gpui_wgpu` | Experimental `wgpu` renderer. |
| `crates/gpui_web` | Experimental web target. |
| `crates/gpui_macros` | Procedural macros. |
| `crates/gpui_util` | Shared utilities. |
| `crates/gpui_shared_string` | Cheap, shareable strings. |
| `crates/gpui_tokio` | Tokio integration. |
| `crates/gpui_python` | Python bindings. |
| `crates/gpui_dotnet` | .NET bindings. |

## Getting started

GPUI is pre-1.0 and under active development; expect breaking changes between
versions. Use the latest stable Rust.

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
gpui = "*"
```

Every application starts with an `Application`:

```rust
use gpui::*;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(|_| HelloWorld)
        })
        .unwrap();
    });
}
```

Browse the [`examples/`](examples) directory for runnable samples:

```sh
cargo run -p gpui --example hello_world
```

## The big picture

GPUI offers three registers you can mix depending on your needs:

- **State with entities.** Application state that communicates across your app is
  stored in `Entity`s, owned by GPUI and accessed through smart pointers. See the
  `app::context` module.
- **Declarative UI with views.** A view is an `Entity` that implements `Render`.
  GPUI calls `render` each frame on the root view, which builds and styles a tree
  of elements using a Tailwind-style API. See the `div` element.
- **Imperative UI with elements.** Elements are the building blocks and give you
  full control over layout and painting — ideal for large lists, custom editors,
  and anything bespoke. See the `element` module.

Additional services include user-defined **actions** (keybindings), platform
services on `App`, an async **executor** integrated with the platform event loop,
and the `#[gpui::test]` macro with `TestAppContext` for testing.

## Platform dependencies

### macOS

GPUI renders with Metal on macOS:

- Install [Xcode](https://apps.apple.com/us/app/xcode/id497799835) and its macOS
  components.
- Install the command line tools:

  ```sh
  xcode-select --install
  ```

- Point the tools at your Xcode install:

  ```sh
  sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer
  ```

### Linux

Install the usual Wayland/X11 and graphics development packages for your
distribution (Vulkan, Wayland, X11, and related `-dev` headers).

### Windows

A recent toolchain with the MSVC build tools is required.

## License

GPUI Standalone Edition is licensed under the **Apache License, Version 2.0**.

- [`LICENSE`](LICENSE) — the license for this fork.
- [`LICENSE-APACHE`](LICENSE-APACHE) — the retained upstream license and copyright.
- [`NOTICE`](NOTICE) — attribution notices required under the Apache License.

This fork is derived from GPUI, Copyright 2022 – 2025 Zed Industries, Inc.
Modifications are Copyright 2025 – 2026 arizkami and the GPUI Standalone Edition
contributors.
