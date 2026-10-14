//! Provider quota: the usage windows a provider publishes (how much of a capped plan is used),
//! polled while serving and kept as the latest snapshot per provider (ADR 0015).
//!
//! Everything here is advisory and fails open. Polling runs off the request path with a timeout;
//! a failed, slow or malformed answer only means the snapshot goes stale, and stale or missing
//! data is *unknown*, never a guess. The provider's API key goes only to that provider's own host
//! over https (plain http to a loopback host is allowed so tests can serve a fixture) and appears
//! in no log, snapshot or response.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use parking_lot::Mutex;
use serde_json::Value;

use crate::clock::Clock;
use crate::config::Config;

/// How long one poll may take.
const POLL_TIMEOUT: Duration = Duration::from_secs(5);
/// A usage document is tiny; refuse anything larger.
const MAX_BODY_BYTES: usize = 64 * 1024;
/// The windows the adapters report, in display order.
pub const WINDOWS: [&str; 3] = ["rolling", "weekly", "monthly"];
/// A window that has not changed is still sampled this often, so a flat stretch is visible as
/// "still there" and not as a gap.
pub const SAMPLE_EVERY_MS: i64 = 15 * 60 * 1000;
/// Text fields from a source are kept short: they are shown to operators, never interpreted.
const MAX_TEXT: usize = 64;

/// One usage window of a provider's plan.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    /// `rolling`, `weekly` or `monthly`.
    pub name: String,
    /// Percent of the window's allowance used, 0 to 100.
    pub percent: f64,
    /// The provider's own status text (for example `ok`), kept as written.
    pub status: String,
    /// When the window resets, as the provider wrote it.
    pub resets_at: Option<String>,
    /// The provider reported more than 100 percent and the value was clamped.
    pub clamped: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct QuotaError(pub String);

fn short_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(MAX_TEXT).collect())
}

/// Reads an `OpenCode` Go usage document:
/// `{"usage": {"rolling": {"status", "percent", "resetsAt"}, "weekly": {...}, "monthly": {...}}}`.
///
/// A window whose percent is missing, not a number or negative is dropped; one above 100 is clamped
/// and flagged; unknown windows and fields are ignored. A document with no usable window is an
/// error, so it can never replace a good snapshot with an empty one.
pub fn parse_opencode_go(body: &str) -> Result<Vec<Window>, QuotaError> {
    let root: Value = serde_json::from_str(body)
        .map_err(|e| QuotaError(format!("the usage document is not valid JSON: {e}")))?;
    let usage = root
        .get("usage")
        .and_then(Value::as_object)
        .ok_or_else(|| QuotaError("the usage document has no `usage` object".into()))?;
    let windows: Vec<Window> = WINDOWS
        .iter()
        .filter_map(|name| {
            let window = usage.get(*name)?;
            let percent = window
                .get("percent")
                .and_then(Value::as_f64)
                .filter(|p| p.is_finite() && *p >= 0.0)?;
            Some(Window {
                name: (*name).to_string(),
                percent: percent.min(100.0),
                status: short_text(window.get("status")).unwrap_or_else(|| "unknown".into()),
                resets_at: short_text(window.get("resetsAt")),
                clamped: percent > 100.0,
            })
        })
        .collect();
    if windows.is_empty() {
        return Err(QuotaError("the usage document has no usable window".into()));
    }
    Ok(windows)
}

/// What a provider's snapshot is worth right now.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    Fresh {
        windows: Vec<Window>,
        fetched_ms: i64,
        age_ms: i64,
        /// A later poll failed (the snapshot is still within the staleness limit).
        last_error: Option<String>,
    },
    Unknown {
        reason: String,
    },
}

struct Entry {
    /// Source and URL the data came from: data from a different source is never reused.
    identity: String,
    ok: Option<(Vec<Window>, i64)>,
    error: Option<(String, i64)>,
}

