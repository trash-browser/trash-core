/**
 * Shell ↔ native IPC client.
 *
 * In the CEF host, `window.trashCef` is injected on `miko://shell/` load and
 * talks to `miko://shell/ipc`. In Vite dev (no host), this module installs a
 * small fallback that still works for local UI work.
 */

export type IpcResponse<T = unknown> = {
  id?: string;
  ok: boolean;
  result?: T;
  error?: string;
};

export type Versions = {
  app: string;
  cef: string;
  chromium: string;
};

export type SettingValue = boolean | string | number;
export type SettingsSnapshot = Record<string, SettingValue>;
export type DownloadPreferences = { location: string; askEveryTime: boolean };
export type HistoryEntry = {
  id: number;
  url: string;
  title: string;
  visited_at: number;
  visit_count: number;
};
export type NativeDownload = {
  id: number;
  name: string;
  url: string;
  path: string;
  totalBytes: number;
  receivedBytes: number;
  currentSpeed: number;
  percentComplete: number;
  status: "complete" | "progress" | "paused" | "failed" | "canceled";
  startedAt: number;
};

declare global {
  interface Window {
    trashCef?: {
      __native?: boolean;
      invoke: (method: string, params?: Record<string, unknown>) => Promise<IpcResponse>;
      send: (method: string, params?: Record<string, unknown>) => void;
      onOverlayAction?: (json: string | object) => void;
      navigate?: (url: string) => Promise<IpcResponse>;
      open?: (url: string) => Promise<IpcResponse>;
      getVersions?: () => Promise<IpcResponse<Versions>>;
      getAdblockStats?: () => Promise<IpcResponse>;
      getAdblockStatus?: () => Promise<IpcResponse>;
      ping?: () => Promise<IpcResponse>;
    };
    trashOverlay?: {
      post: (msg: unknown) => void;
    };
  }
}

const IPC_URL = "miko://shell/ipc";

async function fetchInvoke(
  method: string,
  params: Record<string, unknown> = {},
): Promise<IpcResponse> {
  const id = String(Date.now());
  const body = JSON.stringify({ id, method, params });
  try {
    const res = await fetch(`${IPC_URL}?payload=${encodeURIComponent(body)}`, {
      method: "GET",
      cache: "no-store",
    });
    return (await res.json()) as IpcResponse;
  } catch (error) {
    return { id, ok: false, error: String(error) };
  }
}

function installDevFallback() {
  if (window.trashCef?.__native) return;
  if (window.trashCef?.invoke) return;

  window.trashCef = {
    __native: false,
    invoke: fetchInvoke,
    send(method, params) {
      void fetchInvoke(method, params);
    },
    onOverlayAction(json) {
      try {
        const msg = typeof json === "string" ? JSON.parse(json) : json;
        void fetchInvoke("overlay.action", msg as Record<string, unknown>);
      } catch {
        /* ignore */
      }
    },
    navigate(url) {
      return fetchInvoke("navigate", { url });
    },
    open(url) {
      return fetchInvoke("open", { url });
    },
    getVersions() {
      return fetchInvoke("getVersions") as Promise<IpcResponse<Versions>>;
    },
    getAdblockStats() {
      return fetchInvoke("getAdblockStats");
    },
    getAdblockStatus() {
      return fetchInvoke("getAdblockStatus");
    },
    ping() {
      return fetchInvoke("ping");
    },
  };

  window.trashOverlay = {
    post(msg) {
      window.trashCef?.onOverlayAction?.(msg as object);
    },
  };
}

installDevFallback();

export function isNativeHost(): boolean {
  return Boolean(window.trashCef?.__native);
}

export async function invoke<T = unknown>(
  method: string,
  params: Record<string, unknown> = {},
): Promise<IpcResponse<T>> {
  if (!window.trashCef?.invoke) {
    return { ok: false, error: "trashCef unavailable" };
  }
  return (await window.trashCef.invoke(method, params)) as IpcResponse<T>;
}

export function send(method: string, params: Record<string, unknown> = {}) {
  window.trashCef?.send?.(method, params);
}

/** Navigate the active CEF tab (same rules as the omnibox). */
export async function navigate(url: string): Promise<boolean> {
  const res = await invoke("navigate", { url });
  if (res.ok) return true;
  // Dev / fallback: same-tab navigation.
  window.location.href = url;
  return false;
}

/** Resolve a search box query the same way the host omnibox would. */
export function resolveSearchQuery(query: string): string {
  const q = query.trim();
  if (!q) return "";
  const hasScheme = /^(https?:|file:|miko:|about:|chrome:|view-source:)/i.test(q);
  if (hasScheme) return q;
  const looksLikeDomain =
    !q.includes(" ") && q.includes(".") && !q.startsWith(".") && !q.endsWith(".");
  if (looksLikeDomain) return `https://${q}`;
  return `https://www.google.com/search?q=${encodeURIComponent(q)}`;
}

export async function getVersions(): Promise<Versions> {
  const res = await invoke<Versions>("getVersions");
  if (res.ok && res.result) return res.result;
  return {
    app: "dev",
    cef: "unknown",
    chromium: "unknown",
  };
}

export async function getSettings(): Promise<SettingsSnapshot> {
  const response = await invoke<{ values: SettingsSnapshot }>("settings.getAll");
  return response.ok && response.result ? response.result.values : {};
}

export async function setSetting(
  key: string,
  value: SettingValue,
): Promise<boolean> {
  return (await invoke("settings.set", { key, value })).ok;
}

export async function resetSettings(): Promise<boolean> {
  return (await invoke("settings.reset")).ok;
}

export async function getDownloadPreferences(): Promise<DownloadPreferences> {
  const response = await invoke<DownloadPreferences>("downloads.getPreferences");
  return response.ok && response.result
    ? response.result
    : { location: "", askEveryTime: false };
}

export async function chooseDownloadLocation(): Promise<boolean> {
  return (await invoke("downloads.chooseLocation")).ok;
}

export async function getHistory(query = ""): Promise<HistoryEntry[]> {
  const response = await invoke<{ entries: HistoryEntry[] }>("history.list", {
    query,
    limit: 1000,
  });
  return response.ok && response.result ? response.result.entries : [];
}

export async function removeHistoryEntry(id: number): Promise<boolean> {
  return (await invoke("history.remove", { id })).ok;
}

export async function clearHistory(): Promise<boolean> {
  return (await invoke("history.clear")).ok;
}

export async function getDownloads(): Promise<NativeDownload[]> {
  const response = await invoke<{ items: NativeDownload[] }>("downloads.list");
  return response.ok && response.result ? response.result.items : [];
}

export async function downloadAction(
  id: number,
  action: "pause" | "resume" | "cancel" | "remove",
): Promise<boolean> {
  return (await invoke("downloads.action", { id, action })).ok;
}

export async function clearCompletedDownloads(): Promise<boolean> {
  return (await invoke("downloads.clearCompleted")).ok;
}

/** Wait until the host injects `trashCef` (or timeout in Vite). */
export function whenReady(timeoutMs = 2000): Promise<boolean> {
  if (window.trashCef?.__native) return Promise.resolve(true);
  return new Promise((resolve) => {
    const timer = window.setTimeout(() => resolve(false), timeoutMs);
    window.addEventListener(
      "trash-cef-ready",
      () => {
        window.clearTimeout(timer);
        resolve(true);
      },
      { once: true },
    );
  });
}
