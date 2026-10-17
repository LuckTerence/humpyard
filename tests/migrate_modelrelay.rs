//! `humpyard migrate-modelrelay`: from saved responses and from a live (mock) `ModelRelay`. It prints a
//! fragment that loads, never asks for a key value, never overwrites a file, and the migrated pool routes
//! serve requests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // integration tests fail loudly on purpose

mod common;

use std::process::Command;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use common::{chat_completion, chat_request, serve, usage};
use humpyard::config::Config;
use humpyard::server;

const CONFIG_JSON: &str = include_str!("fixtures/modelrelay/config.json");
const MODELS_JSON: &str = include_str!("fixtures/modelrelay/models.json");

fn fixture(name: &str, body: &str) -> String {
    let path = std::env::temp_dir().join(format!("mig-{}-{name}", std::process::id()));
    std::fs::write(&path, body).unwrap();
    path.to_string_lossy().into_owned()
}

fn run(args: &[&str], env: &[(&str, &str)]) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_humpyard"));
    cmd.arg("migrate-modelrelay").args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.output().unwrap()
}

fn offline_args() -> Vec<String> {
    vec![
        "--config".into(),
        fixture("config.json", CONFIG_JSON),
        "--models".into(),
        fixture("models.json", MODELS_JSON),
    ]
}

