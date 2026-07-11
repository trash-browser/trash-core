use adblock::{
    Engine,
    lists::{FilterSet, ParseOptions, RuleTypes},
    request::Request,
};

use crate::{AdBlockDecision, AdBlockRequest};

/// Engine boundary kept independent from both CEF and the selected rule parser.
pub trait AdBlockEngine: Send + Sync + 'static {
    fn check(&self, request: &AdBlockRequest<'_>) -> AdBlockDecision;
}

/// Sized holder that allows a trait-object engine to live behind `ArcSwap`.
pub struct EngineSnapshot {
    engine: Box<dyn AdBlockEngine>,
}

impl EngineSnapshot {
    pub fn new(engine: impl AdBlockEngine) -> Self {
        Self {
            engine: Box::new(engine),
        }
    }

    pub fn check(&self, request: &AdBlockRequest<'_>) -> AdBlockDecision {
        self.engine.check(request)
    }
}

/// Network-filtering implementation backed by Brave's `adblock-rust` engine.
pub struct BraveAdBlockEngine {
    engine: Engine,
}

impl BraveAdBlockEngine {
    /// Creates a real engine with no subscriptions installed yet.
    pub fn empty() -> Self {
        Self::from_filter_lists(std::iter::empty::<String>())
    }

    /// Compiles complete filter-list texts into an immutable matching engine.
    ///
    /// Compilation is intentionally separate from [`crate::AdBlockService::replace_engine`]
    /// so callers can do this work away from the CEF IO thread and atomically
    /// publish the result afterwards.
    pub fn from_filter_lists(lists: impl IntoIterator<Item = String>) -> Self {
        let mut filters = FilterSet::new(false);
        let options = ParseOptions {
            rule_types: RuleTypes::NetworkOnly,
            ..ParseOptions::default()
        };
        for list in lists {
            filters.add_filter_list(list, options);
        }
        Self {
            engine: Engine::new_with_filter_set(filters),
        }
    }
}

impl Default for BraveAdBlockEngine {
    fn default() -> Self {
        Self::empty()
    }
}

impl AdBlockEngine for BraveAdBlockEngine {
    fn check(&self, request: &AdBlockRequest<'_>) -> AdBlockDecision {
        let Ok(engine_request) = Request::new(
            request.url,
            request.source_url,
            request.resource_type.adblock_name(),
            request.method,
        ) else {
            return AdBlockDecision::Allow;
        };

        let result = self.engine.check_network_request(&engine_request);
        if result.should_block() {
            // Redirect resources returned by adblock-rust are response bodies,
            // not URLs. Until a browser-owned resource registry is installed,
            // cancelling is the only correct action for a blocking match.
            AdBlockDecision::Block
        } else if let Some(url) = result.rewritten_url {
            AdBlockDecision::RewriteUrl(url)
        } else {
            AdBlockDecision::Allow
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RequestContext, ResourceType};

    fn request<'a>(url: &'a str, source: &'a str) -> AdBlockRequest<'a> {
        AdBlockRequest {
            url,
            source_url: source,
            document_url: source,
            method: "GET",
            resource_type: ResourceType::Script,
            is_navigation: false,
            context: RequestContext {
                browser_id: 1,
                frame_id: "main".into(),
                request_id: 7,
            },
        }
    }

    #[test]
    fn matches_network_rules_and_exceptions() {
        let engine = BraveAdBlockEngine::from_filter_lists([String::from(
            "||tracker.example^$script\n@@||tracker.example/allowed.js$script",
        )]);

        assert_eq!(
            engine.check(&request(
                "https://tracker.example/ad.js",
                "https://news.example/"
            )),
            AdBlockDecision::Block
        );
        assert_eq!(
            engine.check(&request(
                "https://tracker.example/allowed.js",
                "https://news.example/"
            )),
            AdBlockDecision::Allow
        );
    }
}