/// The latest quota snapshot per provider. Survives configuration reloads: it is keyed by provider
/// name and identity, so a reload that changes neither keeps the data.
#[derive(Default)]
pub struct QuotaStore {
    inner: Mutex<HashMap<String, Entry>>,
}

impl QuotaStore {
    fn entry<'a>(
        map: &'a mut HashMap<String, Entry>,
        provider: &str,
        identity: &str,
    ) -> &'a mut Entry {
        let entry = map.entry(provider.to_string()).or_insert_with(|| Entry {
            identity: identity.to_string(),
            ok: None,
            error: None,
        });
        if entry.identity != identity {
            *entry = Entry {
                identity: identity.to_string(),
                ok: None,
                error: None,
            };
        }
        entry
    }

    pub fn record_ok(&self, provider: &str, identity: &str, windows: Vec<Window>, now_ms: i64) {
        let mut map = self.inner.lock();
        let entry = Self::entry(&mut map, provider, identity);
        entry.ok = Some((windows, now_ms));
        entry.error = None;
    }

    pub fn record_error(&self, provider: &str, identity: &str, message: String, now_ms: i64) {
        let mut map = self.inner.lock();
        Self::entry(&mut map, provider, identity).error = Some((message, now_ms));
    }

    /// The provider's snapshot if it is fresh enough (`stale_ms`), else why it is unknown.
    pub fn view(&self, provider: &str, identity: &str, now_ms: i64, stale_ms: i64) -> View {
        let map = self.inner.lock();
        let Some(entry) = map.get(provider).filter(|e| e.identity == identity) else {
            return View::Unknown {
                reason: "not polled yet".into(),
            };
        };
        let last_error = entry.error.as_ref().map(|(m, _)| m.clone());
        match &entry.ok {
            Some((windows, fetched_ms)) => {
                let age_ms = (now_ms - fetched_ms).max(0);
                if age_ms <= stale_ms {
                    View::Fresh {
                        windows: windows.clone(),
                        fetched_ms: *fetched_ms,
                        age_ms,
                        last_error,
                    }
                } else {
                    View::Unknown {
                        reason: format!(
                            "stale: the last good read was {}s ago{}",
                            age_ms / 1000,
                            last_error.map_or_else(String::new, |e| format!(" (latest poll: {e})"))
                        ),
                    }
                }
            }
            None => View::Unknown {
                reason: last_error.map_or_else(
                    || "not polled yet".to_string(),
                    |e| format!("the latest poll failed: {e}"),
                ),
            },
        }
    }
}

/// Decides which polls are worth keeping as history: one whose windows differ from the last kept
/// sample, or any poll once [`SAMPLE_EVERY_MS`] has passed since the last one kept. Pure, so the
/// rule is tested without a clock or a database.
#[derive(Default)]
pub struct Sampler {
    last: HashMap<String, (Vec<Window>, i64)>,
}

impl Sampler {
    /// Whether this poll should be stored; remembers it if so.
    pub fn due(&mut self, provider: &str, windows: &[Window], now_ms: i64) -> bool {
        let keep = self.last.get(provider).is_none_or(|(kept, at)| {
            kept.as_slice() != windows || now_ms.saturating_sub(*at) >= SAMPLE_EVERY_MS
        });
        if keep {
            self.last
                .insert(provider.to_string(), (windows.to_vec(), now_ms));
        }
        keep
    }
}

/// Identifies the data a provider's quota snapshot came from.
pub fn identity(source: &str, base_url: &str) -> String {
    format!("{source}|{}", poll_url(source, base_url))
}

/// Where a source's usage document lives: under the provider's own base URL, so the poll can never
/// leave the provider's host.
pub fn poll_url(_source: &str, base_url: &str) -> String {
    // Every known source keeps its usage document at `<base URL>/usage`.
    format!("{}/usage", base_url.trim_end_matches('/'))
}

