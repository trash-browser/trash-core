# Trash Browser — Contributor Documentation

Trash Browser is a small native web browser: a **GPUI** chrome (tabs, toolbar,
omnibox, window controls) drawn in Rust, hosting a **CEF** (Chromium Embedded
Framework) content window as a native child, with a **React** UI shell served
from a custom `miko://` scheme and a **native ad-block engine** running in the
browser process.

```
┌─────────────────────────────────────────────────────────────┐
│  trashbrowser-gpui (Rust binary)                             │
│                                                              │
│  ┌────────────────────────┐   ┌──────────────────────────┐  │
│  │ GPUI chrome (src/)     │   │ CEF content browser      │  │
│  │ tabs · omnibox · nav   │   │ (native child window)    │  │
│  │ window controls        │◄─►│  Chromium page           │  │
│  └────────────────────────┘   └───────────┬──────────────┘  │
│         ▲  events / commands              │ request/response │
│         │                                 ▼                  │
│  ┌──────┴───────────────┐    ┌────────────────────────────┐ │
│  │ cef_host.rs          │◄──►│ trash_adblock crate        │ │
│  │ CEF ⇆ GPUI bridge    │    │ engine · policy · stats    │ │
│  └──────────────────────┘    └────────────────────────────┘ │
│                                                              │
│  miko://shell/  ──serves──►  shell/dist/index.html (React)   │
└─────────────────────────────────────────────────────────────┘
```

## Documents

| Doc | What's inside |
|-----|---------------|
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setup, build, test, coding conventions, PR flow |
| [architecture.md](architecture.md) | How the process, windows, threads, and crates fit together |
| [building.md](building.md) | Detailed build/toolchain notes (Windows & Linux) |
| [adblock.md](adblock.md) | The `trash_adblock` crate: engine, policy, YouTube sanitizer, statistics |
| [shell.md](shell.md) | The React UI shell, routing, overlays, and the host↔shell bridge |

## Repository layout

```
trash-core/
├── Cargo.toml              # workspace binary: trashbrowser-gpui
├── build.rs                # embeds icons/version + generates Tabler icon asset map
├── build.py                # downloads CEF, builds the shell, runs cargo
├── src/                    # GPUI chrome + CEF host
│   ├── main.rs             #   window, tabs, toolbar, omnibox wiring, URL normalize
│   ├── cef_host.rs         #   CEF init, message pump, client handlers, adblock hookup
│   ├── content.rs          #   invisible surface that positions the CEF child window
│   └── omnibox.rs          #   text-input element for the address bar
├── crates/
│   ├── trash_adblock/      # native ad blocker (no CEF dependency)
│   └── gpui/               # vendored GPUI (IGNORE — upstream, not edited here)
└── shell/                  # React/Vite UI served from miko://shell/
```

> **Note:** `crates/gpui/` is a vendored copy of the GPUI UI framework. Treat it
> as a third-party dependency — contribution docs here do not cover it, and
> changes should generally not be made inside it.
