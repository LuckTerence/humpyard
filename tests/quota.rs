//! Provider quota (ADR 0015): strict parsing of untrusted usage documents, validation, and the
//! poller as a running gateway sees it: fresh data, stale data, a source that hangs or fails
//! (requests never notice), the key going only where it should, and reloads keeping the snapshot.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::quota::parse_opencode_go;
use humpyard::server::{self, Reload};
use parking_lot::Mutex;
use serde_json::{Value, json};
use tokio::sync::mpsc;

/// A real answer from the `OpenCode` Go usage endpoint (2026-10-13).
const GOOD: &str = r#"{"usage":{"rolling":{"status":"ok","percent":0,"resetsAt":"2026-10-13T15:00:05.000Z"},"weekly":{"status":"ok","percent":0,"resetsAt":"2026-10-19T00:00:00.000Z"},"monthly":{"status":"ok","percent":11,"resetsAt":"2026-10-23T23:37:14.000Z"}}}"#;
const SECRET: &str = "sekret-key-123";

// ---- parsing ----

#[test]
fn a_real_document_parses_into_three_windows() {
    let windows = parse_opencode_go(GOOD).unwrap();
    let summary: Vec<_> = windows
        .iter()
        .map(|w| (w.name.as_str(), w.percent, w.status.as_str(), w.clamped))
        .collect();
    assert_eq!(
        summary,
        [
            ("rolling", 0.0, "ok", false),
            ("weekly", 0.0, "ok", false),
            ("monthly", 11.0, "ok", false)
        ]
    );
    assert_eq!(
        windows[2].resets_at.as_deref(),
        Some("2026-10-23T23:37:14.000Z")
    );
}

#[test]
fn bad_windows_are_dropped_the_rest_kept_and_nothing_is_guessed() {
    // Over 100 is clamped and flagged; negative, text and missing percents drop only that window.
    let doc = r#"{"usage":{
        "rolling":{"status":"limited","percent":150.5,"resetsAt":"2026-10-13T15:00:05.000Z"},
        "weekly":{"status":"ok","percent":-3},
        "monthly":{"status":"ok","percent":"11"},
        "surprise":{"status":"ok","percent":5}}}"#;
    let windows = parse_opencode_go(doc).unwrap();
    assert_eq!(windows.len(), 1, "{windows:?}");
    assert_eq!(
        (
            windows[0].name.as_str(),
            windows[0].percent,
            windows[0].clamped
        ),
        ("rolling", 100.0, true)
    );
    assert_eq!(
        windows[0].status, "limited",
        "an unknown status is kept as text"
    );
}

#[test]
fn text_from_the_source_is_kept_short_and_a_missing_status_is_unknown() {
    let long = "x".repeat(500);
    let doc = format!(
        r#"{{"usage":{{"weekly":{{"status":"{long}","percent":1}},"monthly":{{"percent":2}}}}}}"#
    );
    let windows = parse_opencode_go(&doc).unwrap();
    assert_eq!(windows[0].status.len(), 64);
    assert_eq!(windows[1].status, "unknown");
    assert_eq!(windows[1].resets_at, None);
}

#[test]
fn a_document_with_no_usable_window_is_an_error_so_it_cannot_blank_a_good_snapshot() {
    for bad in [
        "not json",
        "[]",
        "{}",
        r#"{"usage":null}"#,
        r#"{"usage":{}}"#,
        r#"{"usage":{"weekly":{"percent":null},"monthly":{"percent":-1}}}"#,
        r#"{"usage":{"rolling":{"status":"ok"}}}"#,
    ] {
        assert!(parse_opencode_go(bad).is_err(), "{bad}");
    }
}

// ---- configuration ----

