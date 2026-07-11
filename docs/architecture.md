# Architecture

Trash Browser is one native binary (`trashbrowser-gpui`) that composites two
worlds into a single window:

- **GPUI chrome** — the tab strip, toolbar, omnibox, and window controls,
  rendered by the [GPUI](../crates/gpui) framework in Rust.
- **CEF content** — a Chromium page hosted in a *native child window* parented
  under the GPUI window.

A native ad blocker (`trash_adblock`) runs inside the browser process and hooks
CEF's request pipeline. A React UI shell is embedded in the binary and served
over a custom `miko://` scheme for the browser's own pages (New Tab, Settings,
Downloads, Ad-block manager).

## Process model

`main()` in [`src/main.rs`](../src/main.rs):

1. On Windows, sets `GPUI_DISABLE_DIRECT_COMPOSITION=1` so GPUI uses the HWND
   swap-chain path — DirectComposition's root visual would otherwise paint over
   the native CEF child HWND.
2. On Linux/FreeBSD, unsets `WAYLAND_DISPLAY`/`WAYLAND_SOCKET` and builds GPUI
   with the X11 feature only so the window handle CEF parents under is an X11
   xid (not a Wayland surface).
3. Builds `cef::args::Args` (must outlive the process — `MainArgs` points into
   its argv storage) and calls `cef_host::init(&args)`.

`cef_host::init` runs `execute_process`. **CEF is multi-process**: the same
executable is re-launched as renderer/GPU/utility sub-processes. In those
sub-processes `init` returns `None`, and `main` returns immediately — no window,
no GPUI. Only in the *browser process* does `init` return an event `Receiver`
and let `main` proceed to open the GPUI window.

```
main()
 ├─ cef_host::init(&args)
 │    ├─ execute_process(...)
 │    │    └─ (sub-process) → returns None → main() returns, process exits
 │    └─ (browser process) → returns Some(Receiver<CefEvent>)
 └─ application().run(...)  → opens GPUI window, builds Root entity
```

## The two windows and how they stay aligned

The GPUI side never draws the web page. Instead, [`src/content.rs`](../src/content.rs)
defines `ContentSurface` — an invisible flex-fill GPUI `Element` that:

- Occupies the page area of the layout (below the toolbar).
- In `prepaint`, computes its device-pixel bounds and the parent window handle
  (X11 window id or Win32 `HWND`), then calls `cef_host::set_parent_bounds`.
- In `paint`, fills a dark placeholder quad that's visible only until CEF's
  first frame lands.

`cef_host::set_parent_bounds` reparents/resizes the CEF child window to match.
On Windows this is `SetWindowPos`; on X11 it's `XMoveResizeWindow`. So whenever
GPUI relayouts (resize, maximize, devtools, etc.), the CEF child follows.

Because the content is a real native child window, **input focus must be handed
off explicitly** between the two worlds — see below.

## CEF ⇆ GPUI event bridge

Communication is one channel plus direct command calls.

**CEF → GPUI (events).** CEF client callbacks run on the CEF UI thread and push
`CefEvent`s onto an `mpsc` channel:

| `CefEvent` | Source callback | Effect in `Root` |
|------------|-----------------|------------------|
| `Ready` | `on_context_initialized` | context ready marker |
| `Created` | `on_after_created` | hand keyboard focus to the page |
| `Closed` | `on_before_close` | — |
| `Address(url)` | `on_address_change` | update active tab URL + omnibox (unless user is editing) |
| `Title(title)` | `on_title_change` | update tab title + OS window title |
| `Loading{…}` | `on_loading_state_change` | spinner + back/forward enable state |
| `Favicon(png)` | `download_image` callback | tab favicon |
| `ContentFocus` | `on_got_focus` | stop guarding the omnibox; show page URL |

The browser uses CEF's **external message pump** (`external_message_pump = 1`).
GPUI spawns a background timer (~10 ms) that calls `cef_host::pump`, which runs
`do_message_loop_work()`, pushes any pending ad-block statistics to the shell,
and drains the event channel into `Root::on_cef_event`.

**GPUI → CEF (commands).** UI actions call free functions in `cef_host`:
`navigate`, `go_back`, `go_forward`, `reload`, `stop_load`, `focus_content`,
`focus_chrome`, `close_browser`. Each locks the shared state briefly, clones the
`Browser`, drops the lock, and calls CEF.

