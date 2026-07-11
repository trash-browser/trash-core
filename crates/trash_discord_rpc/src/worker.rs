//! Background Discord IPC worker with reconnect + update coalescing.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use discord_rich_presence::activity::{Activity, ActivityType, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};

use crate::presence::{PresenceStatus, format_presence};
use crate::resolve_client_id;

const MIN_UPDATE_INTERVAL: Duration = Duration::from_secs(2);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(8);

/// Configuration for [`DiscordRpc::start`].
#[derive(Debug, Clone)]
pub struct DiscordRpcConfig {
    /// Discord application client ID. Empty → try env / default / disable.
    pub client_id: String,
    /// Rich Presence large image asset key (upload in Discord Developer Portal).
    /// Empty skips the assets block (text-only presence).
    pub large_image: String,
    /// Hover text for the large image.
    pub large_text: String,
}

impl Default for DiscordRpcConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            large_image: "logo".into(),
            large_text: "Trash Browser".into(),
        }
    }
}

enum Cmd {
    Update(PresenceStatus),
    Clear,
    Shutdown,
}

/// Handle to a background Discord Rich Presence worker.
pub struct DiscordRpc {
    tx: Option<Sender<Cmd>>,
    join: Option<JoinHandle<()>>,
}

impl DiscordRpc {
    /// Start the worker. Returns a disabled handle if no client ID is configured.
    pub fn start(config: DiscordRpcConfig) -> Self {
        let Some(client_id) = resolve_client_id(&config.client_id) else {
            eprintln!(
                "[discord-rpc] disabled — set TRASH_DISCORD_CLIENT_ID or DISCORD_CLIENT_ID in main.rs"
            );
            return Self {
                tx: None,
                join: None,
            };
        };

        eprintln!("[discord-rpc] starting client_id={client_id}");
        let (tx, rx) = mpsc::channel();
        match thread::Builder::new()
            .name("trash-discord-rpc".into())
            .spawn(move || worker_loop(client_id, config, rx))
        {
            Ok(join) => Self {
                tx: Some(tx),
                join: Some(join),
            },
            Err(error) => {
                eprintln!("[discord-rpc] failed to spawn worker: {error}");
                Self {
                    tx: None,
                    join: None,
                }
            }
        }
    }

    /// Whether RPC is enabled (client ID resolved and worker spawned).
    pub fn enabled(&self) -> bool {
        self.tx.is_some()
    }

    /// Push a presence update (coalesced / rate-limited on the worker).
    pub fn update(&self, status: PresenceStatus) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Update(status));
        }
    }

    /// Clear the activity (e.g. before quit).
    pub fn clear(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Clear);
        }
    }

    /// Stop the worker and wait briefly for it to exit.
    pub fn shutdown(mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

impl Drop for DiscordRpc {
    fn drop(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(Cmd::Shutdown);
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn worker_loop(client_id: String, config: DiscordRpcConfig, rx: Receiver<Cmd>) {
    let session_start = unix_now_secs();
    let mut client = DiscordIpcClient::new(&client_id);
    let mut connected = false;
    let mut pending: Option<PresenceStatus> = None;
    let mut last_sent: Option<PresenceStatus> = None;
    let mut last_push = Instant::now()
        .checked_sub(MIN_UPDATE_INTERVAL)
        .unwrap_or_else(Instant::now);
    let mut next_reconnect = Instant::now();
    let mut warned_connect = false;

    loop {
        let wait = if pending.is_some() && connected {
            let elapsed = last_push.elapsed();
            if elapsed >= MIN_UPDATE_INTERVAL {
                Duration::from_millis(0)
            } else {
                MIN_UPDATE_INTERVAL - elapsed
            }
        } else if !connected {
            next_reconnect
                .saturating_duration_since(Instant::now())
                .max(Duration::from_millis(50))
        } else {
            Duration::from_secs(30)
        };

        match rx.recv_timeout(wait) {
            Ok(Cmd::Update(status)) => {
                pending = Some(status);
            }
            Ok(Cmd::Clear) => {
                pending = None;
                last_sent = None;
                if connected {
                    let _ = client.clear_activity();
                }
            }
            Ok(Cmd::Shutdown) => {
                if connected {
                    let _ = client.clear_activity();
                    let _ = client.close();
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if connected {
                    let _ = client.clear_activity();
                    let _ = client.close();
                }
                return;
            }
        }

        if !connected {
            if Instant::now() < next_reconnect {
                continue;
            }
            match client.connect() {
                Ok(()) => {
                    connected = true;
                    warned_connect = false;
                    eprintln!("[discord-rpc] connected");
                    // Force a refresh after reconnect.
                    if last_sent.is_some() && pending.is_none() {
                        pending = last_sent.clone();
                    }
                    last_push = Instant::now()
                        .checked_sub(MIN_UPDATE_INTERVAL)
                        .unwrap_or_else(Instant::now);
                }
                Err(error) => {
                    if !warned_connect {
                        eprintln!(
                            "[discord-rpc] connect failed ({error}); retrying every {}s — is Discord open?",
                            RECONNECT_INTERVAL.as_secs()
                        );
                        warned_connect = true;
                    }
                    next_reconnect = Instant::now() + RECONNECT_INTERVAL;
                    continue;
                }
            }
        }

        let Some(status) = pending.take() else {
            continue;
        };
        if last_sent.as_ref() == Some(&status) {
            continue;
        }
        if last_push.elapsed() < MIN_UPDATE_INTERVAL {
            pending = Some(status);
            continue;
        }

        match push_activity(&mut client, &config, &status, session_start) {
            Ok(()) => {
                let formatted = format_presence(&status);
                eprintln!(
                    "[discord-rpc] activity details={:?} state={:?}",
                    formatted.details, formatted.state
                );
                last_sent = Some(status);
                last_push = Instant::now();
            }
            Err(error) => {
                eprintln!("[discord-rpc] set_activity failed ({error}); reconnecting");
                connected = false;
                pending = Some(status);
                let _ = client.close();
                client = DiscordIpcClient::new(&client_id);
                next_reconnect = Instant::now() + RECONNECT_INTERVAL;
            }
        }
    }
}

fn push_activity(
    client: &mut DiscordIpcClient,
    config: &DiscordRpcConfig,
    status: &PresenceStatus,
    session_start: i64,
) -> Result<(), discord_rich_presence::error::Error> {
    let formatted = format_presence(status);
    let timestamps = Timestamps::new().start(session_start);

    if !config.large_image.trim().is_empty() {
        let assets = Assets::new()
            .large_image(config.large_image.as_str())
            .large_text(config.large_text.as_str());
        let activity = Activity::new()
            .activity_type(ActivityType::Playing)
            .details(formatted.details.as_str())
            .state(formatted.state.as_str())
            .assets(assets)
            .timestamps(timestamps.clone());
        if client.set_activity(activity).is_ok() {
            return Ok(());
        }
        // Missing Rich Presence art asset — fall back to text-only.
        eprintln!(
            "[discord-rpc] activity with image {:?} failed; retrying text-only",
            config.large_image
        );
    }

    let activity = Activity::new()
        .activity_type(ActivityType::Playing)
        .details(formatted.details.as_str())
        .state(formatted.state.as_str())
        .timestamps(timestamps);
    client.set_activity(activity)
}

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
