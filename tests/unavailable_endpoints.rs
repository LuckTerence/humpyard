//! Unavailable endpoints (ADR 0023): a model that is gone (404), a rejected key (401) or a missing-model
//! code counts against its endpoint and the request moves on, by default on `fastest` and pool routes only;
//! request errors (400, 422) still end the request. Dead targets cool, are skipped, and are probed again.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::clock::{Clock, SystemClock};
use humpyard::config::Config;
use humpyard::server::{self, Options};
use serde_json::{Value, json};

struct TestClock(AtomicI64);
impl Clock for TestClock {
    fn now_ms(&self) -> i64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// A mock upstream: a status, an error body, and a count of the calls it received.
struct Upstream {
    status: Mutex<(u16, Value)>,
    calls: AtomicUsize,
}

impl Upstream {
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

async fn completions(State(up): State<Arc<Upstream>>) -> Response {
    up.calls.fetch_add(1, Ordering::SeqCst);
    let (status, body) = up.status.lock().unwrap().clone();
    if status == 200 {
        return axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))).into_response();
    }
    (StatusCode::from_u16(status).unwrap(), axum::Json(body)).into_response()
}

async fn upstream(status: u16, body: Value) -> (Arc<Upstream>, String) {
    let up = Arc::new(Upstream {
        status: Mutex::new((status, body)),
        calls: AtomicUsize::new(0),
    });
    let app = Router::new()
        .route("/chat/completions", post(completions))
        .with_state(up.clone());
    (up, format!("http://{}", serve(app).await))
}

fn not_found() -> Value {
    json!({"error": {"message": "The model `x` does not exist", "code": "model_not_found"}})
}

fn plain() -> Value {
    json!({"error": {"message": "nope"}})
}

struct Rig {
    url: String,
    clock: Arc<TestClock>,
    dead: Arc<Upstream>,
    second_dead: Arc<Upstream>,
    live: Arc<Upstream>,
}

/// Targets `a-dead` (private, strengths general), `b-live` (public, general) and `c-live-twin`, plus
/// `multi` (two endpoints: one dead, one live). `routes` is appended to the config.
async fn rig(dead_status: u16, dead_body: Value, routes: &str) -> Rig {
    let (dead, dead_url) = upstream(dead_status, dead_body.clone()).await;
    let (second_dead, second_dead_url) = upstream(dead_status, dead_body).await;
    let (live, live_url) = upstream(200, json!({})).await;
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[health]
failure_threshold = 2
cooldown_secs = 10
max_cooldown_secs = 10
[providers.pd]
base_url = "{dead_url}"
api_key_env = "PATH"
max_retries = 0
[providers.pd2]
base_url = "{second_dead_url}"
api_key_env = "PATH"
max_retries = 0
[providers.pl]
base_url = "{live_url}"
api_key_env = "PATH"
max_retries = 0
[targets.a-dead]
class = "private"
strengths = ["general"]
endpoints = [{{ provider = "pd", model = "m" }}]
[targets.b-live]
class = "public"
strengths = ["general"]
endpoints = [{{ provider = "pl", model = "m" }}]
[targets.c-dead]
class = "public"
strengths = ["general"]
endpoints = [{{ provider = "pd2", model = "m" }}]
[targets.multi]
endpoints = [{{ provider = "pd", model = "m2" }}, {{ provider = "pl", model = "m2" }}]
{routes}
"#
    );
    let clock = Arc::new(TestClock(AtomicI64::new(SystemClock.now_ms())));
    let config = Config::from_toml(&toml, |n| std::env::var(n).ok()).unwrap();
    let app = server::router_with_options(
        config,
        Options {
            clock: Some(clock.clone()),
            ..Options::default()
        },
    )
    .await
    .unwrap();
    Rig {
        url: format!("http://{}", serve(app).await),
        clock,
        dead,
        second_dead,
        live,
    }
}