> **Threading rule:** never call into CEF while holding the `SHARED` mutex. Win32
> callbacks can synchronously re-enter `cef_host`, and re-locking would deadlock.
> The code consistently clones-then-unlocks; preserve that.

## Focus handoff

The trickiest part of the design. The CEF content is a native child HWND that
keeps OS keyboard focus, so typed keys never reach GPUI unless focus is actively
moved:

- Clicking the omnibox (`Root::toolbar`) sets `url_dirty = true`, focuses the
  GPUI field, and calls `cef_host::focus_chrome()` which `SetFocus`es the parent
  HWND — stealing OS focus back from CEF so keystrokes reach the omnibox.
- Navigating, switching/closing tabs, or clicking the page calls
  `cef_host::focus_content()` — it calls `host.set_focus(1)` *and* `SetFocus`es
  the child HWND, because `set_focus` alone isn't enough when the parent owns
  keyboard focus.
- `url_dirty` guards the omnibox so page-driven `Address` updates don't clobber
  what the user is typing. It's cleared on blur, on `ContentFocus`, and on submit.

## The `miko://` scheme and the shell

The browser's own pages are a React app (see [shell.md](shell.md)). The
production bundle is a single `shell/dist/index.html`, embedded into the binary
with `include_bytes!` and served by a CEF scheme handler:

- `miko` is registered as a standard, local, secure, CORS-enabled scheme in
  `on_register_custom_schemes`.
- `on_context_initialized` installs `MikoSchemeHandlerFactory` for the
  `miko://shell/` origin. Any `miko://shell/...` request streams the embedded
  HTML with `text/html` (the router is hash-based, so all routes resolve to the
  same document).
- The start page is `miko://shell/#/new-tab`. The omnibox accepts the shorthand
  `miko://#/settings` and `normalize()` rewrites it to `miko://shell/#/settings`.

Statistics flow back to the shell by executing JavaScript in the main frame that
dispatches `trash-adblock-stats` / `trash-adblock-status` custom events (only
when the current page is a `miko://shell/` page). The React side listens via
`useSyncExternalStore` in [`shell/src/adblockStats.ts`](../shell/src/adblockStats.ts).

## URL handling

`normalize()` in `src/main.rs` turns omnibox input into a load target:

- `miko://#route` → `miko://shell/#route` (shorthand expansion).
- Anything with a known scheme (`http`, `https`, `file`, `miko`, `about`,
  `data`, `chrome`, `view-source`) is passed through.
- A bare token that looks like a domain (has a dot, no spaces) becomes `https://…`.
- Everything else becomes a Google search query.

Empty input returns the start page.

## Ad blocking in the request path

`ContentClient` registers a `RequestHandler`. For each resource:

1. `resource_request_handler` builds a `RequestContext` and maps CEF's
   `ResourceType` to the crate's `ResourceType`, returning a
   `ContentResourceRequestHandler`.
2. `on_before_resource_load` refreshes the per-frame navigation context and calls
   `AdBlockAdapter::evaluate`, translating the crate's `AdBlockDecision` into a
   CEF `ReturnValue` (`CONTINUE`, `CANCEL`, or set-URL-then-continue).
3. `resource_response_filter` installs a streaming `YoutubeAdResponseFilter` for
   YouTube player/watch responses, which strips ad metadata from the body before
   it reaches the renderer.

See [adblock.md](adblock.md) for the crate internals and why YouTube is handled
by response sanitizing rather than network cancellation.

## Build-time code generation

[`build.rs`](../build.rs):

- On Windows, embeds the app icon (resource id 1, which GPUI's `LoadImageW`
  picks up) plus `VERSIONINFO` into the `.exe`. It deliberately does **not**
  embed a second manifest — GPUI already embeds one, and a duplicate `RT_MANIFEST`
  would collide (CVT1100).
- Walks `assets/tabler_icons/**` and generates `tabler_assets.rs` — a `PATHS`
  slice and a `get(path)` matcher backed by `include_bytes!`, so all SVG icons
  ship inside the binary. `Assets` in `main.rs` implements GPUI's `AssetSource`
  over this generated map.
