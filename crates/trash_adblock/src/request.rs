/// Resource categories understood by the filtering engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    MainFrame,
    SubFrame,
    Stylesheet,
    Script,
    Image,
    Font,
    Object,
    Media,
    Worker,
    SharedWorker,
    Prefetch,
    Favicon,
    XmlHttpRequest,
    Ping,
    ServiceWorker,
    CspReport,
    WebSocket,
    Other,
}

impl ResourceType {
    pub(crate) fn adblock_name(self) -> &'static str {
        match self {
            Self::MainFrame => "document",
            Self::SubFrame => "subdocument",
            Self::Stylesheet => "stylesheet",
            Self::Script => "script",
            Self::Image | Self::Favicon => "image",
            Self::Font => "font",
            Self::Object => "object",
            Self::Media => "media",
            Self::Worker | Self::SharedWorker | Self::ServiceWorker => "script",
            Self::Prefetch => "other",
            Self::XmlHttpRequest => "xmlhttprequest",
            Self::Ping | Self::CspReport => "ping",
            Self::WebSocket => "websocket",
            Self::Other => "other",
        }
    }

    pub(crate) fn storage_name(self) -> &'static str {
        match self {
            Self::MainFrame => "mainFrame",
            Self::SubFrame => "subFrame",
            Self::Stylesheet => "stylesheet",
            Self::Script => "script",
            Self::Image => "image",
            Self::Font => "font",
            Self::Object => "object",
            Self::Media => "media",
            Self::Worker => "worker",
            Self::SharedWorker => "sharedWorker",
            Self::Prefetch => "prefetch",
            Self::Favicon => "favicon",
            Self::XmlHttpRequest => "xmlHttpRequest",
            Self::Ping => "ping",
            Self::ServiceWorker => "serviceWorker",
            Self::CspReport => "cspReport",
            Self::WebSocket => "webSocket",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RequestContext {
    pub browser_id: i32,
    pub frame_id: String,
    pub request_id: u64,
}

/// Borrowed request data used on the browser IO thread.
#[derive(Debug)]
pub struct AdBlockRequest<'a> {
    pub url: &'a str,
    pub source_url: &'a str,
    pub document_url: &'a str,
    pub method: &'a str,
    pub resource_type: ResourceType,
    pub is_navigation: bool,
    pub context: RequestContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdBlockDecision {
    Allow,
    Block,
    /// Change the outgoing URL, for example for a `$removeparam` rule.
    RewriteUrl(String),
    /// Route a request to a browser-owned replacement resource.
    Redirect(String),
}

impl AdBlockDecision {
    pub fn is_blocked(&self) -> bool {
        matches!(self, Self::Block)
    }
}
