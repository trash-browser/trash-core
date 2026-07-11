use serde_json::Value;
use url::Url;

const AD_KEYS: &[&str] = &["adPlacements", "playerAds", "adSlots", "adPlacementConfig"];
const INITIAL_PLAYER_MARKERS: &[&str] = &[
    "var ytInitialPlayerResponse =",
    "window[\"ytInitialPlayerResponse\"] =",
    "\"ytInitialPlayerResponse\":",
    "ytInitialPlayerResponse =",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoutubeResponseKind {
    PlayerJson,
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
    let is_youtube = host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com")
        || host == "youtubei.googleapis.com";
    if !is_youtube {
        return None;
    }

    let mime_type = mime_type.to_ascii_lowercase();
    if parsed.path() == "/youtubei/v1/player" && mime_type.contains("json") {
        Some(YoutubeResponseKind::PlayerJson)
    } else if (parsed.path() == "/watch" || parsed.path().starts_with("/embed/"))
        && mime_type.contains("html")
    {
        Some(YoutubeResponseKind::WatchHtml)
    } else {
        None
    }
}

pub fn sanitize_youtube_response(
    kind: YoutubeResponseKind,
    input: &[u8],
) -> SanitizedYoutubeResponse {
    match kind {
        YoutubeResponseKind::PlayerJson => sanitize_json(input),
        YoutubeResponseKind::WatchHtml => sanitize_watch_html(input),
    }
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

    for marker in INITIAL_PLAYER_MARKERS {
        let mut search_from = 0;
        while let Some(relative_marker) = html[search_from..].find(marker) {
            let marker_end = search_from + relative_marker + marker.len();
            let Some(relative_start) = html[marker_end..].find('{') else {
                break;
            };
            let object_start = marker_end + relative_start;
            let Some(object_end) = find_json_object_end(html.as_bytes(), object_start) else {
                break;
            };
            let sanitized = sanitize_json(&input[object_start..object_end]);
            if sanitized.removed_items > 0 {
                replacements.push((
                    object_start,
                    object_end,
                    sanitized.bytes,
                    sanitized.removed_items,
                ));
            }
            search_from = object_end;
        }
        if !replacements.is_empty() {
            break;
        }
    }

    if replacements.is_empty() {
        return unchanged(input);
    }

    let removed_items = replacements.iter().map(|replacement| replacement.3).sum();
    let mut bytes = input.to_vec();
    for (start, end, replacement, _) in replacements.into_iter().rev() {
        bytes.splice(start..end, replacement);
    }
    SanitizedYoutubeResponse {
        bytes,
        removed_items,
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
            let mut removed = 0;
            for key in AD_KEYS {
                if object.remove(*key).is_some() {
                    removed += 1;
                }
            }
            removed + object.values_mut().map(strip_ad_metadata).sum::<usize>()
        }
        Value::Array(values) => values.iter_mut().map(strip_ad_metadata).sum(),
        _ => 0,
    }
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
    fn classifies_only_youtube_player_responses() {
        assert_eq!(
            classify_youtube_response(
                "https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
                "application/json"
            ),
            Some(YoutubeResponseKind::PlayerJson)
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
            "nested":{"adSlots":[1],"keep":true}
        }"#;
        let result = sanitize_youtube_response(YoutubeResponseKind::PlayerJson, input);
        assert_eq!(result.removed_items, 3);
        let value: Value = serde_json::from_slice(&result.bytes).unwrap();
        assert!(value.get("adPlacements").is_none());
        assert!(value.get("playerAds").is_none());
        assert!(value["nested"].get("adSlots").is_none());
        assert_eq!(value["streamingData"]["formats"][0]["itag"], 18);
    }

    #[test]
    fn strips_initial_player_response_before_html_reaches_renderer() {
        let input = br#"<html><script>var ytInitialPlayerResponse = {"videoDetails":{"videoId":"abc"},"adPlacements":[1],"playerAds":[2]};</script></html>"#;
        let result = sanitize_youtube_response(YoutubeResponseKind::WatchHtml, input);
        let output = String::from_utf8(result.bytes).unwrap();
        assert_eq!(result.removed_items, 2);
        assert!(!output.contains("adPlacements"));
        assert!(!output.contains("playerAds"));
        assert!(output.contains("videoDetails"));
    }
}
