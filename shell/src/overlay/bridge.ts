/**
 * Bridge for CEF / Rust host overlay actions.
 *
 * Prefers `window.trashCef` (injected by the native host). Falls back to
 * postMessage / CustomEvent for iframe or Vite-only embeds.
 */

import { send } from "../native/ipc";

export type OverlayAction =
  | "close"
  | "allow"
  | "deny"
  | "once"
  | "save"
  | "open"
  | "cancel"
  | "show"
  | "hide";

export type OverlayMessage = {
  source: "trash-overlay";
  type: OverlayAction;
  overlay: string;
  payload?: Record<string, unknown>;
};

export function postOverlayAction(
  overlay: string,
  type: OverlayAction,
  payload?: Record<string, unknown>,
) {
  const msg: OverlayMessage = {
    source: "trash-overlay",
    type,
    overlay,
    payload,
  };

  // Preferred: native IPC (trashCef / miko://shell/ipc).
  if (window.trashCef?.onOverlayAction) {
    window.trashCef.onOverlayAction(msg);
  } else {
    send("overlay.action", msg);
  }

  window.trashOverlay?.post(msg);
  window.parent?.postMessage(msg, "*");
  window.dispatchEvent(new CustomEvent("trash-overlay", { detail: msg }));
}
