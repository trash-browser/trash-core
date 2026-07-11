//! Discord Rich Presence for Trash Browser.
//!
//! Shows a browser-style activity while Discord is running:
//! - **details** — page title (or "New Tab")
//! - **state** — `Browsing example.com · 3 tabs` / `Start page`
//! - **timer** — session elapsed since [`DiscordRpc::start`]
//! - **art** — large image asset key (upload as `logo` in the Discord app)
//!
//! Works on Linux (Unix sockets) and Windows (named pipes). CEF-agnostic: the
//! browser pushes [`PresenceStatus`] updates; this crate owns the IPC worker,
//! reconnect, and rate-limited `SET_ACTIVITY` calls.
//!
//! Set `TRASH_DISCORD_CLIENT_ID` (or [`DiscordRpcConfig::client_id`]) to your
//! Discord application ID. Empty / `"0"` disables RPC.

mod ipc;
mod presence;
mod worker;

pub use presence::{PresenceStatus, format_presence};
pub use worker::{DiscordRpc, DiscordRpcConfig};

/// Default Discord application client ID.
///
/// Override at runtime with `TRASH_DISCORD_CLIENT_ID`, or pass
/// [`DiscordRpcConfig::client_id`]. Create an application at
/// <https://discord.com/developers/applications> and upload a Rich Presence
/// image asset named `logo`.
pub const DEFAULT_CLIENT_ID: &str = "";

/// Resolve the client ID: non-empty config value, else `TRASH_DISCORD_CLIENT_ID`,
/// else [`DEFAULT_CLIENT_ID`].
pub fn resolve_client_id(configured: &str) -> Option<String> {
    let from_config = configured.trim();
    if !from_config.is_empty() && from_config != "0" {
        return Some(from_config.to_string());
    }
    match std::env::var("TRASH_DISCORD_CLIENT_ID") {
        Ok(id) => {
            let id = id.trim().to_string();
            if id.is_empty() || id == "0" {
                None
            } else {
                Some(id)
            }
        }
        Err(_) => {
            let id = DEFAULT_CLIENT_ID.trim();
            if id.is_empty() || id == "0" {
                None
            } else {
                Some(id.to_string())
            }
        }
    }
}
