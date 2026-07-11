//! Native, browser-process ad blocking primitives.
//!
//! The crate deliberately does not depend on CEF. Browser integrations translate
//! their callback values into [`AdapterRequest`] and keep CEF objects out of the
//! synchronous matching path.

mod adapter;
mod engine;
mod navigation;
mod policy;
mod request;
mod service;
mod statistics;
mod storage;
mod subscriptions;
mod youtube;

pub use adapter::{AdBlockAdapter, AdapterRequest};
pub use engine::{AdBlockEngine, BraveAdBlockEngine, EngineSnapshot};
pub use navigation::{FrameContext, FrameKey, NavigationContextStore};
pub use policy::{SitePolicy, SitePolicySnapshot, SiteShieldMode};
pub use request::{AdBlockDecision, AdBlockRequest, RequestContext, ResourceType};
pub use service::{AdBlockService, AdBlockServiceBuilder};
pub use statistics::{BlockEvent, StatisticsReceiver, StatisticsSink, statistics_channel};
pub use storage::{ResourceCount, StatisticsRecorder, StatisticsSnapshot, StatisticsStoreError};
pub use subscriptions::{FilterListStatus, FilterListUpdater};
pub use youtube::{
    SanitizedYoutubeResponse, YoutubeResponseKind, classify_youtube_response,
    sanitize_youtube_response,
};