fn offline(extra: &[&str]) -> std::process::Output {
    let base = offline_args();
    let mut args: Vec<&str> = base.iter().map(String::as_str).collect();
    args.extend_from_slice(extra);
    run(&args, &[])
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn saved_responses_become_a_fragment_that_loads() {
    let out = offline(&["--min-ctx", "32000"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    let fragment = text(&out.stdout);
    let header = "listen = \"127.0.0.1:0\"\n";
    Config::from_toml(&format!("{header}{fragment}"), |_| Some("k".into())).unwrap();

    for expected in [
        "[providers.nvidia]",
        "[providers.groq]",
        "[providers.cerebras]",
        "[providers.ollama]",
        "base_url = \"http://ollama.example.net:11434/v1\"",
        "[targets.groq-llama-3-3-70b-versatile]",
        "[targets.nvidia-deepseek-ai-deepseek-v3-2]",
        "[routes.\"auto-fastest\"]",
        "[routes.\"tag:coding+min_ctx:32000\"]",
    ] {
        assert!(fragment.contains(expected), "{expected}\n{fragment}");
    }
    for absent in [
        "openrouter",
        "codestral",
        "kiro",
        "qwen-3-235b",
        "gemma-3-4b",
        "nvap",
        "gsk_",
        "csk-",
    ] {
        assert!(
            !fragment.contains(absent),
            "{absent} must not appear:\n{fragment}"
        );
    }

    let report = text(&out.stderr);
    assert!(report.contains("the fragment loads"), "{report}");
    assert!(
        report.contains("provider `openrouter`: ModelRelay has no key"),
        "{report}"
    );
    assert!(
        report.contains("provider `codestral`: disabled"),
        "{report}"
    );
    assert!(report.contains("provider `kiro`"), "{report}");
    assert!(
        report.contains("model `cerebras/qwen-3-235b-a22b-instruct`: banned"),
        "{report}"
    );
    assert!(
        report.contains("cerebras") && report.contains("2 keys"),
        "{report}"
    );
    assert!(report.contains("API_KEY"), "{report}");
}

#[test]
fn the_class_option_is_applied() {
    let out = offline(&["--class", "free"]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(text(&out.stdout).contains("class = \"free\""));
}

#[test]
fn an_existing_output_file_is_never_overwritten() {
    let path = std::env::temp_dir().join(format!("mig-out-{}.toml", std::process::id()));
    std::fs::write(&path, "keep me").unwrap();
    let out = offline(&["--out", path.to_str().unwrap()]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("never overwritten"),
        "{}",
        text(&out.stderr)
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");

    let fresh = std::env::temp_dir().join(format!("mig-fresh-{}.toml", std::process::id()));
    let _ = std::fs::remove_file(&fresh);
    let out = offline(&["--out", fresh.to_str().unwrap()]);
    assert!(out.status.success(), "{}", text(&out.stderr));
    assert!(
        std::fs::read_to_string(&fresh)
            .unwrap()
            .contains("[providers.groq]")
    );
    assert!(
        text(&out.stdout).is_empty(),
        "with --out nothing is printed to stdout"
    );
}

#[test]
fn wrong_arguments_and_bad_input_are_refused() {
    assert!(!run(&[], &[]).status.success(), "needs a source");
    let only_config = run(&["--config", "x.json"], &[]);
    assert!(!only_config.status.success());
    let bad = fixture("bad.json", "{}");
    let out = run(&["--config", &bad, "--models", &bad], &[]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("/api/config"),
        "{}",
        text(&out.stderr)
    );
    // Nothing usable: the self-check refuses to call the fragment ready.
    let empty = fixture("empty.json", "[]");
    let out = run(&["--config", &empty, "--models", &empty], &[]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("does not load"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn the_url_form_needs_https_or_this_machine() {
    let out = run(
        &[
            "--url",
            "http://modelrelay.internal:7352",
            "--token-env",
            "MIG_TOKEN",
        ],
        &[("MIG_TOKEN", "t")],
    );
    assert!(!out.status.success());
    assert!(text(&out.stderr).contains("https"), "{}", text(&out.stderr));
}

/// A mock `ModelRelay` admin API. It records every path asked for and requires a bearer token.
struct Mock {
    paths: Mutex<Vec<String>>,
    token: String,
}

async fn admin(
    State(mock): State<Arc<Mock>>,
    uri: axum::http::Uri,
    headers: HeaderMap,
) -> axum::response::Response {
    mock.paths.lock().unwrap().push(uri.path().to_string());
    let presented = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if presented != format!("Bearer {}", mock.token) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match uri.path() {
        "/api/config" => CONFIG_JSON.to_string().into_response(),
        "/api/models" => MODELS_JSON.to_string().into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn mock_modelrelay(token: &str) -> (Arc<Mock>, String) {
    let mock = Arc::new(Mock {
        paths: Mutex::new(Vec::new()),
        token: token.to_string(),
    });
    let app = Router::new().fallback(get(admin)).with_state(mock.clone());
    (mock, format!("http://{}", serve(app).await))
}

#[tokio::test]
async fn the_live_form_reads_only_config_and_models_and_never_a_key() {
    let (mock, url) = mock_modelrelay("s3cret").await;
    let out = tokio::task::spawn_blocking(move || {
        run(
            &["--url", &url, "--token-env", "MIG_TOKEN"],
            &[("MIG_TOKEN", "s3cret")],
        )
    })
    .await
    .unwrap();
    assert!(out.status.success(), "{}", text(&out.stderr));
    let paths = mock.paths.lock().unwrap().clone();
    assert_eq!(
        paths,
        ["/api/config", "/api/models"],
        "no other request, in particular no key endpoint"
    );
    assert!(!text(&out.stdout).contains("s3cret"));
    assert!(!text(&out.stderr).contains("s3cret"));
    assert!(text(&out.stdout).contains("[providers.groq]"));
}

#[tokio::test]
async fn a_missing_or_wrong_token_is_a_clear_error() {
    let (_mock, url) = mock_modelrelay("right").await;
    let u1 = url.clone();
    let wrong = tokio::task::spawn_blocking(move || {
        run(
            &["--url", &u1, "--token-env", "MIG_TOKEN"],
            &[("MIG_TOKEN", "wrong")],
        )
    })
    .await
    .unwrap();
    assert!(!wrong.status.success());
    assert!(
        text(&wrong.stderr).contains("401") || text(&wrong.stderr).contains("token"),
        "{}",
        text(&wrong.stderr)
    );
    assert!(
        !text(&wrong.stderr).contains("wrong\n"),
        "the token is never echoed"
    );

    let missing = tokio::task::spawn_blocking(move || {
        run(&["--url", &url, "--token-env", "MIG_TOKEN_UNSET"], &[])
    })
    .await
    .unwrap();
    assert!(!missing.status.success());
    assert!(
        text(&missing.stderr).contains("MIG_TOKEN_UNSET"),
        "{}",
        text(&missing.stderr)
    );
}

#[tokio::test]
async fn a_migrated_pool_route_serves_requests() {
    // Run the real command, point its providers at a mock upstream, and send a request to a drop-in
    // route name exactly as an existing ModelRelay client would.
    let upstream = serve(Router::new().route(
        "/chat/completions",
        post(|| async { axum::Json(chat_completion("m", "ok", Some(usage(10, 5)))) }),
    ))
    .await;
    let out = offline(&["--min-ctx", "32000"]);
    assert!(out.status.success());
    let mut fragment = text(&out.stdout);
    for provider in [
        "https://integrate.api.nvidia.com/v1",
        "https://api.groq.com/openai/v1",
        "https://api.cerebras.ai/v1",
        "http://ollama.example.net:11434/v1",
    ] {
        fragment = fragment.replace(provider, &format!("http://{upstream}"));
    }
    let config = Config::from_toml(&format!("listen = \"127.0.0.1:0\"\n{fragment}"), |_| {
        Some("k".into())
    })
    .unwrap();
    let base = format!(
        "http://{}",
        serve(server::router(config).await.unwrap()).await
    );
    for route in ["auto-fastest", "tag:general+min_ctx:32000", "tag:coding"] {
        let resp = reqwest::Client::new()
            .post(format!("{base}/v1/chat/completions"))
            .json(&chat_request(route, false))
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200, "{route}");
    }
}