/// One provider to poll.
#[derive(Clone)]
pub struct PollTarget {
    pub provider: String,
    pub identity: String,
    pub url: String,
    api_key: String,
}

/// What the poller should do this cycle, taken from the current configuration.
pub struct Plan {
    pub targets: Vec<PollTarget>,
    pub poll: Duration,
}

/// The providers of `config` that name a quota source.
pub fn plan(config: &Config) -> Plan {
    Plan {
        targets: config
            .providers
            .iter()
            .filter_map(|(name, provider)| {
                let source = provider.quota_source.as_deref()?;
                Some(PollTarget {
                    provider: name.clone(),
                    identity: identity(source, &provider.base_url),
                    url: poll_url(source, &provider.base_url),
                    api_key: provider.api_key.clone(),
                })
            })
            .collect(),
        poll: Duration::from_secs(config.quota_poll_secs),
    }
}

/// https, or http to a loopback host (tests); anything else is refused before a key is sent.
fn check_url(url: &str) -> Result<reqwest::Url, String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "the poll URL is not valid".to_string())?;
    let loopback = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback) {
        Ok(parsed)
    } else {
        Err("the poll URL must use https".into())
    }
}

/// Reads and parses one provider's usage document. Errors are short and never contain the key.
async fn fetch(client: &reqwest::Client, target: &PollTarget) -> Result<Vec<Window>, String> {
    let url = check_url(&target.url)?;
    let mut response = client
        .get(url)
        .bearer_auth(&target.api_key)
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "timed out".to_string()
            } else if e.is_connect() {
                "cannot connect".to_string()
            } else {
                "the request failed".to_string()
            }
        })?;
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "the response could not be read".to_string())?
    {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err("the response is too large".into());
        }
        body.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(body).map_err(|_| "the response is not text".to_string())?;
    parse_opencode_go(&text).map_err(|e| format!("unusable response: {e}"))
}

/// Polls every quota-sourced provider of the current configuration, forever, until `current`
/// returns `None` (the server is gone). Each cycle reads the live plan, so a reload that adds,
/// removes or changes a source takes effect without restarting anything.
///
/// `keep` receives each poll worth keeping as history (see [`Sampler`]): the provider, its
/// windows and the time. It must not block.
pub async fn run(
    store: Arc<QuotaStore>,
    clock: Arc<dyn Clock>,
    current: impl Fn() -> Option<Plan> + Send + 'static,
    keep: impl Fn(&str, &[Window], i64) + Send + 'static,
) {
    let Ok(client) = reqwest::Client::builder()
        .timeout(POLL_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    else {
        tracing::error!("quota polling is off: the HTTP client could not be built");
        return;
    };
    // Whether each provider's last poll failed, to log only when that changes.
    let mut failing: HashMap<String, bool> = HashMap::new();
    let mut sampler = Sampler::default();
    loop {
        let Some(plan) = current() else { return };
        let results = join_all(plan.targets.iter().map(|t| {
            let client = &client;
            async move { (t, fetch(client, t).await) }
        }))
        .await;
        let now = clock.now_ms();
        for (target, result) in results {
            let was_failing = failing.get(&target.provider).copied().unwrap_or(false);
            match result {
                Ok(windows) => {
                    if sampler.due(&target.provider, &windows, now) {
                        keep(&target.provider, &windows, now);
                    }
                    store.record_ok(&target.provider, &target.identity, windows, now);
                    if was_failing {
                        tracing::info!(provider = %target.provider, "quota polling recovered");
                    }
                    failing.insert(target.provider.clone(), false);
                }
                Err(message) => {
                    if !was_failing {
                        tracing::warn!(
                            provider = %target.provider,
                            error = %message,
                            "quota poll failed; the snapshot will go stale"
                        );
                    }
                    store.record_error(&target.provider, &target.identity, message, now);
                    failing.insert(target.provider.clone(), true);
                }
            }
        }
        tokio::time::sleep(plan.poll).await;
    }
}
