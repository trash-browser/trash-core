use serde_json::Value;
use url::Url;

/// Known YouTube ad payload keys (player + browse/search surfaces).
const AD_KEYS: &[&str] = &[
    "adPlacements",
    "playerAds",
    "adSlots",
    "adPlacementConfig",
    "adBreakHeartbeatParams",
    "serializedAdServingDataEntry",
    "serializedAdServingDataSet",
    "adBreakServiceMessage",
    "adSlotsInfo",
    "adsEngine",
    "adPlacementsMap",
    "adLayoutLoggingData",
    "adSlotMetadata",
    "playerGoodbyeMessageRenderer",
];

const INITIAL_PLAYER_MARKERS: &[&str] = &[
    "var ytInitialPlayerResponse =",
    "window[\"ytInitialPlayerResponse\"] =",
    "\"ytInitialPlayerResponse\":",
    "ytInitialPlayerResponse =",
];

const INITIAL_DATA_MARKERS: &[&str] = &[
    "var ytInitialData =",
    "window[\"ytInitialData\"] =",
    "\"ytInitialData\":",
    "ytInitialData =",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoutubeResponseKind {
    /// youtubei JSON APIs (player / next / browse / search / …).
    ApiJson,
    /// Watch / embed / home / shorts HTML carrying ytInitial* blobs.
    WatchHtml,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedYoutubeResponse {
    pub bytes: Vec<u8>,
    pub removed_items: usize,
}

pub fn classify_youtube_response(url: &str, mime_type: &str) -> Option<YoutubeResponseKind> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if !is_youtube_host(&host) {
        return None;
    }

    let mime_type = mime_type.to_ascii_lowercase();
    let path = parsed.path();

    if mime_type.contains("json")
        && (path.starts_with("/youtubei/")
            || path == "/get_video_info"
            || path.starts_with("/get_watch"))
    {
        return Some(YoutubeResponseKind::ApiJson);
    }

    if mime_type.contains("html")
        && (path == "/watch"
            || path.starts_with("/embed/")
            || path.starts_with("/shorts/")
            || path.starts_with("/live/")
            || path == "/"
            || path.starts_with("/feed/")
            || path.starts_with("/results")
            || path.starts_with("/channel/")
            || path.starts_with("/@")
            || path.starts_with("/c/")
            || path.starts_with("/user/"))
    {
        return Some(YoutubeResponseKind::WatchHtml);
    }

    None
}

pub fn sanitize_youtube_response(
    kind: YoutubeResponseKind,
    input: &[u8],
) -> SanitizedYoutubeResponse {
    match kind {
        YoutubeResponseKind::ApiJson => sanitize_json(input),
        YoutubeResponseKind::WatchHtml => sanitize_watch_html(input),
    }
}

fn is_youtube_host(host: &str) -> bool {
    host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com")
        || host == "youtubei.googleapis.com"
        || host == "www.youtube-nocookie.com"
}

fn sanitize_json(input: &[u8]) -> SanitizedYoutubeResponse {
    let Ok(mut value) = serde_json::from_slice::<Value>(input) else {
        return unchanged(input);
    };
    let removed_items = strip_ad_metadata(&mut value);
    if removed_items == 0 {
        return unchanged(input);
    }
    match serde_json::to_vec(&value) {
        Ok(bytes) => SanitizedYoutubeResponse {
            bytes,
            removed_items,
        },
        Err(_) => unchanged(input),
    }
}

fn sanitize_watch_html(input: &[u8]) -> SanitizedYoutubeResponse {
    let Ok(html) = std::str::from_utf8(input) else {
        return unchanged(input);
    };
    let mut replacements = Vec::new();

    for markers in [INITIAL_PLAYER_MARKERS, INITIAL_DATA_MARKERS] {
        collect_embedded_json_replacements(html, input, markers, &mut replacements);
    }

    if replacements.is_empty() {
        return unchanged(input);
    }

    // Prefer earlier, longer spans when markers overlap (e.g. `var X =` vs `X =`).
    replacements.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut deduped = Vec::new();
    let mut covered_until = 0usize;
    for replacement in replacements {
        if replacement.0 < covered_until {
            continue;
        }
        covered_until = replacement.1;
        deduped.push(replacement);
    }

    let removed_items = deduped.iter().map(|replacement| replacement.3).sum();
    let mut bytes = input.to_vec();
    for (start, end, replacement, _) in deduped.into_iter().rev() {
        if end > bytes.len() || start > end {
            continue;
        }
        bytes.splice(start..end, replacement);
    }
    SanitizedYoutubeResponse {
        bytes,
        removed_items,
    }
}

fn collect_embedded_json_replacements(
    html: &str,
    input: &[u8],
    markers: &[&str],
    replacements: &mut Vec<(usize, usize, Vec<u8>, usize)>,
) {
    for marker in markers {
        let mut search_from = 0;
        let mut matched_marker = false;
        while let Some(relative_marker) = html[search_from..].find(marker) {
            let marker_end = search_from + relative_marker + marker.len();
            let Some(relative_start) = html[marker_end..].find('{') else {
                break;
            };
            let object_start = marker_end + relative_start;
            let Some(object_end) = find_json_object_end(html.as_bytes(), object_start) else {
                break;
            };
            if object_end > input.len() {
                break;
            }
            let sanitized = sanitize_json(&input[object_start..object_end]);
            if sanitized.removed_items > 0 {
                replacements.push((
                    object_start,
                    object_end,
                    sanitized.bytes,
                    sanitized.removed_items,
                ));
                matched_marker = true;
            }
            search_from = object_end;
        }
        // First marker form that hits wins (avoids overlapping `var X =` / `X =`).
        if matched_marker {
            break;
        }
    }
}

fn find_json_object_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes[start..].iter().copied().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(start + offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn strip_ad_metadata(value: &mut Value) -> usize {
    match value {
        Value::Object(object) => {
            let keys: Vec<String> = object.keys().cloned().collect();
            let mut removed = 0;
            for key in keys {
                if is_ad_key(&key) {
                    object.remove(&key);
                    removed += 1;
                }
            }
            removed + object.values_mut().map(strip_ad_metadata).sum::<usize>()
        }
        Value::Array(values) => values.iter_mut().map(strip_ad_metadata).sum(),
        _ => 0,
    }
}

fn is_ad_key(key: &str) -> bool {
    if AD_KEYS.iter().any(|known| *known == key) {
        return true;
    }
    // Renderer type names used in browse/search/home shelves.
    if key.ends_with("AdRenderer")
        || key.ends_with("AdsRenderer")
        || key.ends_with("AdSlotRenderer")
        || key.ends_with("AdPlacementRenderer")
        || key.ends_with("BannerPromoRenderer")
        || key.ends_with("PromotedSparkles")
        || key == "adSlotRenderer"
        || key == "bannerPromoRenderer"
        || key == "reelPlayerOverlayAd"
        || key == "adHoverTextButtonRenderer"
        || key == "displayAdRenderer"
        || key == "inPlayerAd"
        || key == "instreamVideoAdRenderer"
        || key == "companionAdRenderer"
        || key == "actionCompanionAdRenderer"
    {
        return true;
    }
    false
}

fn unchanged(input: &[u8]) -> SanitizedYoutubeResponse {
    SanitizedYoutubeResponse {
        bytes: input.to_vec(),
        removed_items: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_player_next_and_watch() {
        assert_eq!(
            classify_youtube_response(
                "https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
                "application/json"
            ),
            Some(YoutubeResponseKind::ApiJson)
        );
        assert_eq!(
            classify_youtube_response(
                "https://www.youtube.com/youtubei/v1/next?prettyPrint=false",
                "application/json"
            ),
            Some(YoutubeResponseKind::ApiJson)
        );
        assert_eq!(
            classify_youtube_response(
                "https://www.youtube.com/watch?v=abc",
                "text/html; charset=utf-8"
            ),
            Some(YoutubeResponseKind::WatchHtml)
        );
        assert_eq!(
            classify_youtube_response(
                "https://notyoutube.com/youtubei/v1/player",
                "application/json"
            ),
            None
        );
    }

    #[test]
    fn strips_ad_metadata_without_changing_video_data() {
        let input = br#"{
            "playabilityStatus":{"status":"OK"},
            "streamingData":{"formats":[{"itag":18}]},
            "adPlacements":[{"adPlacementRenderer":{}}],
            "playerAds":[{"playerLegacyDesktopWatchAdsRenderer":{}}],
            "nested":{"adSlots":[1],"keep":true},
            "contents":{"adSlotRenderer":{},"videoRenderer":{"videoId":"x"}}
        }"#;
        let result = sanitize_youtube_response(YoutubeResponseKind::ApiJson, input);
        assert!(result.removed_items >= 3);
        let value: Value = serde_json::from_slice(&result.bytes).unwrap();
        assert!(value.get("adPlacements").is_none());
        assert!(value.get("playerAds").is_none());
        assert!(value["nested"].get("adSlots").is_none());
        assert!(value["contents"].get("adSlotRenderer").is_none());
        assert_eq!(value["streamingData"]["formats"][0]["itag"], 18);
        assert_eq!(value["contents"]["videoRenderer"]["videoId"], "x");
    }

    #[test]
    fn strips_initial_player_and_data_before_html_reaches_renderer() {
        let input = br#"<html><script>var ytInitialPlayerResponse = {"videoDetails":{"videoId":"abc"},"adPlacements":[1],"playerAds":[2]};</script><script>var ytInitialData = {"contents":{"adSlotRenderer":{"ad":1},"item":true}};</script></html>"#;
        let result = sanitize_youtube_response(YoutubeResponseKind::WatchHtml, input);
        let output = String::from_utf8(result.bytes).unwrap();
        assert!(result.removed_items >= 3);
        assert!(!output.contains("adPlacements"));
        assert!(!output.contains("playerAds"));
        assert!(!output.contains("adSlotRenderer"));
        assert!(output.contains("videoDetails"));
        assert!(output.contains("\"item\":true"));
    }
}
