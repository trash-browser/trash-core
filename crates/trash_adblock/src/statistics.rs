use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc::{Receiver, RecvTimeoutError, SyncSender, TryRecvError, TrySendError, sync_channel},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::{RequestContext, ResourceType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEvent {
    pub context: RequestContext,
    pub resource_type: ResourceType,
    pub blocked_at_unix_ms: i64,
}

impl BlockEvent {
    pub(crate) fn now(context: RequestContext, resource_type: ResourceType) -> Self {
        let blocked_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        Self {
            context,
            resource_type,
            blocked_at_unix_ms,
        }
    }
}

#[derive(Clone)]
pub struct StatisticsSink {
    sender: SyncSender<BlockEvent>,
    dropped: Arc<AtomicU64>,
}

pub struct StatisticsReceiver {
    receiver: Receiver<BlockEvent>,
}

pub fn statistics_channel(capacity: usize) -> (StatisticsSink, StatisticsReceiver) {
    assert!(
        capacity > 0,
        "statistics channel must be bounded and non-empty"
    );
    let (sender, receiver) = sync_channel(capacity);
    (
        StatisticsSink {
            sender,
            dropped: Arc::new(AtomicU64::new(0)),
        },
        StatisticsReceiver { receiver },
    )
}

impl StatisticsSink {
    pub fn try_send(&self, event: BlockEvent) {
        if matches!(
            self.sender.try_send(event),
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_))
        ) {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn dropped_events(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl StatisticsReceiver {
    pub fn try_recv(&self) -> Result<BlockEvent, TryRecvError> {
        self.receiver.try_recv()
    }

    pub fn drain(&self) -> impl Iterator<Item = BlockEvent> + '_ {
        self.receiver.try_iter()
    }

    pub(crate) fn recv_timeout(&self, timeout: Duration) -> Result<BlockEvent, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(id: u64) -> BlockEvent {
        BlockEvent {
            context: RequestContext {
                browser_id: 1,
                frame_id: "main".into(),
                request_id: id,
            },
            resource_type: ResourceType::Script,
            blocked_at_unix_ms: 1_700_000_000_000 + id as i64,
        }
    }

    #[test]
    fn full_channel_drops_without_blocking() {
        let (sink, receiver) = statistics_channel(1);
        sink.try_send(event(1));
        sink.try_send(event(2));

        assert_eq!(sink.dropped_events(), 1);
        assert_eq!(receiver.try_recv().unwrap().context.request_id, 1);
    }
}
