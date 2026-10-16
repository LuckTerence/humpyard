//! Latency-ordered routes (ADR 0020): a `fastest` route learns which eligible target answers quickest,
//! tries it first, keeps exploring at the configured rate, ignores failures as speed samples, and never
//! tries a target that eligibility removed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicU64, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::ledger::{Ledger, SessionQuery};
use humpyard::server;
use serde_json::{Value, json};

struct Upstream {
    delay_ms: AtomicU64,
    status: AtomicU16,
}

impl Upstream {
    fn set_status(&self, status: u16) {
        self.status.store(status, Ordering::SeqCst);
    }
}

async fn completions(State(up): State<Arc<Upstream>>) -> Response {
    tokio::time::sleep(Duration::from_millis(up.delay_ms.load(Ordering::SeqCst))).await;
    match up.status.load(Ordering::SeqCst) {
        200 => axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response(),
        code => (
            StatusCode::from_u16(code).unwrap(),
            axum::Json(json!({"error": {"message": "down"}})),
        )
            .into_response(),
    }
}

async fn upstream(delay_ms: u64) -> (Arc<Upstream>, String) {
    let up = Arc::new(Upstream {
        delay_ms: AtomicU64::new(delay_ms),
        status: AtomicU16::new(200),
    });
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(up.clone());
    (up, format!("http://{}", serve(app).await))
}

struct Rig {
    url: String,
    key: String,
    credential: String,
    quick: Arc<Upstream>,
    _dir: tempfile::TempDir,
}

/// Route `pool` lists `slow` (80 ms) before `quick` (instant). `route_extra` is added to the route.
async fn rig(route_extra: &str, targets_extra: &str) -> Rig {
    let (_slow, slow_url) = upstream(80).await;
    let (quick, quick_url) = upstream(0).await;
    let dir = tempfile::tempdir().unwrap();
    let (key, key_hash) = humpyard::auth::generate_key().unwrap();
    let (credential, credential_hash) = humpyard::auth::generate_key().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[ledger]
path = "{db}"
[metrics]
enabled = true
bearer_sha256 = "{credential_hash}"
[health]
failure_threshold = 100
[keys.alice]
sha256 = "{key_hash}"
[providers.ps]
base_url = "{slow_url}"
api_key_env = "K"
max_retries = 0
[providers.pq]
base_url = "{quick_url}"
api_key_env = "K"
max_retries = 0
[targets.slow]
{targets_extra}
endpoints = [{{ provider = "ps", model = "m" }}]
[targets.quick]
endpoints = [{{ provider = "pq", model = "m" }}]
[routes.pool]
type = "fastest"
targets = ["slow", "quick"]
{route_extra}
"#,
        db = dir.path().join("ledger.db").display(),
    );
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    Rig {
        url: format!(
            "http://{}",
            serve(server::router(config).await.unwrap()).await
        ),
        key,
        credential,
        quick,
        _dir: dir,
    }
}

