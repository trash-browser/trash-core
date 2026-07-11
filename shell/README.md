# trash-shell

React UI shell for **Trash Browser** — Chrome-style settings, new tab, downloads, ad-block, and transparent overlay popups for CEF / Rust embeds.

Stack: Vite · React · TypeScript · Tailwind CSS 4 · Framer Motion · react-router (hash) · i18next · Tabler icons

## Setup

```bash
cd trash-shell
bun install
bun dev
```

Open [http://localhost:5173](http://localhost:5173) — default route is `#/new-tab`.

| Script        | What it does              |
|---------------|---------------------------|
| `bun dev`     | Dev server + HMR          |
| `bun run build` | Typecheck + production build → `dist/` |
| `bun run preview` | Serve `dist/` locally  |
| `bun run lint` | ESLint                    |

Use `npm` / `pnpm` if you prefer; lockfile is Bun’s.

## Routes

Hash routing (`HashRouter`) so CEF / file / custom-scheme hosts work without server-side path rewrites.

| Hash | Page |
|------|------|
| `#/new-tab` | New Tab |
| `#/download-manager` | Download Manager |
| `#/adblock` | Ad-block Manager |
| `#/settings` | Redirects to default settings page |
| `#/settings/:page` | Settings list (e.g. `#/settings/you`) |
| `#/settings/:page/:sub` | Settings detail for one row |
| `#/overlay/:kind` | Transparent CEF overlay popup |

Examples in development:

```
http://localhost:5173/#/new-tab
http://localhost:5173/#/settings/privacy
http://localhost:5173/#/settings/languages/preferred
```

The native CEF host embeds the single-file production bundle and serves it
from the standard `miko://shell/` origin:

```
miko://shell/#/new-tab
miko://shell/#/settings
miko://shell/#/settings/privacy
miko://shell/#/settings/languages/preferred
```

The omnibox also accepts shorthand such as `miko://#/settings` and normalizes
it to `miko://shell/#/settings`.

## Overlay popups (CEF / Rust)

Load overlays in a **transparent** host window. The shell adds `overlay-mode` on `html` / `body` so the background is clear.

| Kind | Example |
|------|---------|
| `download` | `#/overlay/download?name=file.zip&size=12MB&from=cdn.example` |
| `permission` | `#/overlay/permission?type=camera&origin=https://example.com` |
| `confirm` | `#/overlay/confirm?title=Reset&body=Continue?&danger=1` |
| `auth` | `#/overlay/auth?host=intranet.local` |
| `find` | `#/overlay/find` |

Permission `type`: `camera` · `microphone` · `location` · `notifications` · `generic`

### Host bridge

User actions are posted as JSON:

```json
{
  "source": "trash-overlay",
  "type": "allow",
  "overlay": "permission",
  "payload": { "origin": "https://example.com", "type": "camera" }
}
```

`type` values include: `close`, `allow`, `deny`, `once`, `save`, `open`, `cancel`, `show`, `hide`.

Bind from the native host (any one is enough):

1. **`window.trashCef.onOverlayAction(json: string)`** — preferred for CEF/Rust bindings  
2. **`window.trashOverlay.post(msg)`** — object form  
3. **`window.postMessage` / `parent.postMessage`** — iframe embeds  
4. **`CustomEvent("trash-overlay")`** — `detail` is the message object  

Rust sketch:

```rust
// Load popup
webview.load_url("http://127.0.0.1:5173/#/overlay/permission?type=camera&origin=https://site.com");

// Bind JS → Rust (exact API depends on your CEF wrapper)
// trashCef.onOverlayAction = |json| { /* parse & close popup */ };
```

Production: the Rust host embeds `dist/index.html` and serves it from
`miko://shell/`; no shell files are required beside the executable.

## Languages

UI strings live in `src/i18n/locales/`:

- `en` — English  
- `th` — Thai  
- `zh` — Chinese  
- `ja` — Japanese  

Switch language from **Settings → Languages**.

## Project layout

```
src/
  App.tsx              # Hash routes + overlay-mode class
  motion.tsx           # Page / overlay motion variants
  index.css            # Theme, overflow, scrollbars, overlay transparency
  components/          # Sidebar, rows, toggles, search
  data/                # Settings + manager mock data
  i18n/                # Locales
  overlay/             # CEF popups + bridge.ts
  pages/               # New Tab, Downloads, Ad-block, Settings
```

## Embed notes

- Prefer **hash URLs** from the native side (`#/…`).  
- Full pages use the dark Chrome shell background; overlays use a transparent document.  
- Nested regions scroll; document overflow is clipped (`index.css`).  
- After `bun run build`, serve `dist/index.html` + assets from CEF or any static file host.
