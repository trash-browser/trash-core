use std::collections::HashMap;

use url::Url;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SiteShieldMode {
    #[default]
    Enabled,
    Disabled,
    Aggressive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePolicy {
    pub host: String,
    pub mode: SiteShieldMode,
}

#[derive(Debug, Clone, Default)]
pub struct SitePolicySnapshot {
    policies: HashMap<String, SiteShieldMode>,
}

impl SitePolicySnapshot {
    pub fn new(policies: impl IntoIterator<Item = SitePolicy>) -> Self {
        let policies = policies
            .into_iter()
            .map(|policy| (normalize_host(&policy.host), policy.mode))
            .filter(|(host, _)| !host.is_empty())
            .collect();
        Self { policies }
    }

    pub fn mode_for_url(&self, url: &str) -> SiteShieldMode {
        let Some(host) = Url::parse(url)
            .ok()
            .and_then(|url| url.host_str().map(normalize_host))
        else {
            return SiteShieldMode::Enabled;
        };
        self.policies.get(&host).copied().unwrap_or_default()
    }

    pub fn is_disabled_for_url(&self, url: &str) -> bool {
        self.mode_for_url(url) == SiteShieldMode::Disabled
    }
}

fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_lookup_normalizes_hosts() {
        let policies = SitePolicySnapshot::new([SitePolicy {
            host: "News.Example.".into(),
            mode: SiteShieldMode::Disabled,
        }]);
        assert!(policies.is_disabled_for_url("https://news.example/article"));
        assert!(!policies.is_disabled_for_url("https://cdn.news.example/image"));
    }
}
