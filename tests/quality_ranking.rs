//! Declared quality (ADR 0020, amended): a pool's `min_quality` floor, and a `fastest` route's
//! `quality_weight` that blends observed speed with quality. Weight 0 is exactly the speed order.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;
use serde_json::{Value, json};

fn load(toml: &str) -> Result<Config, String> {
    Config::from_toml(toml, |n| std::env::var(n).ok()).map_err(|e| format!("{e}"))
}

const HEAD: &str = r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "PATH"
"#;

/// General-purpose targets with qualities 0.9, 0.5, none and 0.7, and windows 64000, 64000, 64000, 8000.
const TARGETS: &str = r#"
[targets.a]
strengths = ["general"]
quality = 0.9
endpoints = [{ provider = "p", model = "a", context_window = 64000 }]
[targets.b]
strengths = ["general"]
quality = 0.5
endpoints = [{ provider = "p", model = "b", context_window = 64000 }]
[targets.c]
strengths = ["general"]
endpoints = [{ provider = "p", model = "c", context_window = 64000 }]
[targets.d]
strengths = ["general"]
quality = 0.7
endpoints = [{ provider = "p", model = "d", context_window = 8000 }]
"#;

fn pool(extra: &str) -> String {
    format!(
        "{HEAD}{TARGETS}\n[routes.pool]\ntype = \"fastest\"\npool = {{ strength = \"general\"{extra} }}\n"
    )
}

#[test]
fn a_quality_floor_keeps_only_targets_that_declare_enough() {
    let config = load(&pool(", min_quality = 0.6")).unwrap();
    let report = &config.pools["pool"];
    assert_eq!(report.members, ["a", "d"]);
    assert_eq!(report.min_quality, Some(0.6));
    let left: Vec<(&str, &str)> = report
        .left_out
        .iter()
        .map(|l| (l.target.as_str(), l.reason))
        .collect();
    assert_eq!(left, [("b", "quality_below"), ("c", "quality_undeclared")]);
    assert_eq!(config.target_quality["a"], 0.9);
    assert!(!config.target_quality.contains_key("c"));
}

#[test]
fn the_floor_combines_with_a_context_window_and_both_reasons_are_reported() {
    let config = load(&pool(", min_quality = 0.6, min_context_window = 32000")).unwrap();
    let report = &config.pools["pool"];
    assert_eq!(
        report.members,
        ["a"],
        "d has the quality but a window that is too small"
    );
    let reasons: Vec<&str> = report.left_out.iter().map(|l| l.reason).collect();
    assert!(reasons.contains(&"context_window_below"), "{reasons:?}");
    assert!(
        reasons.contains(&"quality_below") && reasons.contains(&"quality_undeclared"),
        "{reasons:?}"
    );
}

#[test]
fn a_floor_nothing_meets_is_a_configuration_error_that_says_why() {
    let error = load(&pool(", min_quality = 0.95")).unwrap_err();
    assert!(
        error.contains("route `pool`") && error.contains("quality_below"),
        "{error}"
    );
}

#[test]
fn quality_settings_are_range_checked() {
    for bad in [", min_quality = 1.5", ", min_quality = -0.1"] {
        assert!(
            load(&pool(bad)).unwrap_err().contains("min_quality"),
            "{bad}"
        );
    }
    let target = |q: &str| {
        load(&format!(
            "{HEAD}[targets.t]\nquality = {q}\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n"
        ))
        .map(|_| ())
    };
    assert!(target("0.0").is_ok() && target("1.0").is_ok() && target("0.73").is_ok());
    assert!(target("1.2").unwrap_err().contains("quality"));
    assert!(target("-0.5").unwrap_err().contains("quality"));
    let route = |extra: &str| {
        load(&format!(
            "{HEAD}[targets.t]\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n[routes.r]\ntype = \"fastest\"\ntargets = [\"t\"]\n{extra}\n"
        ))
        .map(|_| ())
    };
    assert!(route("quality_weight = 0.7\nlatency_target_ms = 500\nunknown_quality = 0.4").is_ok());
    assert!(
        route("quality_weight = 1.5")
            .unwrap_err()
            .contains("quality_weight")
    );
    assert!(
        route("unknown_quality = 2.0")
            .unwrap_err()
            .contains("unknown_quality")
    );
    assert!(
        route("latency_target_ms = 0")
            .unwrap_err()
            .contains("latency_target_ms")
    );
}

struct Upstream {
    delay_ms: AtomicU64,
}

async fn completions(State(up): State<Arc<Upstream>>) -> axum::response::Response {
    tokio::time::sleep(Duration::from_millis(up.delay_ms.load(Ordering::SeqCst))).await;
    axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response()
}

