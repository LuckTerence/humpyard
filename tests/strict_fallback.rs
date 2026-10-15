//! Strict fallback (ADR 0017): a route with `allowed_classes` never calls a target of another class,
//! by construction; the default is unchanged; the exclusion is recorded, explained and counted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, AtomicUsize, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::clock::Clock;
use humpyard::config::Config;
use humpyard::server;
use serde_json::{Value, json};

struct Upstream {
    status: AtomicU16,
    calls: AtomicUsize,
}

impl Upstream {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

async fn completions(State(up): State<Arc<Upstream>>) -> axum::response::Response {
    up.calls.fetch_add(1, Ordering::SeqCst);
    match up.status.load(Ordering::SeqCst) {
        200 => axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response(),
        code => (
            StatusCode::from_u16(code).unwrap(),
            axum::Json(json!({"error": {"message": "down"}})),
        )
            .into_response(),
    }
}

async fn upstream(status: u16) -> (Arc<Upstream>, String) {
    let up = Arc::new(Upstream {
        status: AtomicU16::new(status),
        calls: AtomicUsize::new(0),
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
    private: Arc<Upstream>,
    public: Arc<Upstream>,
    _dir: tempfile::TempDir,
}

/// Targets `priv` (class private) and `pub` (class public); routes:
/// `strict` and `lenient` list both, in that order, and only `strict` sets `allowed_classes`;
/// `tiered` is a stage router whose efficient tier is public and capable tier private.
async fn rig(private_status: u16) -> Rig {
    let (private, private_url) = upstream(private_status).await;
    let (public, public_url) = upstream(200).await;
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
failure_threshold = 1
cooldown_secs = 60
[keys.alice]
sha256 = "{key_hash}"
[providers.pp]
base_url = "{private_url}"
api_key_env = "K"
max_retries = 0
[providers.pq]
base_url = "{public_url}"
api_key_env = "K"
max_retries = 0
[targets.priv]
class = "private"
endpoints = [{{ provider = "pp", model = "m" }}]
[targets.pub]
class = "public"
endpoints = [{{ provider = "pq", model = "m" }}]
[routes.strict]
type = "passthrough"
targets = ["priv", "pub"]
allowed_classes = ["private"]
[routes.lenient]
type = "passthrough"
targets = ["priv", "pub"]
[routes.tiered]
type = "stage_router"
efficient = ["pub"]
capable = ["priv"]
allowed_classes = ["private"]
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
        private,
        public,
        _dir: dir,
    }
}

impl Rig {
    async fn ask(&self, route: &str) -> u16 {
        reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.key)
            .json(&chat_request(route, false))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    async fn get(&self, path: &str, bearer: &str) -> reqwest::Response {
        reqwest::Client::new()
            .get(format!("{}{path}", self.url))
            .bearer_auth(bearer)
            .send()
            .await
            .unwrap()
    }

    async fn explain(&self, route: &str) -> Value {
        reqwest::Client::new()
            .post(format!("{}/v1/route/explain", self.url))
            .bearer_auth(&self.key)
            .json(&json!({"model": route}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    /// The newest decision for `route`, waiting for the ledger's asynchronous writer.
    async fn decision(&self, route: &str) -> Value {
        for _ in 0..100 {
            let body: Value = self
                .get(&format!("/v1/decisions?route={route}&limit=1"), &self.key)
                .await
                .json()
                .await
                .unwrap();
            if let Some(first) = body["data"].as_array().and_then(|d| d.first()) {
                return first.clone();
            }
            tokio::time::sleep(Duration::from_millis(30)).await;
        }
        panic!("no decision for {route}");
    }

    async fn metrics(&self) -> String {
        self.get("/metrics", &self.credential)
            .await
            .text()
            .await
            .unwrap()
    }
}

#[tokio::test]
async fn a_strict_route_never_calls_a_disallowed_target() {
    let rig = rig(503).await;
    let status = rig.ask("strict").await;
    assert!(
        status >= 500,
        "the private target's failure reaches the client: {status}"
    );
    assert_eq!(rig.private.calls(), 1);
    assert_eq!(rig.public.calls(), 0, "the public target is never called");

    let decision = rig.decision("strict").await;
    assert_eq!(decision["excluded_targets"], json!(["pub"]));
    assert_eq!(decision["eligible_targets"], json!(["priv"]));
    let attempts = decision["attempts"].as_array().unwrap();
    assert!(attempts.iter().all(|a| a["target"] == "priv"));
    assert_eq!(decision["fallback_crossed_classes"], json!(false));
}

#[tokio::test]
async fn the_default_still_falls_back_across_classes() {
    let rig = rig(503).await;
    assert_eq!(
        rig.ask("lenient").await,
        200,
        "the public target takes over"
    );
    assert_eq!(rig.public.calls(), 1);
    let decision = rig.decision("lenient").await;
    assert_eq!(decision["excluded_targets"], json!([]));
    assert_eq!(decision["fallback_crossed_classes"], json!(true));
}

#[tokio::test]
async fn tier_substitution_stays_inside_the_allowed_set() {
    let rig = rig(200).await;
    assert_eq!(
        rig.ask("tiered").await,
        200,
        "the efficient tier is empty, so the capable (private) tier serves"
    );
    assert_eq!(rig.private.calls(), 1);
    assert_eq!(rig.public.calls(), 0);
}

#[tokio::test]
async fn a_cooling_down_allowed_target_is_still_tried_and_nothing_is_added() {
    let rig = rig(503).await;
    for _ in 0..3 {
        assert!(rig.ask("strict").await >= 500);
    }
    assert_eq!(
        rig.private.calls(),
        3,
        "health never denies service: the private target is tried every time"
    );
    assert_eq!(
        rig.public.calls(),
        0,
        "and the public target is never added"
    );
}

#[tokio::test]
async fn explain_shows_what_is_allowed_and_what_is_excluded() {
    let rig = rig(200).await;
    let strict = rig.explain("strict").await;
    assert_eq!(strict["allowed_classes"], json!(["private"]));
    assert_eq!(strict["fallback_can_cross_classes"], json!(false));
    assert_eq!(strict["classes"], json!(["private"]));
    assert_eq!(strict["excluded_targets"][0]["target"], "pub");
    assert_eq!(strict["excluded_targets"][0]["class"], "public");
    let planned: Vec<&str> = strict["targets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["target"].as_str().unwrap())
        .collect();
    assert_eq!(planned, vec!["priv"]);

    let lenient = rig.explain("lenient").await;
    assert_eq!(lenient["allowed_classes"], Value::Null);
    assert_eq!(lenient["fallback_can_cross_classes"], json!(true));
    assert_eq!(lenient["excluded_targets"], json!([]));
}

#[tokio::test]
async fn a_blocked_fallback_is_counted_only_where_it_happened() {
    let rig = rig(503).await;
    assert!(rig.ask("strict").await >= 500);
    assert_eq!(rig.ask("lenient").await, 200);
    let text = rig.metrics().await;
    assert!(
        text.contains(r#"humpyard_fallback_blocked_total{route="strict"} 1"#),
        "{text}"
    );
    assert!(
        !text.contains(r#"fallback_blocked_total{route="lenient"}"#),
        "a lenient route never blocks"
    );

    let ok = self::rig(200).await;
    assert_eq!(ok.ask("strict").await, 200);
    assert!(
        !ok.metrics()
            .await
            .contains("humpyard_fallback_blocked_total{"),
        "a strict route that succeeds blocked nothing"
    );
}

fn config_error(routes: &str, targets: &str) -> String {
    let toml = format!(
        r#"listen = "127.0.0.1:8080"
[providers.p]
base_url = "http://localhost:1/v1"
api_key_env = "K"
{targets}
{routes}
"#
    );
    Config::from_toml(&toml, |_| Some("k".into()))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default()
}

const TWO_TARGETS: &str = r#"
[targets.a]
class = "private"
endpoints = [{ provider = "p", model = "m" }]
[targets.b]
class = "public"
endpoints = [{ provider = "p", model = "m" }]
"#;

#[test]
fn strict_routes_are_validated_when_the_config_loads() {
    let route = |extra: &str| {
        format!("[routes.r]\ntype = \"passthrough\"\ntargets = [\"a\", \"b\"]\n{extra}\n")
    };
    assert_eq!(
        config_error(&route("allowed_classes = [\"private\"]"), TWO_TARGETS),
        "",
        "a well-formed strict route loads"
    );
    assert!(
        config_error(&route("allowed_classes = []"), TWO_TARGETS).contains("at least one class")
    );
    assert!(
        config_error(&route("allowed_classes = [\"Private!\"]"), TWO_TARGETS)
            .contains("allowed class `Private!`")
    );
    assert!(
        config_error(&route("allowed_classes = [\"paid\"]"), TWO_TARGETS)
            .contains("could never serve")
    );
    let unclassed = r#"
[targets.a]
class = "private"
endpoints = [{ provider = "p", model = "m" }]
[targets.b]
endpoints = [{ provider = "p", model = "m" }]
"#;
    let message = config_error(&route("allowed_classes = [\"private\"]"), unclassed);
    assert!(
        message.contains("route `r`") && message.contains("target `b` has no class"),
        "{message}"
    );
    assert_eq!(
        config_error(&route(""), unclassed),
        "",
        "without allowed_classes, unclassed targets are fine as before"
    );
}

/// A version-3 database, built offline on its own connection (no live ledger pool is altered, so no
/// connection holds a statement prepared against the newer schema): a fresh ledger's file, with the
/// `excluded_targets` column dropped, one legacy decision row inserted and the version rolled back.
async fn legacy_v3_database(path: &std::path::Path) {
    use sqlx::Connection;
    humpyard::ledger::Ledger::open(path)
        .await
        .unwrap()
        .shutdown()
        .await;
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(path);
    let mut conn = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    for statement in [
        "ALTER TABLE decisions DROP COLUMN excluded_targets",
        "INSERT INTO decisions (request_id, ts_ms, format, requested_model, skipped_rules, \
         eligible_targets, stream, tools, images, attempts, status, outcome, latency_ms) \
         VALUES ('before', 1, 'openai_chat', 'm', '[]', '[]', 0, 0, 0, '[]', 200, 'ok', 5)",
        "PRAGMA user_version = 3",
    ] {
        sqlx::query(statement).execute(&mut conn).await.unwrap();
    }
    conn.close().await.unwrap();
}

#[tokio::test]
async fn an_older_ledger_is_upgraded_and_keeps_its_decisions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    legacy_v3_database(&path).await;

    let ledger = humpyard::ledger::Ledger::open(&path).await.unwrap();
    ledger.record_decision(humpyard::ledger::Decision {
        request_id: "after".into(),
        ts_ms: humpyard::clock::SystemClock.now_ms(),
        format: "openai_chat".into(),
        outcome: "ok".into(),
        status: 200,
        excluded_targets: vec!["pub".into()],
        ..humpyard::ledger::Decision::default()
    });
    ledger.flush().await;
    let rows = ledger
        .decisions(&humpyard::ledger::DecisionQuery {
            limit: 10,
            ..humpyard::ledger::DecisionQuery::default()
        })
        .await
        .unwrap();
    let by_id = |id: &str| rows.iter().find(|d| d.request_id == id).unwrap();
    assert_eq!(by_id("before").excluded_targets, Vec::<String>::new());
    assert_eq!(by_id("after").excluded_targets, vec!["pub".to_string()]);
}
