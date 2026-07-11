# The `trash_adblock` crate

A native, browser-process ad blocker. It is **deliberately independent of CEF** —
the CEF layer translates its callback values into plain owned types and keeps
CEF objects out of the synchronous matching path. That boundary is what makes
the crate fully unit-testable (most tests in the repo live here and need neither
CEF nor GPUI).

Location: [`crates/trash_adblock/`](../crates/trash_adblock). Public surface is
re-exported from [`src/lib.rs`](../crates/trash_adblock/src/lib.rs).

## Module map

| Module | Responsibility |
|--------|----------------|
| `request.rs` | Core types: `ResourceType`, `RequestContext`, `AdBlockRequest`, `AdBlockDecision` |
| `engine.rs` | `AdBlockEngine` trait, `EngineSnapshot`, and `BraveAdBlockEngine` (Brave's `adblock-rust`) |
| `service.rs` | `AdBlockService` — the central check entry point, enable flag, bypass rules, stats emit |
| `adapter.rs` | `AdBlockAdapter` — turns owned CEF callback values into an `AdBlockRequest` and resolves document/source URLs |
| `navigation.rs` | `NavigationContextStore` — concurrent per-frame URL map (`DashMap`) |
| `policy.rs` | `SitePolicySnapshot` — per-host shield mode (Enabled / Disabled / Aggressive) |
| `statistics.rs` | Bounded lossy channel (`StatisticsSink`/`Receiver`) + `BlockEvent` |
| `storage.rs` | `StatisticsRecorder` — dedicated SQLite writer thread + snapshot for the UI |
| `subscriptions.rs` | `FilterListUpdater` — cached + live EasyList/EasyPrivacy download worker |
| `youtube.rs` | Response-body sanitizer for YouTube player/watch responses |

## Request evaluation flow

```
CEF on_before_resource_load
      │  (owned values only)
      ▼
AdapterRequest ── AdBlockAdapter::evaluate
      │  resolves document_url / source_url from the navigation store
      ▼
AdBlockRequest ── AdBlockService::check
      │  bypass? disabled site? youtube? → Allow
      ▼
EngineSnapshot::check (BraveAdBlockEngine)
      │
      ▼
AdBlockDecision  →  Allow | Block | RewriteUrl | Redirect
```

### `AdBlockAdapter::evaluate` (adapter.rs)

Resolves the two URLs the engine needs from the per-frame navigation context:

- **`document_url`** — for a main-frame navigation it's the request URL itself;
  otherwise the frame's stored top-level URL. This is what site policy is keyed
  on, so a disabled top-level site also disables blocking in its subframes.
- **`source_url`** — the request initiator if present, else the frame URL, else
  the document URL.

The navigation context exists because not every CEF callback carries a stable
top-level URL. `cef_host.rs` calls `update_navigation_context` on load start and
before each resource load to keep the store fresh, and removes a browser's frames
on `on_before_close`.

### `AdBlockService::check` (service.rs)

Short-circuits to `Allow` when any of these hold, before touching the engine:

- The service is disabled (`set_enabled(false)`).
- The URL uses a bypass scheme: `about:`, `blob:`, `chrome://`,
  `chrome-extension://`, `data:`, `devtools://`, `miko://`.
- The request is YouTube playback traffic on a YouTube document — those are
  handled by the **response sanitizer**, not by cancelling the network request
  (cancelling breaks playback).
- Site policy marks the document host as `Disabled`.

Otherwise it delegates to the current `EngineSnapshot`. A real `Block` records a
statistic (increments an atomic total and emits a `BlockEvent`).

### `BraveAdBlockEngine` (engine.rs)

Wraps Brave's `adblock-rust` `Engine` with `RuleTypes::NetworkOnly` (cosmetic
filtering is not wired up). `check` returns:

- `Block` when the engine says block. (Note: `adblock-rust` "redirect" resources
  are response *bodies*, not URLs, and there's no browser-owned resource registry
  yet, so a blocking match cancels.)
- `RewriteUrl(url)` for a rewritten URL (e.g. `$removeparam`).
- `Allow` otherwise, including when `Request::new` fails to parse (fail open —
  never break browsing on a parse error).

## Hot-swapping without locking the IO thread

Matching runs on the CEF IO thread and must not block. Two mechanisms keep
expensive work off it:

- **`ArcSwap<EngineSnapshot>`** — compiling filter lists into an engine is done
  on a worker thread (`BraveAdBlockEngine::from_filter_lists`), then published
  atomically with `AdBlockService::replace_engine`. Readers do a lock-free load.
  `EngineSnapshot` is a sized box so a trait-object engine can live behind
  `ArcSwap`.
