use crate::{
    AdBlockDecision, AdBlockRequest, AdBlockService, FrameKey, NavigationContextStore,
    RequestContext, ResourceType,
};

/// Owned CEF callback values. The CEF layer creates this without exposing any
/// CEF object to the filtering crate.
#[derive(Debug, Clone)]
pub struct AdapterRequest {
    pub browser_id: i32,
    pub frame_id: String,
    pub request_id: u64,
    pub url: String,
    pub method: String,
    pub resource_type: ResourceType,
    pub is_navigation: bool,
    pub request_initiator: Option<String>,
}

#[derive(Clone)]
pub struct AdBlockAdapter {
    service: std::sync::Arc<AdBlockService>,
    navigation: std::sync::Arc<NavigationContextStore>,
}

impl AdBlockAdapter {
    pub fn new(
        service: std::sync::Arc<AdBlockService>,
        navigation: std::sync::Arc<NavigationContextStore>,
    ) -> Self {
        Self {
            service,
            navigation,
        }
    }

    pub fn evaluate(&self, input: AdapterRequest) -> AdBlockDecision {
        let key = FrameKey {
            browser_id: input.browser_id,
            frame_id: input.frame_id.clone(),
        };
        let frame = self.navigation.get(&key);

        let document_url = if input.is_navigation && input.resource_type == ResourceType::MainFrame
        {
            input.url.as_str()
        } else {
            frame
                .as_ref()
                .map(|context| context.top_level_url.as_str())
                .filter(|url| !url.is_empty())
                .unwrap_or(input.url.as_str())
        };
        let source_url = input
            .request_initiator
            .as_deref()
            .filter(|url| !url.is_empty())
            .or_else(|| {
                frame
                    .as_ref()
                    .map(|context| context.frame_url.as_str())
                    .filter(|url| !url.is_empty())
            })
            .unwrap_or(document_url);

        self.service.check(&AdBlockRequest {
            url: &input.url,
            source_url,
            document_url,
            method: &input.method,
            resource_type: input.resource_type,
            is_navigation: input.is_navigation,
            context: RequestContext {
                browser_id: input.browser_id,
                frame_id: input.frame_id,
                request_id: input.request_id,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BraveAdBlockEngine, FrameContext, SitePolicy, SitePolicySnapshot, SiteShieldMode};

    #[test]
    fn uses_top_level_navigation_context_for_site_policy() {
        let service = std::sync::Arc::new(
            crate::AdBlockServiceBuilder::default()
                .engine(BraveAdBlockEngine::from_filter_lists([
                    "||ads.example^".into()
                ]))
                .policies(SitePolicySnapshot::new([SitePolicy {
                    host: "news.example".into(),
                    mode: SiteShieldMode::Disabled,
                }]))
                .build(),
        );
        let navigation = std::sync::Arc::new(NavigationContextStore::default());
        navigation.upsert(FrameContext {
            key: FrameKey {
                browser_id: 1,
                frame_id: "child".into(),
            },
            frame_url: "https://comments.example/frame".into(),
            top_level_url: "https://news.example/article".into(),
        });
        let adapter = AdBlockAdapter::new(service, navigation);

        assert_eq!(
            adapter.evaluate(AdapterRequest {
                browser_id: 1,
                frame_id: "child".into(),
                request_id: 4,
                url: "https://ads.example/ad.js".into(),
                method: "GET".into(),
                resource_type: ResourceType::Script,
                is_navigation: false,
                request_initiator: None,
            }),
            AdBlockDecision::Allow
        );
    }
}
