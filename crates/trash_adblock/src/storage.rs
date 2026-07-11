use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::{BlockEvent, StatisticsReceiver};

const SCHEMA_VERSION: i64 = 1;
const BATCH_LIMIT: usize = 256;
const FLUSH_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub enum StatisticsStoreError {
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    SpawnThread(std::io::Error),
    Sqlite(rusqlite::Error),
}

impl fmt::Display for StatisticsStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateDirectory { path, source } => {
                write!(formatter, "failed to create {}: {source}", path.display())
            }
            Self::SpawnThread(error) => write!(formatter, "failed to start SQLite worker: {error}"),
            Self::Sqlite(error) => write!(formatter, "SQLite error: {error}"),
        }
    }
}

impl std::error::Error for StatisticsStoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CreateDirectory { source, .. } => Some(source),
            Self::SpawnThread(error) => Some(error),
            Self::Sqlite(error) => Some(error),
        }
    }
}

impl From<rusqlite::Error> for StatisticsStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCount {
    pub resource_type: String,
    pub total: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsSnapshot {
    pub total_blocked: u64,
    pub blocked_today: u64,
    pub blocked_last_seven_days: u64,
    pub by_resource: Vec<ResourceCount>,
    pub recording: bool,
}

impl StatisticsSnapshot {
    pub fn unavailable() -> Self {
        Self::default()
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

struct SharedSnapshot {
    version: AtomicU64,
    snapshot: Mutex<StatisticsSnapshot>,
    last_error: Mutex<Option<String>>,
}

/// Owns a single SQLite writer thread. Request callbacks only interact with the
/// bounded [`crate::StatisticsSink`]; all database work happens here.
pub struct StatisticsRecorder {
    database_path: PathBuf,
    shared: Arc<SharedSnapshot>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl StatisticsRecorder {
    pub fn open(
        database_path: impl Into<PathBuf>,
        receiver: StatisticsReceiver,
    ) -> Result<Self, StatisticsStoreError> {
        let database_path = database_path.into();
        if let Some(parent) = database_path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| {
                StatisticsStoreError::CreateDirectory {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }

        let connection = Connection::open(&database_path)?;
        initialize_database(&connection)?;
        let initial_snapshot = read_snapshot(&connection)?;
        let shared = Arc::new(SharedSnapshot {
            version: AtomicU64::new(1),
            snapshot: Mutex::new(initial_snapshot),
            last_error: Mutex::new(None),
        });
        let stop = Arc::new(AtomicBool::new(false));
        let worker_shared = shared.clone();
        let worker_stop = stop.clone();
        let worker = thread::Builder::new()
            .name("adblock-stats-sqlite".into())
            .spawn(move || recorder_loop(connection, receiver, worker_shared, worker_stop))
            .map_err(StatisticsStoreError::SpawnThread)?;

        Ok(Self {
            database_path,
            shared,
            stop,
            worker: Some(worker),
        })
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub fn latest(&self) -> (u64, StatisticsSnapshot, Option<String>) {
        let version = self.shared.version.load(Ordering::Acquire);
        let snapshot = self
            .shared
            .snapshot
            .lock()
            .map(|snapshot| snapshot.clone())
            .unwrap_or_default();
        let error = self
            .shared
            .last_error
            .lock()
            .ok()
            .and_then(|error| error.clone());
        (version, snapshot, error)
    }

    pub fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for StatisticsRecorder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn recorder_loop(
    mut connection: Connection,
    receiver: StatisticsReceiver,
    shared: Arc<SharedSnapshot>,
    stop: Arc<AtomicBool>,
) {
    while !stop.load(Ordering::Acquire) {
        match receiver.recv_timeout(FLUSH_INTERVAL) {
            Ok(first) => {
                let mut batch = Vec::with_capacity(BATCH_LIMIT);
                batch.push(first);
                batch.extend(receiver.drain().take(BATCH_LIMIT - 1));
                if let Err(error) = persist_and_publish(&mut connection, &batch, &shared) {
                    mark_failed(&shared, error);
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let remaining: Vec<_> = receiver.drain().collect();
    if !remaining.is_empty()
        && let Err(error) = persist_and_publish(&mut connection, &remaining, &shared)
    {
        mark_failed(&shared, error);
    }
}

fn mark_failed(shared: &SharedSnapshot, error: rusqlite::Error) {
    if let Ok(mut snapshot) = shared.snapshot.lock() {
        snapshot.recording = false;
    }
    if let Ok(mut last_error) = shared.last_error.lock() {
        *last_error = Some(error.to_string());
    }
    shared.version.fetch_add(1, Ordering::Release);
}

fn initialize_database(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(&format!(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA busy_timeout = 5000;
         CREATE TABLE IF NOT EXISTS schema_info (
             version INTEGER NOT NULL
         );
         INSERT INTO schema_info(version)
             SELECT {SCHEMA_VERSION}
             WHERE NOT EXISTS (SELECT 1 FROM schema_info);
         CREATE TABLE IF NOT EXISTS blocked_requests (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             blocked_at_unix_ms INTEGER NOT NULL,
             browser_id INTEGER NOT NULL,
             frame_id TEXT NOT NULL,
             request_id INTEGER NOT NULL,
             resource_type TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS blocked_requests_time_idx
             ON blocked_requests(blocked_at_unix_ms);
         CREATE INDEX IF NOT EXISTS blocked_requests_resource_idx
             ON blocked_requests(resource_type);
         CREATE TABLE IF NOT EXISTS blocked_daily (
             day TEXT NOT NULL,
             resource_type TEXT NOT NULL,
             total INTEGER NOT NULL,
             PRIMARY KEY(day, resource_type)
         );
         INSERT INTO blocked_daily(day, resource_type, total)
             SELECT date(blocked_at_unix_ms / 1000, 'unixepoch', 'localtime'),
                    resource_type,
                    COUNT(*)
             FROM blocked_requests
             WHERE NOT EXISTS (SELECT 1 FROM blocked_daily)
             GROUP BY date(blocked_at_unix_ms / 1000, 'unixepoch', 'localtime'),
                      resource_type;"
    ))
}

fn persist_and_publish(
    connection: &mut Connection,
    events: &[BlockEvent],
    shared: &SharedSnapshot,
) -> rusqlite::Result<()> {
    let transaction = connection.transaction()?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO blocked_requests (
                blocked_at_unix_ms, browser_id, frame_id, request_id, resource_type
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut update_daily = transaction.prepare(
            "INSERT INTO blocked_daily(day, resource_type, total)
             VALUES (date(?1 / 1000, 'unixepoch', 'localtime'), ?2, 1)
             ON CONFLICT(day, resource_type)
             DO UPDATE SET total = total + 1",
        )?;
        for event in events {
            let resource_type = event.resource_type.storage_name();
            insert.execute(params![
                event.blocked_at_unix_ms,
                event.context.browser_id,
                event.context.frame_id,
                i64::try_from(event.context.request_id).unwrap_or(i64::MAX),
                resource_type,
            ])?;
            update_daily.execute(params![event.blocked_at_unix_ms, resource_type])?;
        }
    }
    transaction.commit()?;

    let snapshot = read_snapshot(connection)?;
    if let Ok(mut current) = shared.snapshot.lock() {
        *current = snapshot;
    }
    shared.version.fetch_add(1, Ordering::Release);
    Ok(())
}

fn read_snapshot(connection: &Connection) -> rusqlite::Result<StatisticsSnapshot> {
    let (total, today, seven_days): (i64, i64, i64) = connection.query_row(
        "SELECT
            COALESCE(SUM(total), 0),
            COALESCE(SUM(CASE WHEN day = date('now', 'localtime') THEN total ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN day >= date('now', 'localtime', '-6 days') THEN total ELSE 0 END), 0)
         FROM blocked_daily",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    let mut query = connection.prepare(
        "SELECT resource_type, SUM(total) AS total
         FROM blocked_daily
         GROUP BY resource_type
         ORDER BY total DESC, resource_type ASC",
    )?;
    let by_resource = query
        .query_map([], |row| {
            Ok(ResourceCount {
                resource_type: row.get(0)?,
                total: u64::try_from(row.get::<_, i64>(1)?).unwrap_or_default(),
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(StatisticsSnapshot {
        total_blocked: u64::try_from(total).unwrap_or_default(),
        blocked_today: u64::try_from(today).unwrap_or_default(),
        blocked_last_seven_days: u64::try_from(seven_days).unwrap_or_default(),
        by_resource,
        recording: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RequestContext, ResourceType, statistics_channel};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn records_events_and_restores_snapshot_from_disk() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "trash-adblock-storage-{}-{unique}",
            std::process::id()
        ));
        let path = directory.join("adblock.dat");
        let (sink, receiver) = statistics_channel(8);
        let mut recorder = StatisticsRecorder::open(&path, receiver).unwrap();

        sink.try_send(BlockEvent::now(
            RequestContext {
                browser_id: 1,
                frame_id: "main".into(),
                request_id: 8,
            },
            ResourceType::Script,
        ));

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            let (_, snapshot, error) = recorder.latest();
            assert!(error.is_none(), "recorder failed: {error:?}");
            if snapshot.total_blocked == 1 {
                assert_eq!(snapshot.blocked_today, 1);
                assert_eq!(snapshot.by_resource[0].resource_type, "script");
                break;
            }
            assert!(std::time::Instant::now() < deadline, "recorder timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        recorder.shutdown();

        let connection = Connection::open(&path).unwrap();
        initialize_database(&connection).unwrap();
        assert_eq!(read_snapshot(&connection).unwrap().total_blocked, 1);
        drop(connection);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