fn load(extra_top: &str, provider: &str) -> Result<Config, String> {
    Config::from_toml(
        &format!(
            "listen = \"127.0.0.1:0\"\n{extra_top}\n[providers.p]\nbase_url = \"https://example.com/zen/go/v1\"\napi_key_env = \"K\"\n{provider}\n[targets.t]\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n"
        ),
        |_| Some("k".into()),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn quota_settings_have_defaults_and_are_validated() {
    let c = load("", "").unwrap();
    assert_eq!((c.quota_poll_secs, c.quota_stale_secs), (60, 300));
    assert_eq!(c.providers["p"].quota_source, None);
    let c = load(
        "quota_poll_secs = 15\nquota_stale_secs = 90",
        "quota_source = \"opencode-go\"",
    )
    .unwrap();
    assert_eq!((c.quota_poll_secs, c.quota_stale_secs), (15, 90));
    assert_eq!(
        c.providers["p"].quota_source.as_deref(),
        Some("opencode-go")
    );
    for (top, expected) in [
        ("quota_poll_secs = 0", "quota_poll_secs"),
        ("quota_poll_secs = 3601", "quota_poll_secs"),
        (
            "quota_poll_secs = 100\nquota_stale_secs = 99",
            "quota_stale_secs",
        ),
        ("quota_stale_secs = 100000", "quota_stale_secs"),
    ] {
        let message = load(top, "").unwrap_err();
        assert!(message.contains(expected), "{top}: {message}");
    }
}

#[test]
fn a_quota_source_must_be_known_and_its_provider_reachable_over_https() {
    let unknown = load("", "quota_source = \"nope\"").unwrap_err();
    assert!(
        unknown.contains("providers.p.quota_source") && unknown.contains("opencode-go"),
        "{unknown}"
    );
    let plain = Config::from_toml(
        "listen = \"127.0.0.1:0\"\n[providers.p]\nbase_url = \"http://example.com/v1\"\napi_key_env = \"K\"\nquota_source = \"opencode-go\"\n[targets.t]\nendpoints = [{ provider = \"p\", model = \"m\" }]\n",
        |_| Some("k".into()),
    )
    .unwrap_err()
    .to_string();
    assert!(plain.contains("needs an https base_url"), "{plain}");
    // Loopback http is how tests serve a fixture; it is not a way to send a key across a network.
    Config::from_toml(
        "listen = \"127.0.0.1:0\"\n[providers.p]\nbase_url = \"http://127.0.0.1:9/v1\"\napi_key_env = \"K\"\nquota_source = \"opencode-go\"\n[targets.t]\nendpoints = [{ provider = \"p\", model = \"m\" }]\n",
        |_| Some("k".into()),
    )
    .unwrap();
}

// ---- a running gateway ----

enum Mode {
    Doc(String),
    Status(u16),
    Hang,
}

struct Source {
    mode: Mutex<Mode>,
    hits: AtomicUsize,
    seen: Mutex<Vec<(String, Option<String>)>>,
}

async fn usage_doc(State(s): State<Arc<Source>>, headers: HeaderMap) -> Response {
    s.hits.fetch_add(1, Ordering::SeqCst);
    s.seen.lock().push((
        "/usage".into(),
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(String::from),
    ));
    let mode = match &*s.mode.lock() {
        Mode::Doc(doc) => Ok(doc.clone()),
        Mode::Status(code) => Err(Some(*code)),
        Mode::Hang => Err(None),
    };
    match mode {
        Ok(doc) => ([("content-type", "application/json")], doc).into_response(),
        Err(Some(code)) => StatusCode::from_u16(code).unwrap().into_response(),
        Err(None) => {
            tokio::time::sleep(Duration::from_secs(30)).await;
            StatusCode::OK.into_response()
        }
    }
}

async fn completions() -> Response {
    axum::Json(chat_completion("m", "served", Some(usage(10, 5)))).into_response()
}

async fn source() -> (Arc<Source>, String) {
    let s = Arc::new(Source {
        mode: Mutex::new(Mode::Doc(GOOD.into())),
        hits: AtomicUsize::new(0),
        seen: Mutex::new(Vec::new()),
    });
    let app = Router::new()
        .route("/usage", get(usage_doc))
        .route("/chat/completions", post(completions))
        .with_state(s.clone());
    (s, format!("http://{}", serve(app).await))
}

fn config_text(base: &str, hash: &str, poll: u64, stale: u64) -> String {
    format!(
        r#"listen = "127.0.0.1:0"
quota_poll_secs = {poll}
quota_stale_secs = {stale}
[keys.k]
sha256 = "{hash}"
[providers.p]
base_url = "{base}"
api_key_env = "K"
quota_source = "opencode-go"
max_retries = 0
[providers.plain]
base_url = "{base}"
api_key_env = "K"
max_retries = 0
[targets.t]
endpoints = [{{ provider = "p", model = "m" }}]
"#
    )
}

struct Gateway {
    url: String,
    key: String,
}

async fn gateway(base: &str, poll: u64, stale: u64) -> Gateway {
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let config = Config::from_toml(&config_text(base, &hash, poll, stale), |_| {
        Some(SECRET.into())
    })
    .unwrap();
    Gateway {
        url: format!(
            "http://{}",
            serve(server::router(config).await.unwrap()).await
        ),
        key,
    }
}

impl Gateway {
    async fn quota_raw(&self) -> reqwest::Response {
        reqwest::Client::new()
            .get(format!("{}/v1/quota", self.url))
            .bearer_auth(&self.key)
            .send()
            .await
            .unwrap()
    }

    async fn quota(&self) -> Value {
        self.quota_raw().await.json().await.unwrap()
    }

    /// Polls `/v1/quota` until `done` accepts provider `p`'s entry, or fails the test.
    async fn wait_for(&self, what: &str, done: impl Fn(&Value) -> bool) -> Value {
        for _ in 0..200 {
            let entry = self.quota().await["providers"]["p"].clone();
            if done(&entry) {
                return entry;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("never saw: {what}; last: {}", self.quota().await);
    }

    async fn ask(&self) -> (u16, Duration) {
        let started = Instant::now();
        let status = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.key)
            .json(&chat_request("t", false))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16();
        (status, started.elapsed())
    }
}

#[tokio::test]
async fn the_snapshot_is_polled_exposed_and_never_leaks_the_key() {
    let (src, base) = source().await;
    let gw = gateway(&base, 60, 300).await;
    let entry = gw
        .wait_for("a fresh snapshot", |e| e["state"] == "fresh")
        .await;
    assert_eq!(entry["source"], "opencode-go");
    let windows: Vec<(&str, f64)> = entry["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| (w["name"].as_str().unwrap(), w["percent"].as_f64().unwrap()))
        .collect();
    assert_eq!(
        windows,
        [("rolling", 0.0), ("weekly", 0.0), ("monthly", 11.0)]
    );
    assert_eq!(entry["windows"][2]["resets_at"], "2026-10-23T23:37:14.000Z");
    assert!(entry["age_secs"].as_u64().unwrap() <= 2);
    assert_eq!(entry["last_error"], Value::Null);

    let body = gw.quota().await;
    assert_eq!(
        (body["poll_secs"].as_u64(), body["stale_secs"].as_u64()),
        (Some(60), Some(300))
    );
    let names: Vec<&String> = body["providers"].as_object().unwrap().keys().collect();
    assert_eq!(
        names,
        ["p"],
        "only providers with a quota source are listed"
    );
    assert!(
        !body.to_string().contains(SECRET),
        "the provider key must never be exposed"
    );

    // The poll went to the provider's own host, under its base URL, with the provider's key.
    let seen = src.seen.lock().clone();
    assert_eq!(
        seen[0],
        ("/usage".to_string(), Some(format!("Bearer {SECRET}")))
    );
}

#[tokio::test]
async fn quota_needs_a_valid_key() {
    let (_src, base) = source().await;
    let gw = gateway(&base, 60, 300).await;
    let anonymous = reqwest::Client::new()
        .get(format!("{}/v1/quota", gw.url))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), 401);
    let wrong = reqwest::Client::new()
        .get(format!("{}/v1/quota", gw.url))
        .bearer_auth("not-a-key")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), 401);
}