impl Rig {
    async fn ask(&self, route: &str) -> (u16, String) {
        let resp = reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .json(&chat_request(route, false))
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

    async fn state_of(&self, target: &str) -> String {
        let health: Value = reqwest::Client::new()
            .get(format!("{}/v1/health", self.url))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        health["targets"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["target"] == target)
            .unwrap()["endpoints"][0]["state"]
            .as_str()
            .unwrap()
            .to_string()
    }
}

const FASTEST: &str =
    "[routes.fast]\ntype = \"fastest\"\ntargets = [\"a-dead\", \"b-live\"]\nexplore = 0.0\n";
const POOL: &str =
    "[routes.pool]\ntype = \"fastest\"\npool = { strength = \"general\" }\nexplore = 0.0\n";
const PASS: &str = "[routes.pass]\ntype = \"passthrough\"\ntargets = [\"a-dead\", \"b-live\"]\n";
const PASS_POOL: &str =
    "[routes.passpool]\ntype = \"passthrough\"\npool = { strength = \"general\" }\n";
const PASS_OPT_IN: &str = "[routes.optin]\ntype = \"passthrough\"\ntargets = [\"a-dead\", \"b-live\"]\nfallback_on = [\"unavailable\"]\n";
const FAST_EXACT: &str = "[routes.exact]\ntype = \"fastest\"\ntargets = [\"a-dead\", \"b-live\"]\nexplore = 0.0\nfallback_on = [\"rate_limit\"]\n";

#[tokio::test]
async fn a_dead_first_target_no_longer_fails_a_fastest_route_or_a_pool() {
    for (status, body) in [
        (404, plain()),
        (401, plain()),
        (400, not_found()),
        (404, not_found()),
    ] {
        let rig = rig(status, body.clone(), &format!("{FASTEST}{POOL}")).await;
        for route in ["fast", "pool"] {
            for _ in 0..4 {
                let (code, target) = rig.ask(route).await;
                assert_eq!(
                    (code, target.as_str()),
                    (200, "b-live"),
                    "{route} with a first target answering {status} {body}"
                );
            }
        }
    }
}

#[tokio::test]
async fn request_errors_still_end_the_request() {
    for (status, body) in [(400, plain()), (422, plain())] {
        let rig = rig(status, body, FASTEST).await;
        let (code, _) = rig.ask("fast").await;
        assert_eq!(code, status, "a request error is not an endpoint failure");
        assert_eq!(rig.live.calls(), 0, "the next target is not asked");
        for _ in 0..4 {
            rig.ask("fast").await;
        }
        assert_eq!(
            rig.state_of("a-dead").await,
            "healthy",
            "and it does not cool the endpoint"
        );
    }
}

#[tokio::test]
async fn other_routes_keep_their_behavior_unless_they_opt_in() {
    for status in [404u16, 401] {
        let rig = rig(status, plain(), &format!("{PASS}{PASS_OPT_IN}{FAST_EXACT}")).await;
        // A plain passthrough route returns the error and does not call the second target.
        let (code, _) = rig.ask("pass").await;
        assert_eq!(code, status);
        assert_eq!(rig.live.calls(), 0);
        // An explicit list is taken exactly as written, even on a fastest route.
        let (code, _) = rig.ask("exact").await;
        assert_eq!(code, status);
        assert_eq!(rig.live.calls(), 0);
        // Opting in moves on.
        let (code, target) = rig.ask("optin").await;
        assert_eq!((code, target.as_str()), (200, "b-live"));
    }
}

#[tokio::test]
async fn a_pool_of_any_route_type_moves_on_by_default() {
    let rig = rig(404, plain(), PASS_POOL).await;
    for _ in 0..3 {
        let (code, target) = rig.ask("passpool").await;
        assert_eq!((code, target.as_str()), (200, "b-live"));
    }
}

#[tokio::test]
async fn a_dead_endpoint_cools_is_skipped_and_probed_after_the_cooldown() {
    let rig = rig(404, plain(), FASTEST).await;
    // Two answers in a row (the threshold) cool it.
    for _ in 0..2 {
        assert_eq!(rig.ask("fast").await, (200, "b-live".to_string()));
    }
    assert_eq!(rig.dead.calls(), 2);
    assert_eq!(rig.state_of("a-dead").await, "cooling_down");
    // While it cools it is not asked at all.
    for _ in 0..4 {
        assert_eq!(rig.ask("fast").await.1, "b-live");
    }
    assert_eq!(rig.dead.calls(), 2);
    // After the cooldown one request probes it; it fails again and the request still succeeds.
    rig.clock.0.fetch_add(10_001, Ordering::SeqCst);
    assert_eq!(rig.ask("fast").await, (200, "b-live".to_string()));
    assert_eq!(rig.dead.calls(), 3, "exactly one probe");
    assert_eq!(rig.state_of("a-dead").await, "cooling_down");
}

#[tokio::test]
async fn health_counts_apply_to_every_route_even_where_the_request_is_not_moved_on() {
    // A passthrough route returns the 404, but its dead first target still cools, so the next request
    // skips it and is served.
    let rig = rig(404, plain(), PASS).await;
    for _ in 0..2 {
        assert_eq!(rig.ask("pass").await.0, 404);
    }
    assert_eq!(rig.state_of("a-dead").await, "cooling_down");
    assert_eq!(rig.ask("pass").await, (200, "b-live".to_string()));
}

#[tokio::test]
async fn a_403_counts_against_health_but_keeps_its_own_class() {
    let rig = rig(403, plain(), FASTEST).await;
    // 403 has its own class (forbidden), already allowed by default, so the request moves on.
    for _ in 0..2 {
        assert_eq!(rig.ask("fast").await, (200, "b-live".to_string()));
    }
    assert_eq!(rig.state_of("a-dead").await, "cooling_down");
}

#[tokio::test]
async fn a_target_with_several_endpoints_moves_on_within_itself_on_every_route() {
    let rig = rig(
        404,
        plain(),
        "[routes.one]\ntype = \"passthrough\"\ntargets = [\"multi\"]\n",
    )
    .await;
    for _ in 0..3 {
        assert_eq!(rig.ask("one").await.0, 200, "the second endpoint serves");
    }
    assert!(rig.live.calls() >= 3);
}

#[tokio::test]
async fn when_everything_is_gone_the_client_gets_the_last_status() {
    let routes =
        "[routes.allfast]\ntype = \"fastest\"\ntargets = [\"a-dead\", \"c-dead\"]\nexplore = 0.0\n";
    let rig = rig(404, not_found(), routes).await;
    let resp = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", rig.url))
        .json(&chat_request("allfast", false))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: Value = resp.json().await.unwrap();
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap()
            .contains("does not exist"),
        "the provider's message comes through: {body}"
    );
    assert!(
        rig.dead.calls() >= 1 && rig.second_dead.calls() >= 1,
        "both were tried"
    );
}

#[tokio::test]
async fn a_strict_route_never_crosses_classes_to_get_away_from_a_dead_target() {
    let routes = "[routes.private-pool]\ntype = \"fastest\"\npool = { strength = \"general\" }\nallowed_classes = [\"private\"]\nexplore = 0.0\n";
    let rig = rig(404, plain(), routes).await;
    // Only the private target is allowed; it is dead, and the public one must not be used.
    let (code, _) = rig.ask("private-pool").await;
    assert_eq!(code, 404);
    assert_eq!(rig.live.calls(), 0);
}

#[tokio::test]
async fn explain_says_whether_a_route_moves_on_for_unavailable_answers() {
    let rig = rig(404, plain(), &format!("{FASTEST}{PASS}{PASS_OPT_IN}")).await;
    let explain = |route: &'static str| {
        let url = rig.url.clone();
        async move {
            reqwest::Client::new()
                .post(format!("{url}/v1/route/explain"))
                .json(&json!({"model": route}))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };
    assert_eq!(explain("fast").await["fallback_includes_unavailable"], true);
    assert_eq!(
        explain("pass").await["fallback_includes_unavailable"],
        false
    );
    assert_eq!(
        explain("optin").await["fallback_includes_unavailable"],
        true
    );
}
