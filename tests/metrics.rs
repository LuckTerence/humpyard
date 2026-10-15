//! `/metrics`: off by default, its own credential, exact counts from real traffic, no client
//! strings in the output, key labels on request, and a listener of its own.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::time::Duration;

use axum::Router;
use axum::routing::post;
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;
use tokio::sync::mpsc;

async fn upstream() -> String {
    let app = Router::new().route(
        "/chat/completions",
        post(|| async { axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))) }),
    );
    format!("http://{}", serve(app).await)
}

struct Rig {
    url: String,
    alice: String,
    credential: String,
    _dir: tempfile::TempDir,
}

/// One priced target (input $1, output $2 per million), route `plain`, key `alice`, and the
/// `[metrics]` block you give. `credential_hash` is substituted for `CRED`.
async fn rig(metrics: &str) -> Rig {
    let up = upstream().await;
    let dir = tempfile::tempdir().unwrap();
    let (alice, alice_hash) = humpyard::auth::generate_key().unwrap();
    let (credential, credential_hash) = humpyard::auth::generate_key().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[ledger]
path = "{db}"
[keys.alice]
sha256 = "{alice_hash}"
[providers.p]
base_url = "{up}"
api_key_env = "K"
max_retries = 0
[targets.a]
endpoints = [{{ provider = "p", model = "m", price = {{ input = 1.0, output = 2.0 }} }}]
[routes.plain]
type = "passthrough"
targets = ["a"]
{metrics}
"#,
        db = dir.path().join("ledger.db").display(),
        metrics = metrics.replace("CRED", &credential_hash),
    );
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    Rig {
        url: format!(
            "http://{}",
            serve(server::router(config).await.unwrap()).await
        ),
        alice,
        credential,
        _dir: dir,
    }
}

impl Rig {
    async fn get(&self, path: &str, key: Option<&str>) -> reqwest::Response {
        let mut req = reqwest::Client::new().get(format!("{}{path}", self.url));
        if let Some(key) = key {
            req = req.bearer_auth(key);
        }
        req.send().await.unwrap()
    }