#[tokio::test]
async fn a_source_that_hangs_never_delays_a_request_and_quota_is_unknown() {
    let (src, base) = source().await;
    *src.mode.lock() = Mode::Hang;
    let gw = gateway(&base, 60, 300).await;
    tokio::time::sleep(Duration::from_millis(300)).await; // the first poll is in flight and stuck
    assert_eq!(src.hits.load(Ordering::SeqCst), 1);
    for _ in 0..3 {
        let (status, took) = gw.ask().await;
        assert_eq!(status, 200);
        assert!(
            took < Duration::from_secs(2),
            "a request waited {took:?} for quota"
        );
    }
    let entry = gw.quota().await["providers"]["p"].clone();
    assert_eq!(entry["state"], "unknown");
    assert_eq!(entry["reason"], "not polled yet");
}

#[tokio::test]
async fn a_failing_or_malformed_source_keeps_the_last_good_snapshot_until_it_goes_stale() {
    let (src, base) = source().await;
    let gw = gateway(&base, 1, 3).await;
    gw.wait_for("a fresh snapshot", |e| e["state"] == "fresh")
        .await;

    *src.mode.lock() = Mode::Status(500);
    let entry = gw
        .wait_for("the failure noted on a still-fresh snapshot", |e| {
            e["last_error"] == "HTTP 500"
        })
        .await;
    assert_eq!(
        entry["state"], "fresh",
        "within the staleness limit the last good data still counts"
    );
    assert_eq!(entry["windows"].as_array().unwrap().len(), 3);

    *src.mode.lock() = Mode::Doc(r#"{"usage":{"weekly":{"percent":"x"}}}"#.into());
    gw.wait_for("a malformed document noted", |e| {
        e["last_error"]
            .as_str()
            .is_some_and(|m| m.starts_with("unusable response"))
    })
    .await;

    let stale = gw
        .wait_for("the snapshot going stale", |e| e["state"] == "unknown")
        .await;
    let reason = stale["reason"].as_str().unwrap();
    assert!(
        reason.starts_with("stale: the last good read was"),
        "{reason}"
    );
    assert!(reason.contains("unusable response"), "{reason}");

    *src.mode.lock() = Mode::Doc(GOOD.into());
    let back = gw.wait_for("recovery", |e| e["state"] == "fresh").await;
    assert_eq!(back["last_error"], Value::Null);
    assert_eq!(gw.ask().await.0, 200);
}

#[tokio::test]
async fn a_failing_first_poll_says_why() {
    let (src, base) = source().await;
    *src.mode.lock() = Mode::Status(401);
    let gw = gateway(&base, 1, 3).await;
    let entry = gw
        .wait_for("the first failure", |e| e["reason"] != "not polled yet")
        .await;
    assert_eq!(entry["state"], "unknown");
    assert_eq!(entry["reason"], "the latest poll failed: HTTP 401");
}

// ---- reload ----

struct Running {
    path: std::path::PathBuf,
    url: String,
    reload: mpsc::Sender<()>,
    stop: mpsc::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), String>>,
    _dir: tempfile::TempDir,
}

