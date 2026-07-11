//! Cross-platform Discord IPC (Unix sockets + Windows named pipes).

use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use crate::presence::FormattedPresence;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

#[derive(Debug)]
pub enum IpcError {
    Connect,
    Io(std::io::Error),
    Protocol(&'static str),
}

impl std::fmt::Display for IpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect => write!(f, "Discord IPC not found"),
            Self::Io(error) => write!(f, "{error}"),
            Self::Protocol(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<std::io::Error> for IpcError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Connected Discord IPC client (browser-process Rich Presence).
pub struct DiscordIpc {
    client_id: String,
    stream: Stream,
}

enum Stream {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    Pipe(std::fs::File),
}

impl DiscordIpc {
    pub fn connect(client_id: impl Into<String>) -> Result<Self, IpcError> {
        let client_id = client_id.into();
        let stream = open_discord_stream()?;
        let mut client = Self { client_id, stream };
        client.handshake()?;
        Ok(client)
    }

    pub fn set_activity(
        &mut self,
        presence: &FormattedPresence,
        session_start: i64,
        large_image: Option<&str>,
        large_text: &str,
    ) -> Result<(), IpcError> {
        let mut activity = json!({
            "details": presence.details,
            "state": presence.state,
            "timestamps": { "start": session_start },
            "type": 0,
        });
        if let Some(image) = large_image.filter(|s| !s.is_empty()) {
            activity["assets"] = json!({
                "large_image": image,
                "large_text": large_text,
            });
        }

        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": activity,
            },
            "nonce": nonce(),
        });
        self.send(OP_FRAME, &payload)
    }

    pub fn clear_activity(&mut self) -> Result<(), IpcError> {
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": {
                "pid": std::process::id(),
                "activity": Value::Null,
            },
            "nonce": nonce(),
        });
        self.send(OP_FRAME, &payload)
    }

    pub fn close(&mut self) {
        let _ = self.send(OP_CLOSE, &json!({}));
    }

    fn handshake(&mut self) -> Result<(), IpcError> {
        let payload = json!({
            "v": 1,
            "client_id": self.client_id,
        });
        self.send(OP_HANDSHAKE, &payload)?;
        let _ = self.recv()?;
        Ok(())
    }

    fn send(&mut self, opcode: u32, payload: &Value) -> Result<(), IpcError> {
        let body = payload.to_string();
        let mut packet = Vec::with_capacity(8 + body.len());
        packet.extend_from_slice(&opcode.to_le_bytes());
        packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
        packet.extend_from_slice(body.as_bytes());
        self.stream.write_all(&packet)?;
        Ok(())
    }

    fn recv(&mut self) -> Result<(u32, Value), IpcError> {
        let mut header = [0u8; 8];
        self.stream.read_exact(&mut header)?;
        let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let length = u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        if length > 1 << 20 {
            return Err(IpcError::Protocol("Discord IPC frame too large"));
        }
        let mut body = vec![0u8; length];
        self.stream.read_exact(&mut body)?;
        let text = std::str::from_utf8(&body).map_err(|_| IpcError::Protocol("invalid utf-8"))?;
        let json = serde_json::from_str(text).map_err(|_| IpcError::Protocol("invalid json"))?;
        Ok((opcode, json))
    }
}

impl Stream {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), IpcError> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.write_all(buf).map_err(IpcError::from),
            #[cfg(windows)]
            Self::Pipe(pipe) => pipe.write_all(buf).map_err(IpcError::from),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), IpcError> {
        match self {
            #[cfg(unix)]
            Self::Unix(stream) => stream.read_exact(buf).map_err(IpcError::from),
            #[cfg(windows)]
            Self::Pipe(pipe) => pipe.read_exact(buf).map_err(IpcError::from),
        }
    }
}

fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("trash-{nanos}-{}", std::process::id())
}

#[cfg(unix)]
fn open_discord_stream() -> Result<Stream, IpcError> {
    use std::os::unix::net::UnixStream;
    use std::path::PathBuf;

    let mut bases = Vec::new();
    for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
        if let Ok(value) = std::env::var(key) {
            if !value.is_empty() {
                bases.push(PathBuf::from(value));
            }
        }
    }
    bases.push(PathBuf::from("/tmp"));

    // Flatpak / snap Discord layouts under the runtime dir.
    let mut candidates = Vec::new();
    for base in &bases {
        for i in 0..10 {
            candidates.push(base.join(format!("discord-ipc-{i}")));
            candidates.push(base.join(format!("app/com.discordapp.Discord/discord-ipc-{i}")));
            candidates.push(base.join(format!("app/dev.vencord.Vesktop/discord-ipc-{i}")));
            candidates.push(base.join(format!("snap.discord/discord-ipc-{i}")));
        }
    }

    for path in candidates {
        if let Ok(stream) = UnixStream::connect(&path) {
            let _ = stream.set_nonblocking(false);
            return Ok(Stream::Unix(stream));
        }
    }
    Err(IpcError::Connect)
}

#[cfg(windows)]
fn open_discord_stream() -> Result<Stream, IpcError> {
    use std::fs::OpenOptions;
    use std::os::windows::fs::OpenOptionsExt;

    // GENERIC_READ | GENERIC_WRITE — required for Discord named pipes.
    // (discord-rich-presence used access_mode(0x3), which often fails on Windows.)
    const GENERIC_READ_WRITE: u32 = 0xC000_0000;

    for i in 0..10 {
        for path in [
            format!(r"\\.\pipe\discord-ipc-{i}"),
            format!(r"\\?\pipe\discord-ipc-{i}"),
        ] {
            match OpenOptions::new()
                .read(true)
                .write(true)
                .access_mode(GENERIC_READ_WRITE)
                .open(&path)
            {
                Ok(file) => return Ok(Stream::Pipe(file)),
                Err(error) if error.raw_os_error() == Some(231) => {
                    // ERROR_PIPE_BUSY — brief wait then retry this path.
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if let Ok(file) = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .access_mode(GENERIC_READ_WRITE)
                        .open(&path)
                    {
                        return Ok(Stream::Pipe(file));
                    }
                }
                Err(_) => continue,
            }
        }
    }
    Err(IpcError::Connect)
}
