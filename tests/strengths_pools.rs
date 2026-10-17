//! Strengths and pool routes (ADR 0021): labels on targets, pools resolved when the configuration loads,
//! explain showing members and why a target was left out, and strengths never removing a target.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::post;
use common::{chat_completion, chat_request, logs, serve, usage};
use humpyard::config::Config;
use humpyard::server::{self, Reload};
use serde_json::{Value, json};
use tokio::sync::mpsc;

const HEADER: &str = r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "http://127.0.0.1:1"
api_key_env = "PATH"
"#;

fn load(body: &str) -> Result<Config, String> {
    Config::from_toml(&format!("{HEADER}{body}"), |n| std::env::var(n).ok())
        .map_err(|e| e.to_string())
}

/// Targets a, b, c carry `general`; d carries `coding`; e has none. Windows: a 64000, b 8000, c none.
const TARGETS: &str = r#"
[targets.c]
strengths = ["general", "fast"]
endpoints = [{ provider = "p", model = "mc" }]
[targets.a]
strengths = ["general", "coding"]
endpoints = [{ provider = "p", model = "ma", context_window = 64000 }]
[targets.b]
strengths = ["general"]
endpoints = [{ provider = "p", model = "mb", context_window = 8000 }]
[targets.d]
strengths = ["coding"]
endpoints = [{ provider = "p", model = "md", context_window = 128000 }]
[targets.e]
endpoints = [{ provider = "p", model = "me" }]
"#;

fn pool_route(extra: &str) -> String {
    format!("{TARGETS}\n[routes.pool]\ntype = \"passthrough\"\n{extra}\n")
}

#[test]
fn strengths_are_declared_on_targets() {
    let config = load(&pool_route("pool = { strength = \"general\" }")).unwrap();
    assert_eq!(config.target_strengths["a"], ["general", "coding"]);
    assert!(
        !config.target_strengths.contains_key("e"),
        "only targets that declare some"
    );
}

