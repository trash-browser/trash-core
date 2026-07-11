use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use arc_swap::ArcSwap;

use crate::{
    AdBlockDecision, AdBlockRequest, BlockEvent, BraveAdBlockEngine, EngineSnapshot,
    SitePolicySnapshot, StatisticsSink,
};

pub struct AdBlockServiceBuilder {
    engine: EngineSnapshot,
    policies: SitePolicySnapshot,
    statistics: Option<StatisticsSink>,
    enabled: bool,
}

impl Default for AdBlockServiceBuilder {
    fn default() -> Self {
        Self {
            engine: EngineSnapshot::new(BraveAdBlockEngine::empty()),
            policies: SitePolicySnapshot::default(),
            statistics: None,
            enabled: true,
        }
    }
}

impl AdBlockServiceBuilder {
    pub fn engine(mut self, engine: impl crate::AdBlockEngine) -> Self {
        self.engine = EngineSnapshot::new(engine);
        self
    }

    pub fn policies(mut self, policies: SitePolicySnapshot) -> Self {
        self.policies = policies;
        self
    }

    pub fn statistics(mut self, statistics: StatisticsSink) -> Self {
        self.statistics = Some(statistics);
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn build(self) -> AdBlockService {
        AdBlockService {
            enabled: AtomicBool::new(self.enabled),
            engine: ArcSwap::from_pointee(self.engine),
            policies: ArcSwap::from_pointee(self.policies),
            statistics: self.statistics,
            blocked_total: AtomicU64::new(0),
        }
    }
}

pub struct AdBlockService {
    enabled: AtomicBool,
    engine: ArcSwap<EngineSnapshot>,
    policies: ArcSwap<SitePolicySnapshot>,
    statistics: Option<StatisticsSink>,
    blocked_total: AtomicU64,
}

impl AdBlockService {
    pub fn check(&self, request: &AdBlockRequest<'_>) -> AdBlockDecision {
        if !self.enabled.load(Ordering::Relaxed)
            || should_bypass_url(request.url)
            || should_use_youtube_response_sanitizer(request)
            || self
                .policies
                .load()
                .is_disabled_for_url(request.document_url)
        {
            return AdBlockDecision::Allow;
        }

        let decision = self.engine.load().check(request);
        if decision.is_blocked() {
            self.record_blocked_request(request.context.clone(), request.resource_type);
        }
        decision
    }

    /// Record a request whose response was neutralized by a browser-process
    /// response sanitizer rather than cancelled by the network engine.
    pub fn record_blocked_request(
        &self,
        context: crate::RequestContext,
        resource: crate::ResourceType,
    ) {
        self.blocked_total.fetch_add(1, Ordering::Relaxed);
        if let Some(statistics) = &self.statistics {
            statistics.try_send(BlockEvent::now(context, resource));
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn replace_engine(&self, engine: EngineSnapshot) {
        self.engine.store(Arc::new(engine));
    }

    pub fn replace_policies(&self, policies: SitePolicySnapshot) {
        self.policies.store(Arc::new(policies));
    }

    pub fn blocked_total(&self) -> u64 {
        self.blocked_total.load(Ordering::Relaxed)
    }
}

fn should_bypass_url(url: &str) -> bool {
    const BYPASS_PREFIXES: &[&str] = &[
        "about:",
        "blob:",
        "chrome://",
        "chrome-extension://",
        "data:",
        "devtools://",
        "miko://",
    ];
    BYPASS_PREFIXES.iter().any(|prefix| {
        url.get(..prefix.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(prefix))
    })
}

fn should_use_youtube_response_sanitizer(request: &AdBlockRequest<'_>) -> bool {
    let document_host = url::Url::parse(request.document_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase));
    let request_host = url::Url::parse(request.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_ascii_lowercase));
    document_host
        .as_deref()
        .is_some_and(is_youtube_document_host)
        && request_host
            .as_deref()
            .is_some_and(is_youtube_playback_host)
}

fn is_youtube_document_host(host: &str) -> bool {
    host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com")
}

fn is_youtube_playback_host(host: &str) -> bool {
    is_youtube_document_host(host)
        || host == "youtubei.googleapis.com"
        || host == "googlevideo.com"
        || host.ends_with(".googlevideo.com")
        || host == "ytimg.com"
        || host.ends_with(".ytimg.com")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RequestContext, ResourceType, SitePolicy, SiteShieldMode, statistics_channel};

    fn request<'a>(url: &'a str, document_url: &'a str) -> AdBlockRequest<'a> {
        AdBlockRequest {
            url,
            source_url: document_url,
            document_url,
            method: "GET",
            resource_type: ResourceType::Script,
            is_navigation: false,
            context: RequestContext {
                browser_id: 3,
                frame_id: "main".into(),
                request_id: 11,
            },
        }
    }

    #[test]
    fn swaps_engines_and_emits_only_real_blocks() {
        let (sink, receiver) = statistics_channel(4);
        let service = AdBlockServiceBuilder::default().statistics(sink).build();
        let input = request("https://ads.example/ad.js", "https://news.example/");
        assert_eq!(service.check(&input), AdBlockDecision::Allow);

        let engine = BraveAdBlockEngine::from_filter_lists(["||ads.example^".into()]);
        service.replace_engine(EngineSnapshot::new(engine));
        assert_eq!(service.check(&input), AdBlockDecision::Block);
        assert_eq!(service.blocked_total(), 1);
        assert_eq!(receiver.try_recv().unwrap().context.request_id, 11);
    }

    #[test]
    fn disabled_site_and_internal_scheme_bypass_engine() {
        let policies = SitePolicySnapshot::new([SitePolicy {
            host: "news.example".into(),
            mode: SiteShieldMode::Disabled,
        }]);
        let service = AdBlockServiceBuilder::default()
            .engine(BraveAdBlockEngine::from_filter_lists([
                "||ads.example^".into()
            ]))
            .policies(policies)
            .build();

        assert_eq!(
            service.check(&request(
                "https://ads.example/ad.js",
                "https://news.example/"
            )),
            AdBlockDecision::Allow
        );
        assert_eq!(
            service.check(&request("miko://shell/app.js", "miko://shell/")),
            AdBlockDecision::Allow
        );
    }

    #[test]
    fn youtube_playback_requests_use_response_sanitizer_instead_of_network_cancel() {
        let service = AdBlockServiceBuilder::default()
            .engine(BraveAdBlockEngine::from_filter_lists([
                "||youtube.com^\n||googlevideo.com^\n||doubleclick.net^".into(),
            ]))
            .build();

        assert_eq!(
            service.check(&request(
                "https://www.youtube.com/youtubei/v1/player",
                "https://www.youtube.com/watch?v=abc"
            )),
            AdBlockDecision::Allow
        );
        assert_eq!(
            service.check(&request(
                "https://rr1---sn.example.googlevideo.com/videoplayback",
                "https://www.youtube.com/watch?v=abc"
            )),
            AdBlockDecision::Allow
        );
        assert_eq!(
            service.check(&request(
                "https://securepubads.g.doubleclick.net/tag/js/gpt.js",
                "https://www.youtube.com/watch?v=abc"
            )),
            AdBlockDecision::Block
        );
    }
}