impl Rig {
    /// The serving target's name (empty when the request failed) and the status.
    async fn ask(&self) -> (u16, String) {
        let resp = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.key)
            .json(&chat_request("pool", false))
            .send()
            .await
            .unwrap();
        let target = resp
            .headers()
            .get("x-humpyard-target")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        (resp.status().as_u16(), target)
    }

    async fn served(&self, requests: usize) -> Vec<String> {
        let mut served = Vec::new();
        for _ in 0..requests {
            served.push(self.ask().await.1);
        }
        served
    }

    async fn get(&self, path: &str) -> Value {
        reqwest::Client::new()
            .get(format!("{}{path}", self.url))
            .bearer_auth(&self.key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn explain(&self) -> Value {
        reqwest::Client::new()
            .post(format!("{}/v1/route/explain", self.url))
            .bearer_auth(&self.key)
            .json(&json!({"model": "pool"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn newest_decision(&self) -> Value {
        for _ in 0..100 {
            let body = self.get("/v1/decisions?route=pool&limit=1").await;
            if let Some(first) = body["data"].as_array().and_then(|d| d.first()) {
                return first.clone();
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        panic!("no decision");
    }

    async fn metrics(&self) -> String {
        reqwest::Client::new()
            .get(format!("{}/metrics", self.url))
            .bearer_auth(&self.credential)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    }
}

const NO_EXPLORING: &str = "explore = 0.0\nmin_samples = 2";

#[tokio::test]
async fn the_route_measures_each_target_then_settles_on_the_faster_one() {
    let rig = rig(NO_EXPLORING, "").await;
    let served = rig.served(8).await;
    // Both start unmeasured, so the written order rules until each has two samples; then the faster
    // target is first and stays first.
    assert_eq!(
        served,
        [
            "slow", "slow", "quick", "quick", "quick", "quick", "quick", "quick"
        ]
    );
}

#[tokio::test]
async fn the_decision_records_the_order_and_why() {
    let rig = rig(NO_EXPLORING, "").await;
    rig.served(6).await;
    let decision = rig.newest_decision().await;
    assert_eq!(decision["ranking"]["order"], json!(["quick", "slow"]));
    assert_eq!(decision["ranking"]["reason"], "fastest");
}

#[tokio::test]
async fn explain_and_statistics_show_the_figures_and_the_order() {
    let rig = rig(NO_EXPLORING, "").await;
    let before = rig.explain().await;
    assert_eq!(
        before["ordering"]["order"],
        json!(["slow", "quick"]),
        "unmeasured: written order"
    );
    assert!(before["targets"][0]["current_latency_ms"].is_null());

    rig.served(6).await;
    let explained = rig.explain().await;
    assert_eq!(explained["ordering"]["type"], "fastest");
    assert_eq!(explained["ordering"]["order"], json!(["quick", "slow"]));
    assert_eq!(explained["ordering"]["explore"], 0.0);
    assert_eq!(explained["targets"][0]["target"], "quick");
    let slow = &explained["targets"][1];
    assert!(
        slow["current_latency_ms"].as_f64().unwrap() >= 70.0,
        "{slow}"
    );
    assert_eq!(slow["latency_samples"], 2);

    let health = rig.get("/v1/health").await;
    let quick = health["targets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["target"] == "quick")
        .unwrap();
    assert!(quick["latency_samples"].as_u64().unwrap() >= 4);

    // The live figure sits next to the window's percentiles in target statistics.
    let mut stats = Value::Null;
    for _ in 0..100 {
        stats = rig.get("/v1/stats?group_by=target&window=1h").await;
        if stats["data"].as_array().is_some_and(|d| d.len() == 2) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let row = stats["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["group"] == "slow")
        .unwrap();
    assert!(row["current_latency_ms"].as_f64().unwrap() >= 70.0, "{row}");
    assert_eq!(row["latency_samples"], 2);

    let text = rig.metrics().await;
    assert!(
        text.contains(r#"humpyard_target_latency_seconds{target="quick"}"#),
        "{text}"
    );
    assert!(
        text.contains(r#"humpyard_target_latency_seconds{target="slow"}"#),
        "{text}"
    );
}

#[tokio::test]
async fn exploration_keeps_sending_some_requests_to_the_slower_target() {
    let rig = rig("explore = 1.0\nmin_samples = 2\nseed = 7", "").await;
    rig.served(6).await;
    let served = rig.served(40).await;
    assert!(served.iter().any(|t| t == "slow"), "{served:?}");
    assert!(served.iter().any(|t| t == "quick"), "{served:?}");
    let decision = rig.newest_decision().await;
    assert!(["fastest", "explore"].contains(&decision["ranking"]["reason"].as_str().unwrap()));
}

#[tokio::test]
async fn a_failed_attempt_is_not_a_speed_sample_and_the_request_falls_over() {
    let rig = rig(NO_EXPLORING, "").await;
    rig.served(6).await;
    let samples_of = |health: &Value| {
        health["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["target"] == "quick")
            .unwrap()["latency_samples"]
            .as_u64()
            .unwrap()
    };
    let before = samples_of(&rig.get("/v1/health").await);
    let figure_before = rig.get("/v1/health").await;

    rig.quick.set_status(500);
    let (status, target) = rig.ask().await;
    assert_eq!(
        (status, target.as_str()),
        (200, "slow"),
        "the fast target failed over to the slow one"
    );
    let after = rig.get("/v1/health").await;
    assert_eq!(
        samples_of(&after),
        before,
        "the failure did not count as a sample"
    );
    let quick_ms = |h: &Value| {
        h["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["target"] == "quick")
            .unwrap()["current_latency_ms"]
            .as_f64()
            .unwrap()
    };
    assert!((quick_ms(&after) - quick_ms(&figure_before)).abs() < f64::EPSILON);
}

#[tokio::test]
async fn ordering_never_brings_back_a_target_that_eligibility_removed() {
    // `quick` is public and the route allows only private targets, so it can never be tried however
    // fast it is.
    let toml_class = r#"class = "private""#;
    let (_slow, slow_url) = upstream(80).await;
    let (_quick, quick_url) = upstream(0).await;
    let dir = tempfile::tempdir().unwrap();
    let (key, key_hash) = humpyard::auth::generate_key().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[keys.alice]
sha256 = "{key_hash}"
[providers.ps]
base_url = "{slow_url}"
api_key_env = "K"
max_retries = 0
[providers.pq]
base_url = "{quick_url}"
api_key_env = "K"
max_retries = 0
[targets.slow]
{toml_class}
endpoints = [{{ provider = "ps", model = "m" }}]
[targets.quick]
class = "public"
endpoints = [{{ provider = "pq", model = "m" }}]
[routes.pool]
type = "fastest"
targets = ["slow", "quick"]
allowed_classes = ["private"]
explore = 1.0
min_samples = 1
"#
    );
    let _ = dir;
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    let url = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    for _ in 0..12 {
        let resp = reqwest::Client::new()
            .post(format!("{url}/v1/chat/completions"))
            .bearer_auth(&key)
            .json(&chat_request("pool", false))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(resp.headers()["x-humpyard-target"], "slow");
    }
}

#[test]
fn the_route_settings_are_validated() {
    let check = |extra: &str| {
        Config::from_toml(
            &format!(
                r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "K"
[targets.t]
endpoints = [{{ provider = "p", model = "m" }}]
[routes.r]
type = "fastest"
targets = ["t"]
{extra}
"#
            ),
            |_| Some("k".into()),
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    };
    assert!(check("").is_ok());
    assert!(check("explore = 0.5\nmin_samples = 5\nmax_age_secs = 60\nseed = 3").is_ok());
    assert!(check("explore = 1.5").unwrap_err().contains("explore"));
    assert!(check("explore = -0.1").unwrap_err().contains("explore"));
    assert!(
        check("min_samples = 0")
            .unwrap_err()
            .contains("min_samples")
    );
    assert!(
        check("max_age_secs = 0")
            .unwrap_err()
            .contains("max_age_secs")
    );
    assert!(check("colour = 1").is_err(), "unknown settings are refused");
}

#[tokio::test]
async fn an_older_ledger_gains_the_ranking_column_and_keeps_its_decisions() {
    use sqlx::Connection;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    let ledger = Ledger::open(&path).await.unwrap();
    ledger.flush().await;
    ledger.shutdown().await;
    // A version-7 file: no `ranking` column. Built offline so no live pool holds a stale statement.
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(&path);
    let mut conn = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    for statement in [
        "ALTER TABLE decisions DROP COLUMN ranking",
        "PRAGMA user_version = 7",
    ] {
        sqlx::query(statement).execute(&mut conn).await.unwrap();
    }
    conn.close().await.unwrap();

    let ledger = Ledger::open(&path).await.unwrap();
    ledger.flush().await;
    let scan = ledger
        .session_scan(&SessionQuery {
            key: None,
            since_ms: 0,
            row_cap: 10,
        })
        .await;
    assert!(scan.is_ok(), "the upgraded ledger opens and reads");
    ledger.shutdown().await;
    let mut conn = sqlx::SqliteConnection::connect_with(&options.read_only(true))
        .await
        .unwrap();
    let has: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('decisions') WHERE name = 'ranking'",
    )
    .fetch_one(&mut conn)
    .await
    .unwrap();
    assert_eq!(has, 1, "the upgrade added the column");
}
