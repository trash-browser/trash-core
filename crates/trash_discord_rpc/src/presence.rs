//! Presence payload formatting (Discord string limits, host extraction).

const DISCORD_MAX_LEN: usize = 128;

/// Snapshot of what the browser wants Discord to show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PresenceStatus {
    /// Active tab title (may be empty → treated as New Tab).
    pub title: String,
    /// Active tab URL (empty / `miko://` → start page).
    pub url: String,
    /// Open tab count (shown in state).
    pub tab_count: usize,
}

/// Formatted strings ready for `Activity::details` / `Activity::state`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedPresence {
    pub details: String,
    pub state: String,
}

/// Build Discord details/state from a browser status snapshot.
pub fn format_presence(status: &PresenceStatus) -> FormattedPresence {
    let start_page = is_start_page(&status.url);
    let title = status.title.trim();
    let details = if title.is_empty() || title.eq_ignore_ascii_case("Start") {
        if start_page {
            "New Tab".to_string()
        } else {
            "Untitled".to_string()
        }
    } else {
        truncate(title, DISCORD_MAX_LEN)
    };

    let state = if start_page {
        match status.tab_count {
            0 | 1 => "Start page".to_string(),
            n => format!("Start page · {n} tabs"),
        }
    } else {
        let host = display_host(&status.url);
        match status.tab_count {
            0 | 1 => format!("Browsing {host}"),
            n => format!("Browsing {host} · {n} tabs"),
        }
    };

    FormattedPresence {
        details,
        state: truncate(&state, DISCORD_MAX_LEN),
    }
}

fn is_start_page(url: &str) -> bool {
    let url = url.trim();
    url.is_empty() || url.starts_with("miko://") || url.starts_with("about:")
}

fn display_host(url: &str) -> String {
    let raw = url.trim();
    if raw.is_empty() {
        return "the web".into();
    }
    if let Ok(parsed) = url::Url::parse(raw) {
        if let Some(host) = parsed.host_str() {
            let host = host.strip_prefix("www.").unwrap_or(host);
            if !host.is_empty() {
                return host.to_string();
            }
        }
    }
    // Fallback: strip scheme-ish prefix for display.
    let stripped = raw
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.");
    let host = stripped.split(['/', '?', '#']).next().unwrap_or(stripped);
    if host.is_empty() {
        "the web".into()
    } else {
        truncate(host, 64)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_page() {
        let f = format_presence(&PresenceStatus {
            title: "Start".into(),
            url: String::new(),
            tab_count: 1,
        });
        assert_eq!(f.details, "New Tab");
        assert_eq!(f.state, "Start page");
    }

    #[test]
    fn browsing_host_and_tabs() {
        let f = format_presence(&PresenceStatus {
            title: "Example Domain".into(),
            url: "https://www.example.com/path".into(),
            tab_count: 3,
        });
        assert_eq!(f.details, "Example Domain");
        assert_eq!(f.state, "Browsing example.com · 3 tabs");
    }
}