fn reload_text(base: &str, extra: &str) -> String {
    format!(
        r#"listen = "127.0.0.1:0"
quota_poll_secs = 600
quota_stale_secs = 900
[providers.p]
base_url = "{base}"
api_key_env = "PATH"
max_retries = 0
quota_source = "opencode-go"
[targets.t]
endpoints = [{{ provider = "p", model = "m" }}]
{extra}
"#
    )
}

async fn start_running(text: &str) -> Running {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("humpyard.toml");
    std::fs::write(&path, text).unwrap();
    let config = Config::from_toml(text, |n| std::env::var(n).ok()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (reload, triggers) = mpsc::channel(4);
    let (stop, signals) = mpsc::channel(4);
    let task = tokio::spawn(server::run(
        config,
        listener,
        signals,
        Some(Reload {
            path: path.clone(),
            triggers,
        }),
    ));
    Running {
        path,
        url,
        reload,
        stop,
        task,
        _dir: dir,
    }
}

impl Running {
    async fn quota_p(&self) -> Value {
        reqwest::get(format!("{}/v1/quota", self.url))
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["providers"]["p"]
            .clone()
    }

    async fn reload_and_wait(&self, text: &str) {
        let before = self.reload_attempts().await;
        std::fs::write(&self.path, text).unwrap();
        self.reload.send(()).await.unwrap();
        for _ in 0..100 {
            if self.reload_attempts().await > before {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("the reload did not finish");
    }

    async fn reload_attempts(&self) -> u64 {
        reqwest::get(format!("{}/v1/health", self.url))
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap()["reload"]["attempts"]
            .as_u64()
            .unwrap_or(0)
    }
}

#[tokio::test]
async fn a_reload_keeps_the_snapshot_unless_the_source_changed() {
    let (_a, base_a) = source().await;
    let (_b, base_b) = source().await;
    let running = start_running(&reload_text(&base_a, "")).await;
    for _ in 0..100 {
        if running.quota_p().await["state"] == "fresh" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(running.quota_p().await["state"], "fresh");

    // An unrelated edit: the snapshot stays (the next poll is ten minutes away, so a kept
    // snapshot is the only way it can still be fresh).
    running
        .reload_and_wait(&reload_text(
            &base_a,
            "[routes.r]\ntype = \"passthrough\"\ntargets = [\"t\"]",
        ))
        .await;
    let kept = running.quota_p().await;
    assert_eq!(kept["state"], "fresh", "{kept}");
    assert_eq!(kept["windows"][2]["percent"], 11.0);

    // A different provider URL is a different source: data from the old one is not reused.
    running.reload_and_wait(&reload_text(&base_b, "")).await;
    let changed = running.quota_p().await;
    assert_eq!(changed["state"], "unknown", "{changed}");
    assert_eq!(changed["reason"], "not polled yet");

    running.stop.send(()).await.unwrap();
    let _ = tokio::time::timeout(Duration::from_secs(10), running.task).await;
}

#[tokio::test]
async fn a_gateway_without_quota_sources_never_polls_and_lists_nothing() {
    let (src, base) = source().await;
    let (key, hash) = humpyard::auth::generate_key().unwrap();
    let text = format!(
        "listen = \"127.0.0.1:0\"\nquota_poll_secs = 1\nquota_stale_secs = 3\n[keys.k]\nsha256 = \"{hash}\"\n[providers.p]\nbase_url = \"{base}\"\napi_key_env = \"K\"\n[targets.t]\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n"
    );
    let config = Config::from_toml(&text, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    tokio::time::sleep(Duration::from_millis(2500)).await; // more than two poll intervals
    assert_eq!(
        src.hits.load(Ordering::SeqCst),
        0,
        "no quota source, no polling"
    );
    let body: Value = reqwest::Client::new()
        .get(format!("{url}/v1/quota"))
        .bearer_auth(&key)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["providers"], json!({}));
}
