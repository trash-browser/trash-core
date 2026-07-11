use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::Serialize;

use crate::{AdBlockService, BraveAdBlockEngine, EngineSnapshot};

const MAX_LIST_BYTES: u64 = 16 * 1024 * 1024;

struct FilterListSource {
    id: &'static str,
    file_name: &'static str,
    url: &'static str,
}

const DEFAULT_SOURCES: &[FilterListSource] = &[
    FilterListSource {
        id: "easylist",
        file_name: "easylist.txt",
        url: "https://easylist.to/easylist/easylist.txt",
    },
    FilterListSource {
        id: "easyprivacy",
        file_name: "easyprivacy.txt",
        url: "https://easylist.to/easylist/easyprivacy.txt",
    },
];

/// Always-on YouTube network filters (merged after EasyList / EasyPrivacy).
const BUILTIN_YOUTUBE_FILTERS: &str = include_str!("filters/youtube.txt");

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterListStatus {
    pub state: String,
    pub lists_loaded: usize,
    pub last_error: Option<String>,
}

impl FilterListStatus {
    fn loading() -> Self {
        Self {
            state: "loading".into(),
            lists_loaded: 0,
            last_error: None,
        }
    }

    fn active(lists_loaded: usize, last_error: Option<String>) -> Self {
        Self {
            state: "active".into(),
            lists_loaded,
            last_error,
        }
    }