#[test]
fn invalid_strengths_are_refused_with_the_target_and_label() {
    let check = |strengths: &str| {
        load(&format!(
            "[targets.t]\nstrengths = {strengths}\nendpoints = [{{ provider = \"p\", model = \"m\" }}]\n"
        ))
        .map(|_| ())
    };
    assert!(check(r#"["coding", "general"]"#).is_ok());
    for bad in [
        r#"[""]"#,
        r#"["Coding"]"#,
        r#"["has space"]"#,
        r#"["a", "a"]"#,
    ] {
        let error = check(bad).unwrap_err();
        assert!(
            error.contains("target `t`") && error.contains("strength"),
            "{bad}: {error}"
        );
    }
    let long = format!("[\"{}\"]", "x".repeat(33));
    assert!(check(&long).unwrap_err().contains("strength"));
    let nine = format!(
        "[{}]",
        (0..9)
            .map(|i| format!("\"s{i}\""))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(check(&nine).unwrap_err().contains("at most 8"));
    assert!(check(r#"["ok-1_x"]"#).is_ok());
}

#[test]
fn a_pool_takes_every_target_with_the_strength_sorted_by_name() {
    let config = load(&pool_route("pool = { strength = \"general\" }")).unwrap();
    let route = &config.routes["pool"];
    assert_eq!(
        route.target_refs(),
        ["a", "b", "c"],
        "d and e do not carry it"
    );
    let report = &config.pools["pool"];
    assert_eq!(report.members, ["a", "b", "c"]);
    assert_eq!(report.left_out, []);
}

#[test]
fn a_minimum_window_counts_only_declared_windows_and_says_who_was_left_out() {
    let config = load(&pool_route(
        "pool = { strength = \"general\", min_context_window = 32000 }",
    ))
    .unwrap();
    let report = &config.pools["pool"];
    assert_eq!(report.members, ["a"]);
    let left: Vec<(&str, &str)> = report
        .left_out
        .iter()
        .map(|l| (l.target.as_str(), l.reason))
        .collect();
    assert_eq!(
        left,
        [
            ("b", "context_window_below"),
            ("c", "context_window_undeclared")
        ]
    );
    assert_eq!(config.routes["pool"].target_refs(), ["a"]);
}

#[test]
fn a_target_qualifies_when_any_endpoint_declares_enough() {
    let body = r#"
[targets.multi]
strengths = ["general"]
endpoints = [
  { provider = "p", model = "small", context_window = 4000 },
  { provider = "p", model = "big", context_window = 100000 },
]
[routes.pool]
type = "passthrough"
pool = { strength = "general", min_context_window = 32000 }
"#;
    assert_eq!(load(body).unwrap().pools["pool"].members, ["multi"]);
}

#[test]
fn an_empty_pool_is_refused_and_says_what_exists() {
    let unknown = load(&pool_route("pool = { strength = \"genral\" }")).unwrap_err();
    assert!(
        unknown.contains("route `pool`") && unknown.contains("`genral`"),
        "{unknown}"
    );
    assert!(
        unknown.contains("coding") && unknown.contains("general"),
        "lists what exists: {unknown}"
    );

    let none = load(
        "[targets.t]\nendpoints = [{ provider = \"p\", model = \"m\" }]\n[routes.r]\ntype = \"passthrough\"\npool = { strength = \"x\" }\n",
    )
    .unwrap_err();
    assert!(none.contains("no target declares any strengths"), "{none}");

    let too_big = load(&pool_route(
        "pool = { strength = \"general\", min_context_window = 1000000 }",
    ))
    .unwrap_err();
    assert!(
        too_big.contains("left out") && too_big.contains("context_window_below"),
        "{too_big}"
    );
}

#[test]
fn pool_is_exclusive_with_targets_and_weights_and_for_three_route_types_only() {
    let both = load(&pool_route(
        "pool = { strength = \"general\" }\ntargets = [\"a\"]",
    ))
    .unwrap_err();
    assert!(both.contains("cannot be combined"), "{both}");

    let weights = load(&format!(
        "{TARGETS}\n[routes.r]\ntype = \"random\"\nweights = [1.0]\npool = {{ strength = \"general\" }}\n"
    ))
    .unwrap_err();
    assert!(weights.contains("weights"), "{weights}");

    for kind in ["passthrough", "fastest", "random"] {
        let ok = load(&format!(
            "{TARGETS}\n[routes.r]\ntype = \"{kind}\"\npool = {{ strength = \"general\" }}\n"
        ));
        assert!(ok.is_ok(), "{kind}: {ok:?}");
    }
    let tiered = load(&format!(
        "{TARGETS}\n[routes.r]\ntype = \"stage_router\"\nefficient = [\"a\"]\ncapable = [\"b\"]\npool = {{ strength = \"general\" }}\n"
    ));
    assert!(tiered.is_err(), "stage routers keep explicit tiers");

    assert!(
        load(&pool_route(
            "pool = { strength = \"general\", min_context_window = 0 }"
        ))
        .unwrap_err()
        .contains("min_context_window")
    );
    assert!(load(&pool_route("pool = { strength = \"General\" }")).is_err());
    assert!(load(&pool_route("pool = { strength = \"general\", colour = 1 }")).is_err());
}

struct Upstream;

async fn completions(State(_): State<Arc<Upstream>>) -> axum::response::Response {
    axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response()
}

async fn mock() -> String {
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(Arc::new(Upstream));
    format!("http://{}", serve(app).await)
}

/// Three free general targets (one on a strict private route's class `public`) and one private.
fn gateway_config(url: &str, extra_strength: &str) -> String {
    format!(
        r#"listen = "127.0.0.1:0"
[providers.p]
base_url = "{url}"
api_key_env = "PATH"
max_retries = 0
[targets.free-a]
class = "public"
strengths = ["general"]
endpoints = [{{ provider = "p", model = "fa", context_window = 64000 }}]
[targets.free-b]
class = "public"
strengths = ["general"{extra_strength}]
endpoints = [{{ provider = "p", model = "fb", context_window = 64000 }}]
[targets.home]
class = "private"
strengths = ["general"]
endpoints = [{{ provider = "p", model = "home", context_window = 64000 }}]
[routes.pool]
type = "fastest"
pool = {{ strength = "general", min_context_window = 32000 }}
[routes.private-pool]
type = "passthrough"
pool = {{ strength = "general" }}
allowed_classes = ["private"]
"#
    )
}

#[tokio::test]
async fn a_pool_route_serves_and_explain_shows_its_members() {
    let url = mock().await;
    let config = Config::from_toml(&gateway_config(&url, ""), |n| std::env::var(n).ok()).unwrap();
    let base = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    let resp = reqwest::Client::new()
        .post(format!("{base}/v1/chat/completions"))
        .json(&chat_request("pool", false))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let explained: Value = reqwest::Client::new()
        .post(format!("{base}/v1/route/explain"))
        .json(&json!({"model": "pool"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(explained["pool"]["strength"], "general");
    assert_eq!(explained["pool"]["min_context_window"], 32000);
    assert_eq!(
        explained["pool"]["members"],
        json!(["free-a", "free-b", "home"])
    );
    assert_eq!(explained["pool"]["left_out"], json!([]));
    assert_eq!(
        explained["ordering"]["type"], "fastest",
        "a pool composes with latency ordering"
    );
}

#[tokio::test]
async fn a_strict_route_narrows_a_pool_per_request_and_never_widens_it() {
    let url = mock().await;
    let config = Config::from_toml(&gateway_config(&url, ""), |n| std::env::var(n).ok()).unwrap();
    let base = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    for _ in 0..6 {
        let resp = reqwest::Client::new()
            .post(format!("{base}/v1/chat/completions"))
            .json(&chat_request("private-pool", false))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers()["x-humpyard-target"],
            "home",
            "the public members are removed"
        );
    }
}

#[tokio::test]
async fn a_reload_that_moves_a_strength_rebuilds_the_pool_and_logs_it() {
    logs();
    let url = mock().await;
    let before = gateway_config(&url, "");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("humpyard.toml");
    std::fs::write(&path, &before).unwrap();
    let config = Config::from_toml(&before, |n| std::env::var(n).ok()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (reload, triggers) = mpsc::channel(4);
    let (_stop, signals) = mpsc::channel(4);
    tokio::spawn(server::run(
        config,
        listener,
        signals,
        Some(Reload {
            path: path.clone(),
            triggers,
        }),
    ));
    let members = |base: String| async move {
        let explained: Value = reqwest::Client::new()
            .post(format!("{base}/v1/route/explain"))
            .json(&json!({"model": "pool"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        explained["pool"]["members"].clone()
    };
    assert_eq!(
        members(base.clone()).await,
        json!(["free-a", "free-b", "home"])
    );

    // Drop `general` from free-b: the pool shrinks without anyone editing the route.
    let after = before.replace(
        "[targets.free-b]\nclass = \"public\"\nstrengths = [\"general\"]",
        "[targets.free-b]\nclass = \"public\"\nstrengths = [\"coding\"]",
    );
    assert_ne!(before, after);
    std::fs::write(&path, &after).unwrap();
    reload.send(()).await.unwrap();
    for _ in 0..100 {
        if members(base.clone()).await == json!(["free-a", "home"]) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(members(base.clone()).await, json!(["free-a", "home"]));
    let captured = logs().contents();
    assert!(
        captured.contains("pool membership changed") && captured.contains("free-a,home"),
        "{captured}"
    );
}