    async fn ask(&self, model: &str) -> u16 {
        reqwest::Client::new()
            .post(format!("{}/v1/chat/completions", self.url))
            .bearer_auth(&self.alice)
            .json(&chat_request(model, false))
            .send()
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    async fn scrape(&self) -> String {
        let resp = self.get("/metrics", Some(&self.credential)).await;
        assert_eq!(resp.status(), 200);
        resp.text().await.unwrap()
    }
}

const ON_MAIN: &str = "[metrics]\nenabled = true\nbearer_sha256 = \"CRED\"\n";

#[tokio::test]
async fn metrics_are_off_by_default() {
    let rig = rig("").await;
    assert_eq!(rig.get("/metrics", Some(&rig.alice)).await.status(), 404);
    assert_eq!(rig.get("/metrics", None).await.status(), 404);
}

#[tokio::test]
async fn only_the_metrics_credential_reads_metrics_and_it_opens_nothing_else() {
    let rig = rig(ON_MAIN).await;
    assert_eq!(rig.get("/metrics", None).await.status(), 401);
    assert_eq!(
        rig.get("/metrics", Some(&rig.alice)).await.status(),
        401,
        "a client key does not read metrics"
    );
    let ok = rig.get("/metrics", Some(&rig.credential)).await;
    assert_eq!(ok.status(), 200);
    assert!(
        ok.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain; version=0.0.4")
    );
    assert_eq!(
        rig.get("/v1/models", Some(&rig.credential)).await.status(),
        401
    );
    let chat = reqwest::Client::new()
        .post(format!("{}/v1/chat/completions", rig.url))
        .bearer_auth(&rig.credential)
        .json(&chat_request("plain", false))
        .send()
        .await
        .unwrap();
    assert_eq!(
        chat.status(),
        401,
        "the metrics credential cannot call inference"
    );
}

#[tokio::test]
async fn real_traffic_is_counted_exactly_and_client_strings_stay_out() {
    let rig = rig(ON_MAIN).await;
    assert_eq!(rig.ask("plain").await, 200);
    assert_eq!(rig.ask("plain").await, 200);
    assert_eq!(rig.ask("evil\"model{route=\"x\"}").await, 404);
    let text = rig.scrape().await;

    for expected in [
        r#"humpyard_requests_total{protocol="openai_chat",route="plain",target="a",provider="p",status_class="2xx"} 2"#,
        r#"humpyard_requests_total{protocol="openai_chat",route="none",target="none",provider="none",status_class="4xx"} 1"#,
        r#"humpyard_tokens_total{route="plain",target="a",direction="input"} 20"#,
        r#"humpyard_tokens_total{route="plain",target="a",direction="output"} 10"#,
        r#"humpyard_cost_micro_usd_total{route="plain",target="a"} 40"#,
        r#"humpyard_refusals_total{reason="unknown_model"} 1"#,
        r#"humpyard_endpoint_state{target="a",provider="p",model="m",state="healthy"} 1"#,
        r#"humpyard_endpoint_state{target="a",provider="p",model="m",state="cooling_down"} 0"#,
        r#"humpyard_request_duration_seconds_count{route="plain",target="a"} 2"#,
        "humpyard_ledger_queue_depth ",
        "humpyard_ledger_dropped_total 0",
        "humpyard_config_last_reload_ok 1",
        "humpyard_build_info{version=",
    ] {
        assert!(text.contains(expected), "missing `{expected}` in:\n{text}");
    }
    assert!(
        !text.contains("evil"),
        "a client string reached the output:\n{text}"
    );
    assert!(!text.contains("alice"), "keys are not labels by default");
}

#[tokio::test]
async fn key_labels_appear_only_when_asked_for() {
    let rig = rig(&format!("{ON_MAIN}key_labels = true\n")).await;
    rig.ask("plain").await;
    let text = rig.scrape().await;
    assert!(
        text.contains(r#"status_class="2xx",key="alice"} 1"#),
        "requests carry the key:\n{text}"
    );
    assert!(text.contains(r#"direction="input",key="alice"} 10"#));
    assert!(
        text.contains(r#"humpyard_cost_micro_usd_total{route="plain",target="a",key="alice"} 20"#)
    );
    assert!(
        !text.contains(r#"humpyard_request_duration_seconds_bucket{route="plain",target="a",key="#),
        "histograms never carry the key"
    );
}

fn config_error(extra: &str) -> String {
    let toml = format!(
        r#"listen = "127.0.0.1:8080"
[keys.alice]
sha256 = "sha256:0000000000000000000000000000000000000000000000000000000000000001"
[providers.p]
base_url = "http://localhost:1/v1"
api_key_env = "K"
[targets.a]
endpoints = [{{ provider = "p", model = "m" }}]
{extra}
"#
    );
    Config::from_toml(&toml, |_| Some("k".into()))
        .err()
        .map(|e| e.to_string())
        .unwrap_or_default()
}

#[test]
fn metrics_config_is_validated() {
    assert!(
        config_error("[metrics]\nenabled = true\n").contains("bearer_sha256"),
        "the main listener needs a credential"
    );
    assert!(
        config_error("[metrics]\nenabled = true\nlisten = \"127.0.0.1:8080\"\n")
            .contains("must differ from listen")
    );
    assert!(
        config_error(
            "[metrics]\nenabled = true\nbearer_sha256 = \"sha256:0000000000000000000000000000000000000000000000000000000000000001\"\n"
        )
        .contains("must not be the key `alice`")
    );
    assert!(
        config_error("[metrics]\nenabled = true\nbearer_sha256 = \"nope\"\n")
            .contains("metrics.bearer_sha256 must be `sha256:`")
    );
    assert!(config_error("[metrics]\nkey_label_limit = 0\n").contains("key_label_limit"));
    assert_eq!(
        config_error("[metrics]\nenabled = true\nlisten = \"127.0.0.1:9100\"\n"),
        "",
        "a separate listener needs no credential"
    );
    assert_eq!(config_error(""), "", "the block is optional");
}

#[tokio::test]
async fn a_separate_listener_keeps_metrics_off_the_public_port() {
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let metrics_addr = probe.local_addr().unwrap();
    drop(probe);
    let up = upstream().await;
    let dir = tempfile::tempdir().unwrap();
    let toml = format!(
        r#"listen = "127.0.0.1:0"
[metrics]
enabled = true
listen = "{metrics_addr}"
[providers.p]
base_url = "{up}"
api_key_env = "K"
[targets.a]
endpoints = [{{ provider = "p", model = "m" }}]
[ledger]
path = "{db}"
"#,
        db = dir.path().join("l.db").display()
    );
    let config = Config::from_toml(&toml, |_| Some("k".into())).unwrap();
    let main = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let main_addr = main.local_addr().unwrap();
    let (_signals, rx) = mpsc::channel(1);
    let task = tokio::spawn(server::run(config, main, rx, None));

    let mut served = None;
    for _ in 0..100 {
        if let Ok(resp) = reqwest::get(format!("http://{metrics_addr}/metrics")).await {
            served = Some(resp);
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let served = served.expect("the metrics listener came up");
    assert_eq!(
        served.status(),
        200,
        "no credential is needed on the separate listener"
    );
    assert!(served.text().await.unwrap().contains("humpyard_build_info"));
    let public = reqwest::get(format!("http://{main_addr}/metrics"))
        .await
        .unwrap();
    assert_eq!(public.status(), 404, "/metrics is not on the public port");
    task.abort();
}

#[tokio::test]
async fn scraping_while_serving_neither_fails_nor_loses_counts() {
    let rig = std::sync::Arc::new(rig(ON_MAIN).await);
    let scraper = {
        let rig = rig.clone();
        tokio::spawn(async move {
            for _ in 0..40 {
                assert_eq!(
                    rig.get("/metrics", Some(&rig.credential)).await.status(),
                    200
                );
            }
        })
    };
    let mut requests = Vec::new();
    for _ in 0..40 {
        let rig = rig.clone();
        requests.push(tokio::spawn(async move { rig.ask("plain").await }));
    }
    for request in requests {
        assert_eq!(
            request.await.unwrap(),
            200,
            "no request fails during a scrape"
        );
    }
    scraper.await.unwrap();
    let text = rig.scrape().await;
    assert!(
        text.contains(r#"status_class="2xx"} 40"#),
        "every request was counted once:\n{text}"
    );
    assert!(text.contains(r#"humpyard_cost_micro_usd_total{route="plain",target="a"} 800"#));
}
