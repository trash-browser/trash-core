/**
 * Bridge for CEF / Rust host.
 *
 * Example (Rust/CEF):
 *   load_url("http://localhost:5173/#/overlay/permission?type=camera&origin=https://site.com")
 *   bind `trashCef.onOverlayAction(json)`
 *
 * Message shape:
 *   { source: "trash-overlay", type, overlay, payload? }
 */

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

declare global {
  interface Window {
    trashOverlay?: {
      post: (msg: OverlayMessage) => void;
    };
    trashCef?: {
      onOverlayAction?: (json: string) => void;
    };
  }
}

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

  window.trashOverlay?.post(msg);
  window.trashCef?.onOverlayAction?.(JSON.stringify(msg));
  window.parent?.postMessage(msg, "*");
  window.dispatchEvent(new CustomEvent("trash-overlay", { detail: msg }));
}
