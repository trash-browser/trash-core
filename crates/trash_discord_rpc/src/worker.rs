//! Background Discord IPC worker with reconnect + update coalescing.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::ipc::DiscordIpc;
use crate::presence::{PresenceStatus, format_presence};
use crate::resolve_client_id;

const MIN_UPDATE_INTERVAL: Duration = Duration::from_secs(2);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(8);
/// Steady re-push so we stay above other apps (e.g. Zed) on Discord's status line.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
/// After connect / first publish, hammer SET_ACTIVITY for a few seconds.
const CLAIM_BURST_OFFSETS: &[Duration] = &[
    Duration::from_millis(200),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(7),
];

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
    let mut client: Option<DiscordIpc> = None;
    let mut pending: Option<PresenceStatus> = None;
    let mut last_sent: Option<PresenceStatus> = None;
    let mut last_push = Instant::now()
        .checked_sub(MIN_UPDATE_INTERVAL)
        .unwrap_or_else(Instant::now);
    let mut next_reconnect = Instant::now();
    let mut warned_connect = false;
    let mut claim_at: Vec<Instant> = Vec::new();
    let mut force_claim = false;

    loop {
        let now = Instant::now();
        let connected = client.is_some();
        let next_claim = claim_at.iter().copied().find(|t| *t > now);
        let wait = if pending.is_some() && connected {
            Duration::from_millis(0)
        } else if !connected {
            next_reconnect
                .saturating_duration_since(now)
                .max(Duration::from_millis(50))
        } else if let Some(at) = next_claim {
            at.saturating_duration_since(now)
                .max(Duration::from_millis(20))
        } else if last_sent.is_some() {
            REFRESH_INTERVAL
                .saturating_sub(last_push.elapsed())
                .max(Duration::from_millis(50))
        } else {
            Duration::from_secs(30)
        };

        match rx.recv_timeout(wait) {
            Ok(Cmd::Update(status)) => {
                pending = Some(status);
                force_claim = true;
                arm_claim_burst(&mut claim_at);
            }
            Ok(Cmd::Clear) => {
                pending = None;
                last_sent = None;
                claim_at.clear();
                force_claim = false;
                if let Some(ipc) = client.as_mut() {
                    let _ = ipc.clear_activity();
                }
            }
            Ok(Cmd::Shutdown) => {
                if let Some(mut ipc) = client.take() {
                    let _ = ipc.clear_activity();
                    ipc.close();
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(mut ipc) = client.take() {
                    let _ = ipc.clear_activity();
                    ipc.close();
                }
                return;
            }
        }

        if client.is_none() {
            if Instant::now() < next_reconnect {
                continue;
            }
            match DiscordIpc::connect(&client_id) {
                Ok(ipc) => {
                    client = Some(ipc);
                    warned_connect = false;
                    eprintln!("[discord-rpc] connected");
                    force_claim = true;
                    arm_claim_burst(&mut claim_at);
                    if pending.is_none() {
                        pending = last_sent.clone().or_else(|| {
                            Some(PresenceStatus {
                                title: "New Tab".into(),
                                url: String::new(),
                                tab_count: 1,
                            })
                        });
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

        let now = Instant::now();
        let before = claim_at.len();
        claim_at.retain(|t| *t > now);
        let due_now = claim_at.len() < before;

        if pending.is_none()
            && client.is_some()
            && let Some(status) = last_sent.clone()
            && (due_now || last_push.elapsed() >= REFRESH_INTERVAL)
        {
            pending = Some(status);
            if due_now {
                force_claim = true;
            }
        }

        let Some(status) = pending.take() else {
            continue;
        };
        let same_as_last = last_sent.as_ref() == Some(&status);
        let claiming = force_claim || due_now;
        if same_as_last && !claiming && last_push.elapsed() < REFRESH_INTERVAL {
            continue;
        }
        if !same_as_last && !claiming && last_push.elapsed() < MIN_UPDATE_INTERVAL {
            pending = Some(status);
            continue;
        }

        let Some(ipc) = client.as_mut() else {
            pending = Some(status);
            continue;
        };

        match push_activity(ipc, &config, &status, session_start, claiming) {
            Ok(()) => {
                if !same_as_last || claiming {
                    let formatted = format_presence(&status);
                    eprintln!(
                        "[discord-rpc] activity details={:?} state={:?}{}",
                        formatted.details,
                        formatted.state,
                        if claiming { " (claim)" } else { "" }
                    );
                }
                last_sent = Some(status);
                last_push = Instant::now();
                force_claim = false;
            }
            Err(error) => {
                eprintln!("[discord-rpc] set_activity failed ({error}); reconnecting");
                if let Some(mut old) = client.take() {
                    old.close();
                }
                pending = Some(status);
                claim_at.clear();
                force_claim = true;
                next_reconnect = Instant::now() + RECONNECT_INTERVAL;
            }
        }
    }
}

fn arm_claim_burst(claim_at: &mut Vec<Instant>) {
    let start = Instant::now();
    claim_at.clear();
    claim_at.extend(CLAIM_BURST_OFFSETS.iter().map(|d| start + *d));
}

fn push_activity(
    client: &mut DiscordIpc,
    config: &DiscordRpcConfig,
    status: &PresenceStatus,
    session_start: i64,
    claim: bool,
) -> Result<(), crate::ipc::IpcError> {
    let formatted = format_presence(status);
    // Clear-then-set helps Discord promote us over a long-running peer activity.
    if claim {
        let _ = client.clear_activity();
        thread::sleep(Duration::from_millis(40));
    }
    let image = config.large_image.trim();
    if !image.is_empty() {
        if client
            .set_activity(
                &formatted,
                session_start,
                Some(image),
                config.large_text.as_str(),
            )
            .is_ok()
        {
            return Ok(());
        }
        eprintln!(
            "[discord-rpc] activity with image {:?} failed; retrying text-only",
            config.large_image
        );
    }
    client.set_activity(&formatted, session_start, None, config.large_text.as_str())
}

fn unix_now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
