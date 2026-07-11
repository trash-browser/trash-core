//! Bidirectional IPC between the React shell (`miko://shell/`) and the native
//! CEF/GPUI host.
//!
//! Transport:
//! - Shell → host: `POST/GET miko://shell/ipc` with a JSON-RPC-ish body
//! - Host → shell: `execute_java_script` (events + `window.trashCef` bootstrap)

use cef::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::cef_host;

/// JSON request from the shell.
#[derive(Debug, Clone, Deserialize)]
pub struct IpcRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

/// JSON response back to the shell.
#[derive(Debug, Clone, Serialize)]
pub struct IpcResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl IpcResponse {
    fn ok(id: Option<String>, result: Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    fn err(id: Option<String>, error: impl Into<String>) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error.into()),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_else(|_| b"{\"ok\":false,\"error\":\"encode\"}".to_vec())
    }
}

pub fn is_ipc_url(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("miko://shell/") else {
        return false;
    };
    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    path == "ipc" || path.starts_with("ipc/")
}

/// Handle an IPC HTTP-style request from the miko scheme handler.
pub fn handle_scheme_request(request: &Request) -> Vec<u8> {
    let url = CefString::from(&request.url()).to_string();
    let method = CefString::from(&request.method()).to_string();
    let body = read_post_body(request);
    let raw = if !body.is_empty() {
        String::from_utf8_lossy(&body).into_owned()
    } else if let Some(q) = url.split_once('?').map(|(_, q)| q) {
        // GET miko://shell/ipc?payload=<url-encoded-json>
        // or ?method=ping
        parse_query_payload(q)
    } else {
        String::new()
    };

    let response = if raw.is_empty() && method.eq_ignore_ascii_case("GET") {
        IpcResponse::ok(
            None,
            json!({
                "service": "trash-shell-ipc",
                "version": env!("CARGO_PKG_VERSION"),
            }),
        )
    } else {
        match serde_json::from_str::<IpcRequest>(&raw) {
            Ok(req) => dispatch(req),
            Err(err) => IpcResponse::err(None, format!("invalid ipc payload: {err}")),
        }
    };
    response.to_bytes()
}

fn parse_query_payload(query: &str) -> String {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next().unwrap_or("");
        let value = parts.next().unwrap_or("");
        if key == "payload" {
            return urlencoding_decode(value);
        }
        if key == "method" {
            let method = urlencoding_decode(value);
            return json!({ "method": method, "params": {} }).to_string();
        }
    }
    String::new()
}

fn urlencoding_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let h = hex_nibble(bytes[i + 1]);
                let l = hex_nibble(bytes[i + 2]);
                if let (Some(h), Some(l)) = (h, l) {
                    out.push((h << 4) | l);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn read_post_body(request: &Request) -> Vec<u8> {
    let Some(post) = request.post_data() else {
        return Vec::new();
    };
    let count = post.element_count();
    if count == 0 {
        return Vec::new();
    }
    let mut elements: Vec<Option<PostDataElement>> = (0..count).map(|_| None).collect();
    post.elements(Some(&mut elements));
    let mut out = Vec::new();
    for element in elements.into_iter().flatten() {
        let n = element.bytes_count();
        if n == 0 {
            continue;
        }
        let mut buf = vec![0u8; n];
        let read = element.bytes(n, buf.as_mut_ptr());
        out.extend_from_slice(&buf[..read.min(n)]);
    }
    out
}

fn dispatch(req: IpcRequest) -> IpcResponse {
    let id = req.id.clone();
    match req.method.as_str() {
        "ping" => IpcResponse::ok(
            id,
            json!({
                "pong": true,
                "version": env!("CARGO_PKG_VERSION"),
            }),
        ),
        "getVersions" => IpcResponse::ok(
            id,
            json!({
                "app": env!("CARGO_PKG_VERSION"),
                "cef": option_env!("TRASH_CEF_VERSION").unwrap_or("150"),
                "chromium": option_env!("TRASH_CHROMIUM_VERSION").unwrap_or("unknown"),
            }),
        ),
        "navigate" | "open" => match param_string(&req.params, "url") {
            Some(url) if !url.trim().is_empty() => {
                let target = normalize_shell_url(&url);
                cef_host::navigate(&target);
                IpcResponse::ok(id, json!({ "url": target }))
            }
            _ => IpcResponse::err(id, "missing params.url"),
        },
        "getAdblockStats" => match cef_host::current_adblock_statistics_json() {
            Ok(value) => IpcResponse::ok(id, value),
            Err(err) => IpcResponse::err(id, err),
        },
        "getAdblockStatus" => match cef_host::current_adblock_filter_status_json() {
            Ok(value) => IpcResponse::ok(id, value),
            Err(err) => IpcResponse::err(id, err),
        },
        "overlay.action" => {
            let action = req
                .params
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let overlay = req
                .params
                .get("overlay")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let payload = req.params.get("payload").cloned().unwrap_or(Value::Null);
            cef_host::emit_shell_overlay(action, overlay, payload);
            IpcResponse::ok(id, json!({ "accepted": true }))
        },
        other => IpcResponse::err(id, format!("unknown method: {other}")),
    }
}

fn param_string(params: &Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            // Allow `{ "url": ... }` at top level or bare string params.
            if key == "url" {
                params.as_str().map(str::to_string)
            } else {
                None
            }
        })
}

