use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FrameKey {
    pub browser_id: i32,
    pub frame_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameContext {
    pub key: FrameKey,
    pub frame_url: String,
    pub top_level_url: String,
}

/// Concurrent navigation snapshot used by request callbacks that cannot rely on
/// every CEF callback carrying a stable top-level URL.
#[derive(Default)]
pub struct NavigationContextStore {
    frames: DashMap<FrameKey, FrameContext>,
}

impl NavigationContextStore {
    pub fn upsert(&self, context: FrameContext) {
        self.frames.insert(context.key.clone(), context);
    }

    pub fn get(&self, key: &FrameKey) -> Option<FrameContext> {
        self.frames.get(key).map(|entry| entry.clone())
    }

    pub fn remove_frame(&self, key: &FrameKey) {
        self.frames.remove(key);
    }

    pub fn remove_browser(&self, browser_id: i32) {
        self.frames.retain(|key, _| key.browser_id != browser_id);
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_all_frames_for_closed_browser() {
        let store = NavigationContextStore::default();
        for browser_id in [1, 1, 2] {
            let frame_id = format!("{}-{}", browser_id, store.len());
            store.upsert(FrameContext {
                key: FrameKey {
                    browser_id,
                    frame_id,
                },
                frame_url: "https://example.test/frame".into(),
                top_level_url: "https://example.test/".into(),
            });
        }

        store.remove_browser(1);
        assert_eq!(store.len(), 1);
    }
}
