# The React UI shell (`shell/`)

`shell/` is the browser's own UI — New Tab, Settings, Downloads, the Ad-block
manager, and transparent overlay popups (permission prompts, downloads, find,
auth, confirm). It's a Vite + React + TypeScript app that the Rust host embeds
and serves from the `miko://shell/` origin.

Stack: **Vite · React 19 · TypeScript · Tailwind CSS 4 · Framer Motion ·
react-router (hash) · i18next · Tabler icons.**

> `shell/README.md` is the app-focused readme (routes, overlay bridge, embed
> notes). This doc is the contributor orientation.

## How it ships

- `vite.config.ts` uses `vite-plugin-singlefile` with `base: './'`, so
  `bun run build` produces a **single** `shell/dist/index.html` with all JS/CSS
  inlined.
- The Rust host embeds that one file (`include_bytes!` in `cef_host.rs`) and a
  CEF scheme handler streams it for any `miko://shell/...` request.
- Because everything is one file and routing is hash-based, every route resolves
  to the same document — no server, no path rewrites.

Consequences for contributors:

- **Don't add runtime fetches of external assets** that assume a real web
  server. Inline what you need or import it so Vite bundles it.
- **Keep routing hash-based** (`HashRouter`). Path routes would break under the
  `miko://` and `file://` hosts.

## Dev workflow

```sh
cd shell
bun install
bun dev        # http://localhost:5173  (default route #/new-tab)
bun run lint   # ESLint
bun run build  # tsc -b typecheck + single-file production build → dist/
```

Use `npm`/`pnpm` if you prefer; the lockfile is Bun's. In dev you can hit routes
directly, e.g. `http://localhost:5173/#/settings/privacy`.

## Routing

Defined in [`src/App.tsx`](../shell/src/App.tsx) with `HashRouter` +
`AnimatePresence` page transitions:

| Hash | Page |
|------|------|
| `#/new-tab` | New Tab (default) |
| `#/download-manager` | Download manager |
| `#/adblock` | Ad-block manager |
| `#/settings` | Redirects to `#/settings/{DEFAULT_NAV}` (`appearance`) |
| `#/settings/:page` | Settings list for a section |
| `#/settings/:page/:sub` | Settings detail for one row |
| `#/overlay/:kind` | Transparent CEF overlay popup |

`useOverlayDocumentClass` toggles an `overlay-mode` class on `html`/`body` for
overlay routes so the document background is transparent (the popup floats over
a native host window).

## Source layout

```
src/
  App.tsx              # Hash routes + overlay-mode class toggling
  main.tsx             # React entry
  motion.tsx           # Framer Motion page/overlay variants
  index.css            # Theme, overflow rules, scrollbars, overlay transparency
  adblockStats.ts      # Host→shell ad-block stats/status store (useSyncExternalStore)
  components/          # Sidebar, setting rows, toggles, search, icons, language switcher
  data/                # settings.ts, managers.ts, subpages.ts, version.ts (mock/static data)
  i18n/                # i18next setup + locales/{en,th,zh,ja}.json
  overlay/             # Overlay popups + bridge.ts (host messaging)
  pages/               # NewTabPage, DownloadManagerPage, AdblockManagerPage, Settings*
  assets/              # images
  scripts/gen-sub-i18n.py   # helper that scaffolds settings sub-page i18n entries
```

Much of `data/` is **static/mock data** (settings rows, NTP shortcuts, sample
downloads) — the UI is real but not all of it is wired to live browser state
yet. The ad-block manager *is* live (see below).

## Host ⇆ shell contracts

There are two distinct channels between the native host and the shell.

### 1. Ad-block statistics (host → shell)

[`src/adblockStats.ts`](../shell/src/adblockStats.ts) exposes two React hooks,
`useAdblockStatistics()` and `useAdblockFilterStatus()`, built on
`useSyncExternalStore`. They:

- Read initial state from `window.__trashAdblockStats` /
  `window.__trashAdblockStatus` (seeded by the host).
- Subscribe to `trash-adblock-stats` / `trash-adblock-status` `CustomEvent`s.

The host (`cef_host::push_adblock_statistics`) executes JS on the main frame that
sets those globals and dispatches those events — but only while a `miko://shell/`
page is loaded and only when the underlying data version changed. The payload
shapes (`AdblockStatistics`, `AdblockFilterStatus`) mirror the Rust
`StatisticsSnapshot` / `FilterListStatus` `serde` output (camelCase). **If you
change one side's shape, change the other.**

### 2. Overlay actions (shell → host)

[`src/overlay/bridge.ts`](../shell/src/overlay/bridge.ts) defines
`postOverlayAction(overlay, type, payload?)`, which emits a message in four ways
for maximum host compatibility:

1. `window.trashOverlay.post(msg)` (object form)
2. `window.trashCef.onOverlayAction(json)` (preferred CEF/Rust binding)
3. `window.parent.postMessage(msg, "*")` (iframe embeds)
4. `CustomEvent("trash-overlay", { detail: msg })`

Message shape:

```json
{ "source": "trash-overlay", "type": "allow", "overlay": "permission",
  "payload": { "origin": "https://example.com", "type": "camera" } }
```

`type` ∈ `close · allow · deny · once · save · open · cancel · show · hide`.
Overlay kinds are registered in
[`src/overlay/OverlayPage.tsx`](../shell/src/overlay/OverlayPage.tsx):
`download · permission · confirm · auth · find`.

## Internationalization

[`src/i18n/index.ts`](../shell/src/i18n/index.ts) initializes i18next with four
bundled locales — **en, th, zh, ja** — detects the initial language from
`localStorage` then `navigator.language`, and persists the user's choice.
Add new UI strings to **all four** `locales/*.json` files. Language is switchable
from **Settings → Languages**.

`scripts/gen-sub-i18n.py` scaffolds the settings sub-page string keys; it's a
convenience for keeping the settings tree's i18n entries consistent, not part of
the build.

## Version metadata

[`src/data/version.ts`](../shell/src/data/version.ts) reads `VITE_APP_VERSION`,
`VITE_CEF_VERSION`, `VITE_CHROMIUM_VERSION` (injected by `build.py`) for the
About page. Standalone `bun run build` uses `dev`/`unknown` fallbacks.

## Conventions

- Run `bun run lint` before pushing; keep TypeScript strict (build runs `tsc -b`).
- Keep the single-file constraint in mind (no external runtime asset assumptions).
- New user-visible text → i18n keys in all locales, not hard-coded strings.
- Match the existing Tailwind + Framer Motion patterns; overlays use the shared
  `motion.tsx` variants and the transparent `overlay-mode` document.