    pub fn unavailable(error: impl Into<String>) -> Self {
        Self {
            state: "error".into(),
            lists_loaded: 0,
            last_error: Some(error.into()),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

struct SharedStatus {
    version: AtomicU64,
    status: Mutex<FilterListStatus>,
}

/// Loads cached subscriptions on a worker and refreshes them from their
/// official sources. Engine compilation never runs on the CEF IO thread.
pub struct FilterListUpdater {
    shared: Arc<SharedStatus>,
    worker: Option<JoinHandle<()>>,
}

impl FilterListUpdater {
    pub fn start(cache_directory: impl Into<PathBuf>, service: Arc<AdBlockService>) -> Self {
        let cache_directory = cache_directory.into();
        let shared = Arc::new(SharedStatus {
            version: AtomicU64::new(1),
            status: Mutex::new(FilterListStatus::loading()),
        });
        let worker_shared = shared.clone();
        let worker = thread::Builder::new()
            .name("adblock-filter-updater".into())
            .spawn(move || update_filter_lists(&cache_directory, &service, &worker_shared));

        match worker {
            Ok(worker) => Self {
                shared,
                worker: Some(worker),
            },
            Err(error) => {
                publish(
                    &shared,
                    FilterListStatus::unavailable(format!("failed to start updater: {error}")),
                );
                Self {
                    shared,
                    worker: None,
                }
            }
        }
    }

    pub fn latest(&self) -> (u64, FilterListStatus) {
        let version = self.shared.version.load(Ordering::Acquire);
        let status = self
            .shared
            .status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| FilterListStatus::unavailable("filter status lock poisoned"));
        (version, status)
    }

    pub fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for FilterListUpdater {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn update_filter_lists(cache_directory: &Path, service: &AdBlockService, shared: &SharedStatus) {
    if let Err(error) = std::fs::create_dir_all(cache_directory) {
        publish(
            shared,
            FilterListStatus::unavailable(format!("failed to create list cache: {error}")),
        );
        return;
    }

    // Ship YouTube network filters immediately, even before EasyList downloads.
    install(service, &HashMap::new());

    let mut effective = load_cached_lists(cache_directory);
    if !effective.is_empty() {
        install(service, &effective);
        publish(shared, FilterListStatus::active(effective.len(), None));
    }

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent("TrashBrowser/0.1 adblock-list-updater")
        .build();
    let agent: ureq::Agent = config.into();
    let mut errors = Vec::new();

    for source in DEFAULT_SOURCES {
        match download_list(&agent, source) {
            Ok(contents) => {
                if let Err(error) =
                    write_cache_file(&cache_directory.join(source.file_name), &contents)
                {
                    errors.push(format!("{} cache: {error}", source.id));
                }
                effective.insert(source.id, contents);
            }
            Err(error) => errors.push(format!("{}: {error}", source.id)),
        }
    }

    if effective.is_empty() {
        publish(shared, FilterListStatus::unavailable(errors.join("; ")));
        return;
    }

    install(service, &effective);
    publish(
        shared,
        FilterListStatus::active(
            effective.len(),
            (!errors.is_empty()).then(|| errors.join("; ")),
        ),
    );
}

fn load_cached_lists(cache_directory: &Path) -> HashMap<&'static str, String> {
    DEFAULT_SOURCES
        .iter()
        .filter_map(|source| {
            let contents = std::fs::read_to_string(cache_directory.join(source.file_name)).ok()?;
            validate_list(&contents).then_some((source.id, contents))
        })
        .collect()
}

fn install(service: &AdBlockService, lists: &HashMap<&'static str, String>) {
    let mut ordered: Vec<String> = DEFAULT_SOURCES
        .iter()
        .filter_map(|source| lists.get(source.id).cloned())
        .collect();
    ordered.push(BUILTIN_YOUTUBE_FILTERS.to_string());
    let engine = BraveAdBlockEngine::from_filter_lists(ordered);
    service.replace_engine(EngineSnapshot::new(engine));
}

fn download_list(agent: &ureq::Agent, source: &FilterListSource) -> Result<String, String> {
    let contents = agent
        .get(source.url)
        .call()
        .map_err(|error| error.to_string())?
        .body_mut()
        .with_config()
        .limit(MAX_LIST_BYTES)
        .read_to_string()
        .map_err(|error| error.to_string())?;
    if !validate_list(&contents) {
        return Err("response was not a valid Adblock Plus filter list".into());
    }
    Ok(contents)
}

fn validate_list(contents: &str) -> bool {
    contents.starts_with("[Adblock Plus") && contents.lines().take(1_001).count() > 1_000
}

fn write_cache_file(path: &Path, contents: &str) -> std::io::Result<()> {
    let temporary = path.with_extension("txt.tmp");
    {
        use std::io::Write as _;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
    }
    if let Err(error) = std::fs::rename(&temporary, path) {
        if path.exists() {
            std::fs::remove_file(path)?;
            std::fs::rename(&temporary, path)
        } else {
            Err(error)
        }
    } else {
        Ok(())
    }
}

fn publish(shared: &SharedStatus, status: FilterListStatus) {
    if let Ok(mut current) = shared.status.lock() {
        *current = status;
    }
    shared.version.fetch_add(1, Ordering::Release);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AdBlockDecision, AdBlockRequest, AdBlockServiceBuilder, RequestContext, ResourceType,
    };

    #[test]
    fn cached_list_installs_a_real_blocking_engine() {
        let service = AdBlockServiceBuilder::default().build();
        let lists = HashMap::from([(
            "easylist",
            format!(
                "[Adblock Plus 2.0]\n||ads.example^\n{}",
                "! padding\n".repeat(1_000)
            ),
        )]);
        install(&service, &lists);

        let decision = service.check(&AdBlockRequest {
            url: "https://ads.example/banner.js",
            source_url: "https://news.example/",
            document_url: "https://news.example/",
            method: "GET",
            resource_type: ResourceType::Script,
            is_navigation: false,
            context: RequestContext {
                browser_id: 1,
                frame_id: "main".into(),
                request_id: 1,
            },
        });
        assert_eq!(decision, AdBlockDecision::Block);
    }

    #[test]
    #[ignore = "requires network access to the official EasyList servers"]
    fn official_sources_download_and_block_a_known_tracker() {
        let cache_directory =
            std::env::temp_dir().join(format!("trash-adblock-live-lists-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&cache_directory);
        let service = Arc::new(AdBlockServiceBuilder::default().build());
        let mut updater = FilterListUpdater::start(&cache_directory, service.clone());
        let deadline = std::time::Instant::now() + Duration::from_secs(45);
        loop {
            let (_, status) = updater.latest();
            if status.state == "active" && status.lists_loaded == DEFAULT_SOURCES.len() {
                break;
            }
            assert!(
                status.state != "error",
                "official list update failed: {:?}",
                status.last_error
            );
            assert!(
                std::time::Instant::now() < deadline,
                "official list update timed out"
            );
            std::thread::sleep(Duration::from_millis(50));
        }

        let decision = service.check(&AdBlockRequest {
            url: "https://www.google-analytics.com/analytics.js",
            source_url: "https://news.example/",
            document_url: "https://news.example/",
            method: "GET",
            resource_type: ResourceType::Script,
            is_navigation: false,
            context: RequestContext {
                browser_id: 1,
                frame_id: "main".into(),
                request_id: 2,
            },
        });
        assert_eq!(decision, AdBlockDecision::Block);
        updater.shutdown();
        std::fs::remove_dir_all(cache_directory).unwrap();
    }
}