async fn upstream(delay_ms: u64) -> String {
    let up = Arc::new(Upstream {
        delay_ms: AtomicU64::new(delay_ms),
    });
    format!(
        "http://{}",
        serve(
            Router::new()
                .route("/chat/completions", post(completions))
                .with_state(up)
        )
        .await
    )
}

/// `quick` answers at once with quality 0.4; `strong` takes about 150 ms with quality 0.8; `plain` takes
/// about 150 ms and declares nothing. Routes: `speedy` (no weight) and `blended` (weight 0.7, target 100 ms).
async fn gateway() -> String {
    let (quick, strong, plain) = (upstream(0).await, upstream(150).await, upstream(150).await);
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[providers.pq]
base_url = "{quick}"
api_key_env = "PATH"
max_retries = 0
[providers.ps]
base_url = "{strong}"
api_key_env = "PATH"
max_retries = 0
[providers.pp]
base_url = "{plain}"
api_key_env = "PATH"
max_retries = 0
[targets.quick]
quality = 0.4
endpoints = [{{ provider = "pq", model = "m" }}]
[targets.strong]
quality = 0.8
endpoints = [{{ provider = "ps", model = "m" }}]
[targets.plain]
endpoints = [{{ provider = "pp", model = "m" }}]
[routes.speedy]
type = "fastest"
targets = ["quick", "strong"]
explore = 0.0
min_samples = 2
[routes.blended]
type = "fastest"
targets = ["quick", "strong"]
explore = 0.0
min_samples = 2
quality_weight = 0.7
latency_target_ms = 100
[routes.blended-unknown]
type = "fastest"
targets = ["quick", "plain"]
explore = 0.0
min_samples = 2
quality_weight = 0.7
latency_target_ms = 100
"#
    );
    let config = Config::from_toml(&toml, |n| std::env::var(n).ok()).unwrap();
    format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    )
}

async fn ask(base: &str, route: &str) -> String {
    let resp = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .json(&chat_request(route, false))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    resp.headers()["x-humpyard-target"]
        .to_str()
        .unwrap()
        .to_string()
}

async fn explain(base: &str, route: &str) -> Value {
    reqwest::Client::new()
        .post(format!("{base}/v1/route/explain"))
        .json(&json!({"model": route}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn a_quality_weight_changes_who_leads_and_weight_zero_does_not() {
    let base = gateway().await;
    // Warm both targets up through the unweighted route (it is measured, then faster-first).
    let mut served = Vec::new();
    for _ in 0..8 {
        served.push(ask(&base, "speedy").await);
    }
    assert_eq!(
        &served[..4],
        ["quick", "quick", "strong", "strong"],
        "unmeasured first, in written order"
    );
    assert!(
        served[4..].iter().all(|t| t == "quick"),
        "then the faster one: {served:?}"
    );
    // The same measured figures, a quality weight: the stronger model leads despite being slower.
    // quick: 0.3*1.0 + 0.7*0.4 = 0.58; strong: 0.3*(100/150) + 0.7*0.8 = 0.76.
    for _ in 0..3 {
        assert_eq!(ask(&base, "blended").await, "strong");
    }
    // And weight 0 stays the speed order on the same data.
    assert_eq!(ask(&base, "speedy").await, "quick");
}

#[tokio::test]
async fn an_undeclared_quality_ranks_as_the_neutral_value() {
    let base = gateway().await;
    for _ in 0..8 {
        ask(&base, "blended-unknown").await;
    }
    // quick (0.4) vs plain (unknown = 0.5, speed 100/150): 0.3 + 0.28 = 0.58 vs 0.3*0.667 + 0.35 = 0.55.
    assert_eq!(ask(&base, "blended-unknown").await, "quick");
}

#[tokio::test]
async fn explain_shows_the_qualities_and_the_scores() {
    let base = gateway().await;
    for _ in 0..8 {
        ask(&base, "speedy").await;
    }
    let report = explain(&base, "blended").await;
    assert_eq!(report["ordering"]["quality_weight"], 0.7);
    assert_eq!(report["ordering"]["order"], json!(["strong", "quick"]));
    let scores = report["ordering"]["scores"].as_array().unwrap();
    let strong = scores.iter().find(|s| s["target"] == "strong").unwrap();
    assert_eq!(strong["quality"], 0.8);
    let score = strong["score"].as_f64().unwrap();
    assert!((score - 0.76).abs() < 0.03, "{score}");
    assert!(strong["speed"].as_f64().unwrap() < 1.0);
    let targets = report["targets"].as_array().unwrap();
    assert_eq!(targets[0]["quality"], 0.8);
    // An unweighted route shows speeds but no blended score.
    let plain = explain(&base, "speedy").await;
    assert_eq!(plain["ordering"]["quality_weight"], 0.0);
    assert!(plain["ordering"]["scores"][0]["score"].is_null());
}