/// Same rules as the omnibox `normalize` in `main.rs`, kept local so the
/// scheme handler does not depend on GPUI chrome.
pub fn normalize_shell_url(input: &str) -> String {
    let s = input.trim();
    if s.is_empty() {
        return String::new();
    }
    if let Some(route) = s.strip_prefix("miko://#") {
        return format!("miko://shell/#{route}");
    }
    let has_scheme = [
        "http://",
        "https://",
        "file://",
        "miko://",
        "about:",
        "data:",
        "chrome://",
        "view-source:",
    ]
    .iter()
    .any(|p| s.starts_with(p));
    if has_scheme {
        return s.to_string();
    }
    let looks_like_domain =
        !s.contains(' ') && s.contains('.') && !s.starts_with('.') && !s.ends_with('.');
    if looks_like_domain {
        format!("https://{s}")
    } else {
        let mut encoded = String::new();
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                b' ' => encoded.push_str("%20"),
                _ => encoded.push_str(&format!("%{b:02X}")),
            }
        }
        format!("https://www.google.com/search?q={encoded}")
    }
}

/// JS bootstrap installed on every `miko://shell/` document load.
pub fn bridge_bootstrap_script() -> &'static str {
    r#"(() => {
  if (window.trashCef && window.trashCef.__native) return;
  const IPC_URL = 'miko://shell/ipc';
  let seq = 0;
  async function invoke(method, params) {
    const id = String(++seq);
    const body = JSON.stringify({ id, method, params: params ?? {} });
    try {
      const res = await fetch(IPC_URL, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      });
      return await res.json();
    } catch (err) {
      try {
        const q = encodeURIComponent(body);
        const res = await fetch(IPC_URL + '?payload=' + q, { method: 'GET' });
        return await res.json();
      } catch (err2) {
        console.warn('[trash-ipc] invoke failed', method, err2 || err);
        return { id, ok: false, error: String(err2 || err) };
      }
    }
  }
  function send(method, params) {
    void invoke(method, params);
  }
  function onOverlayAction(json) {
    try {
      const msg = typeof json === 'string' ? JSON.parse(json) : json;
      send('overlay.action', msg);
    } catch (e) {
      console.warn('[trash-ipc] bad overlay action', e);
    }
  }
  window.trashCef = {
    __native: true,
    invoke,
    send,
    onOverlayAction,
    navigate(url) { return invoke('navigate', { url }); },
    open(url) { return invoke('open', { url }); },
    getVersions() { return invoke('getVersions'); },
    getAdblockStats() { return invoke('getAdblockStats'); },
    getAdblockStatus() { return invoke('getAdblockStatus'); },
    ping() { return invoke('ping'); },
  };
  window.trashOverlay = {
    post(msg) { onOverlayAction(msg); },
  };
    window.dispatchEvent(new CustomEvent('trash-cef-ready'));
})();"#
}

pub fn inject_bridge(frame: &Frame) {
    frame.execute_java_script(
        Some(&CefString::from(bridge_bootstrap_script())),
        Some(&CefString::from("miko://shell/trash-cef-bridge.js")),
        1,
    );
}