- **`ArcSwap<SitePolicySnapshot>`** — same pattern for per-site rules
  (`replace_policies`).

`AdBlockServiceBuilder` constructs the service (engine, policies, statistics
sink, enabled flag). Defaults to an empty real engine.

## Statistics pipeline

Blocking on the IO thread must never stall, so recording is fully asynchronous
and **lossy under pressure by design**:

```
IO thread ──try_send──► bounded sync_channel ──► SQLite writer thread ──► snapshot
  (BlockEvent)          (drops if full)          (batched, WAL)          (versioned)
                                                                              │
                                          cef_host pumps latest() ──► JS event ──► shell
```

- `statistics.rs`: `StatisticsSink::try_send` never blocks; a full channel just
  increments a dropped counter. Capacity is set at construction
  (`statistics_channel(4096)` in `cef_host.rs`).
- `storage.rs`: `StatisticsRecorder::open` spawns a named thread
  (`adblock-stats-sqlite`) that batches events (up to `BATCH_LIMIT`, flush every
  `FLUSH_INTERVAL`) into a WAL SQLite database. It maintains a `blocked_requests`
  log and a `blocked_daily` rollup, and publishes a versioned
  `StatisticsSnapshot` (total / today / last-7-days / by-resource). On a DB error
  it flips `recording=false` and records `last_error`.
- `cef_host::push_adblock_statistics` (called from the pump) reads the latest
  snapshot + filter status, and only when the version changed and the current
  page is a `miko://shell/` page, executes JS dispatching `trash-adblock-stats`
  and `trash-adblock-status` events. The shell consumes them via
  `useSyncExternalStore`.

## Filter subscriptions

`subscriptions.rs`: `FilterListUpdater::start` spawns a worker
(`adblock-filter-updater`) that:

1. Loads any cached lists from disk and installs them immediately (fast start).
2. Downloads the default sources over HTTPS (`ureq`, 20 s timeout, 16 MiB cap):
   - EasyList — `https://easylist.to/easylist/easylist.txt`
   - EasyPrivacy — `https://easylist.to/easylist/easyprivacy.txt`
3. Validates each (`[Adblock Plus` header + >1000 lines), writes it to the cache
   atomically (temp file + rename), recompiles the engine, and publishes an
   `active` status (or `error` with details).

Status is exposed as a versioned `FilterListStatus` (`loading` / `active` /
`error`, lists-loaded count, last error) and pushed to the shell alongside stats.

## YouTube response sanitizing

YouTube serves ads inline in the same responses as the video, so cancelling
network requests breaks playback. Instead, `youtube.rs` rewrites the **response
body**:

- `classify_youtube_response(url, mime)` recognizes YouTube hosts:
  - `ApiJson` — `/youtubei/…` (and a few player helpers) with a JSON mime.
  - `WatchHtml` — watch / embed / home / search / channel HTML.
- `sanitize_youtube_response` removes ad keys (`adPlacements`, `playerAds`,
  `adSlots`, `adPlacementConfig`, `*AdRenderer`, …) from the JSON, recursively.
  For HTML it finds embedded `ytInitialPlayerResponse` **and** `ytInitialData`
  objects (brace-matched, string- and escape-aware) and sanitizes those in place.
- Network filters still cancel `pagead` / `doubleclick` / similar; only media and
  player payloads are exempt (rewritten instead of cancelled).

`cef_host.rs` drives this with a streaming `ResponseFilter`
(`YoutubeAdResponseFilter` + `YoutubeFilterState`): it buffers the body up to
`MAX_YOUTUBE_RESPONSE_BYTES` (32 MiB), sanitizes at end-of-input, and falls back
to passthrough if the body is larger. When it actually removes something, it
records a blocked-request statistic via `AdBlockService::record_blocked_request`.

## Site policy

`policy.rs`: `SiteShieldMode` is `Enabled` (default), `Disabled`, or
`Aggressive` (defined but not yet differentiated in matching). Hosts are
normalized (trim, strip trailing dot, lowercase). `is_disabled_for_url` is what
`AdBlockService::check` consults to bypass a site.

## Testing

```sh
cargo test -p trash_adblock
```

Coverage includes engine matching + exceptions, service enable/bypass/YouTube
routing, adapter document-context resolution, navigation store cleanup, policy
host normalization, the lossy statistics channel, SQLite persistence round-trip,
and the YouTube JSON/HTML sanitizers. The one network test
(`official_sources_download_and_block_a_known_tracker`) is `#[ignore]`d — run it
with `-- --ignored` when touching the downloader.
