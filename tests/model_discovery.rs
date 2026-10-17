//! `humpyard models check` and `models sync` (ADR 0022), against a mock dataset and a mock provider: what is
//! proposed, what is kept and removed, the guardrails, idempotence, where the key goes, and that the file it
//! writes is one `serve` accepts.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test scaffolding

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use common::serve;
use humpyard::config::{Config, DiscoveryState};
use serde_json::{Value, json};

const KEY: &str = "provider-secret-key";

/// Both mock servers' shared, changeable state.
struct World {
    dataset: Mutex<Value>,
    /// `None` makes the provider's `/models` answer 500.
    provider_models: Mutex<Option<Vec<String>>>,
    dataset_saw_authorization: Mutex<bool>,
    provider_auth: Mutex<Vec<String>>,
}

#[allow(clippy::needless_pass_by_value)] // a test helper, called with json! literals
fn model(cost: Option<(f64, f64)>, context: u64, extra: Value) -> Value {
    let mut m = json!({
        "limit": {"context": context},
        "modalities": {"input": ["text"], "output": ["text"]},
        "tool_call": true,
        "reasoning": false,
    });
    if let Some((input, output)) = cost {
        m["cost"] = json!({"input": input, "output": output});
    }
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        m[k] = v;
    }
    m
}

fn dataset(models: &[(&str, Value)]) -> Value {
    let map: serde_json::Map<String, Value> = models
        .iter()
        .map(|(id, m)| ((*id).to_string(), m.clone()))
        .collect();
    json!({"freeprov": {"id": "freeprov", "models": map}})
}

fn standard_dataset() -> Value {
    dataset(&[
        ("alpha", model(Some((0.0, 0.0)), 32_000, json!({}))),
        (
            "beta",
            model(
                Some((0.0, 0.0)),
                128_000,
                json!({"reasoning": true, "modalities": {"input": ["text", "image"], "output": ["text"]}}),
            ),
        ),
        ("paidone", model(Some((1.0, 2.0)), 8_000, json!({}))),
        (
            "whisper",
            model(
                Some((0.0, 0.0)),
                1,
                json!({"modalities": {"input": ["audio"], "output": ["text"]}}),
            ),
        ),
        ("noctx", model(Some((0.0, 0.0)), 0, json!({}))),
        ("nocost", model(None, 4_000, json!({}))),
        ("ghost", model(Some((0.0, 0.0)), 16_000, json!({}))),
    ])
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

async fn dataset_handler(State(w): State<Arc<World>>, headers: HeaderMap) -> impl IntoResponse {
    if headers.contains_key("authorization") {
        *w.dataset_saw_authorization.lock().unwrap() = true;
    }
    axum::Json(w.dataset.lock().unwrap().clone())
}

async fn models_handler(
    State(w): State<Arc<World>>,
    headers: HeaderMap,
) -> axum::response::Response {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    w.provider_auth.lock().unwrap().push(auth);
    match w.provider_models.lock().unwrap().clone() {
        Some(list) => {
            axum::Json(json!({"data": list.iter().map(|id| json!({"id": id})).collect::<Vec<_>>()}))
                .into_response()
        }
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

struct Rig {
    world: Arc<World>,
    dataset_url: String,
    config: PathBuf,
    file: PathBuf,
    _dir: tempfile::TempDir,
}

async fn rig(extra_config: &str, hand_written: &str) -> Rig {
    let world = Arc::new(World {
        dataset: Mutex::new(standard_dataset()),
        provider_models: Mutex::new(Some(ids(&["alpha", "beta", "paidone", "noctx", "whisper"]))),
        dataset_saw_authorization: Mutex::new(false),
        provider_auth: Mutex::new(Vec::new()),
    });
    let dataset_url = format!(
        "http://{}/api.json",
        serve(
            Router::new()
                .route("/api.json", get(dataset_handler))
                .with_state(world.clone())
        )
        .await
    );
    let provider_url = format!(
        "http://{}",
        serve(
            Router::new()
                .route("/models", get(models_handler))
                .with_state(world.clone())
        )
        .await
    );
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("discovered.toml");
    let config = dir.path().join("humpyard.toml");
    std::fs::write(
        &config,
        format!(
            r#"listen = "127.0.0.1:0"
[discovery]
enabled = true
file = "{file}"
{extra_config}
[providers.free]
base_url = "{provider_url}"
api_key_env = "DISCOVERY_TEST_KEY"
price_source = "models-dev:freeprov"
discover = {{ strengths = ["general"] }}
[providers.other]
base_url = "http://127.0.0.1:1"
api_key_env = "DISCOVERY_TEST_KEY"
[targets.mine]
strengths = ["general"]
endpoints = [{{ provider = "other", model = "mine-model" }}]
{hand_written}
[routes.pool]
type = "fastest"
pool = {{ strength = "general" }}
"#,
            file = file.display()
        ),
    )
    .unwrap();
    Rig {
        world,
        dataset_url,
        config,
        file,
        _dir: dir,
    }
}

struct Out {
    code: i32,
    stdout: String,
    stderr: String,
}

async fn models(rig: &Rig, args: &[&str]) -> Out {
    let mut full: Vec<String> = vec!["models".into()];
    full.extend(args.iter().map(|a| (*a).to_string()));
    full.extend([
        "--config".into(),
        rig.config.to_string_lossy().into_owned(),
        "--source-url".into(),
        rig.dataset_url.clone(),
    ]);
    let out = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_humpyard"))
            .args(&full)
            .env("DISCOVERY_TEST_KEY", KEY)
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    Out {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn load(rig: &Rig) -> Config {
    let text = std::fs::read_to_string(&rig.config).unwrap();
    Config::from_toml_with(
        &text,
        |n| std::env::var(n).ok().or(Some(KEY.into())),
        &|p: &Path| std::fs::read_to_string(p).ok(),
    )
    .unwrap()
}

fn generated_names(rig: &Rig) -> Vec<String> {
    load(rig).generated_targets.into_iter().collect()
}

#[tokio::test]
async fn check_proposes_only_free_chat_models_both_sources_agree_on() {
    let rig = rig("", "").await;
    let out = models(&rig, &["check"]).await;
    assert_eq!(
        out.code, 1,
        "something to review: {}{}",
        out.stdout, out.stderr
    );
    // alpha and beta: free, chat, with a window, listed by the provider.
    assert!(out.stdout.contains("auto-free-alpha"), "{}", out.stdout);
    assert!(out.stdout.contains("auto-free-beta"), "{}", out.stdout);
    for absent in ["paidone", "whisper", "noctx", "nocost"] {
        assert!(
            !out.stdout.contains(&format!("auto-free-{absent}")),
            "{absent}\n{}",
            out.stdout
        );
    }
    // ghost is free in the dataset but the provider does not list it: reported, not proposed.
    assert!(
        out.stdout.contains("unconfirmed") && out.stdout.contains("ghost"),
        "{}",
        out.stdout
    );
    assert!(!out.stdout.contains("auto-free-ghost"));
    assert!(!rig.file.exists(), "check never writes");
}

#[tokio::test]
async fn check_json_and_the_paid_option() {
    let rig = rig("", "").await;
    let json_out = models(&rig, &["check", "--json"]).await;
    let report: Value = serde_json::from_str(&json_out.stdout).unwrap();
    assert_eq!(report["drift"], true);
    let new: Vec<&str> = report["new"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["model"].as_str().unwrap())
        .collect();
    assert_eq!(new, ["alpha", "beta"]);
    assert_eq!(report["new"][1]["reasoning"], true);
    assert_eq!(report["new"][1]["vision"], true);
    assert_eq!(
        report["new_paid"],
        json!([]),
        "paid models are only reported on request"
    );

    // With --paid the paid chat model is listed with its price, and is still never proposed for sync.
    let paid = models(&rig, &["check", "--paid"]).await;
    assert!(
        paid.stdout.contains("new (paid)") && paid.stdout.contains("paidone"),
        "{}",
        paid.stdout
    );
    assert!(paid.stdout.contains("write a target by hand"));
    let synced = models(&rig, &["sync"]).await;
    assert_eq!(synced.code, 0, "{}{}", synced.stdout, synced.stderr);
    assert!(
        !std::fs::read_to_string(&rig.file)
            .unwrap()
            .contains("paidone")
    );
}

#[tokio::test]
async fn check_exits_zero_when_there_is_nothing_to_review() {
    let rig = rig("", "").await;
    assert_eq!(models(&rig, &["sync"]).await.code, 0);
    let out = models(&rig, &["check"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("nothing to review"));
}

#[tokio::test]
async fn sync_writes_a_file_serve_accepts_and_pools_pick_up() {
    let rig = rig("", "").await;
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(generated_names(&rig), ["auto-free-alpha", "auto-free-beta"]);
    let config = load(&rig);
    assert_eq!(config.discovery_state, DiscoveryState::Loaded(2));
    assert_eq!(
        config.pools["pool"].members,
        ["auto-free-alpha", "auto-free-beta", "mine"],
        "generated targets joined the general pool"
    );
    let text = std::fs::read_to_string(&rig.file).unwrap();
    assert!(text.contains(r#"strengths = ["general", "all"]"#), "{text}");
    assert!(
        text.contains(r#"strengths = ["general", "reasoning", "all"]"#),
        "reasoning is added: {text}"
    );
    assert!(
        text.contains("context_window = 128000") && text.contains("vision = true"),
        "{text}"
    );
    assert!(text.contains("input = 0.0"), "free: {text}");
}

#[tokio::test]
async fn a_second_run_changes_nothing_and_a_change_keeps_the_previous_version() {
    let rig = rig("", "").await;
    models(&rig, &["sync"]).await;
    let first = std::fs::read_to_string(&rig.file).unwrap();
    let again = models(&rig, &["sync"]).await;
    assert!(again.stdout.contains("unchanged"), "{}", again.stdout);
    assert_eq!(std::fs::read_to_string(&rig.file).unwrap(), first);
    assert!(!rig.file.with_extension("toml.prev").exists());

    // beta turns paid: it is removed, and the previous file is kept next to the new one.
    rig.world.dataset.lock().unwrap()["freeprov"]["models"]["beta"]["cost"] =
        json!({"input": 0.5, "output": 1.0});
    let changed = models(&rig, &["sync"]).await;
    assert_eq!(changed.code, 0, "{}{}", changed.stdout, changed.stderr);
    assert_eq!(generated_names(&rig), ["auto-free-alpha"]);
    let prev = PathBuf::from(format!("{}.prev", rig.file.display()));
    assert_eq!(std::fs::read_to_string(prev).unwrap(), first);
}

#[tokio::test]
async fn a_lagging_dataset_never_removes_a_model_the_provider_still_lists() {
    let rig = rig("", "").await;
    models(&rig, &["sync"]).await;
    let first = std::fs::read_to_string(&rig.file).unwrap();
    // The dataset forgets alpha for a run: the provider still lists it, so it stays and nothing is rewritten.
    rig.world.dataset.lock().unwrap()["freeprov"]["models"]
        .as_object_mut()
        .unwrap()
        .remove("alpha");
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("unchanged"), "{}", out.stdout);
    assert_eq!(std::fs::read_to_string(&rig.file).unwrap(), first);
    assert_eq!(generated_names(&rig), ["auto-free-alpha", "auto-free-beta"]);
}

#[tokio::test]
async fn a_model_the_provider_drops_is_removed() {
    let rig = rig("", "").await;
    models(&rig, &["sync"]).await;
    *rig.world.provider_models.lock().unwrap() = Some(ids(&["beta", "paidone"]));
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(generated_names(&rig), ["auto-free-beta"]);
}

#[tokio::test]
async fn hand_written_targets_and_the_exclude_list_are_respected() {
    let hand =
        "[targets.alpha-by-hand]\nendpoints = [{ provider = \"free\", model = \"alpha\" }]\n";
    let rig = rig("exclude = [\"free/beta\"]", hand).await;
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert_eq!(
        generated_names(&rig),
        Vec::<String>::new(),
        "alpha is hand-written, beta is excluded"
    );
}

#[tokio::test]
async fn the_guardrails_refuse_a_run_and_write_nothing() {
    // Too many additions.
    let capped = rig("max_added_per_run = 1", "").await;
    let out = models(&capped, &["sync"]).await;
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert!(out.stderr.contains("max_added_per_run"), "{}", out.stderr);
    assert!(!capped.file.exists());

    // Too many removals: four generated, the provider suddenly lists only one.
    let rig = self::rig("", "").await;
    {
        let mut data = rig.world.dataset.lock().unwrap();
        for id in ["g1", "g2"] {
            data["freeprov"]["models"][id] = model(Some((0.0, 0.0)), 8_000, json!({}));
        }
    }
    *rig.world.provider_models.lock().unwrap() = Some(ids(&["alpha", "beta", "g1", "g2"]));
    assert_eq!(models(&rig, &["sync"]).await.code, 0);
    assert_eq!(generated_names(&rig).len(), 4);
    let before = std::fs::read_to_string(&rig.file).unwrap();
    *rig.world.provider_models.lock().unwrap() = Some(ids(&["alpha"]));
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 1, "{}{}", out.stdout, out.stderr);
    assert!(
        out.stderr.contains("max_removed_fraction"),
        "{}",
        out.stderr
    );
    assert_eq!(
        std::fs::read_to_string(&rig.file).unwrap(),
        before,
        "the file is untouched"
    );
}

#[tokio::test]
async fn a_provider_whose_list_cannot_be_read_is_left_alone() {
    let rig = rig("", "").await;
    models(&rig, &["sync"]).await;
    let before = std::fs::read_to_string(&rig.file).unwrap();
    *rig.world.provider_models.lock().unwrap() = None; // 500
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 0, "{}{}", out.stdout, out.stderr);
    assert!(out.stdout.contains("left alone"), "{}", out.stdout);
    assert_eq!(std::fs::read_to_string(&rig.file).unwrap(), before);
    // An empty list is treated the same, not as "everything is gone".
    *rig.world.provider_models.lock().unwrap() = Some(Vec::new());
    models(&rig, &["sync"]).await;
    assert_eq!(std::fs::read_to_string(&rig.file).unwrap(), before);
}

#[tokio::test]
async fn the_provider_key_goes_only_to_the_provider() {
    let rig = rig("", "").await;
    models(&rig, &["check"]).await;
    models(&rig, &["sync"]).await;
    assert!(
        !*rig.world.dataset_saw_authorization.lock().unwrap(),
        "no key reaches the dataset"
    );
    let seen = rig.world.provider_auth.lock().unwrap().clone();
    assert_ne!(seen, Vec::<String>::new());
    assert!(
        seen.iter().all(|a| a == &format!("Bearer {KEY}")),
        "{seen:?}"
    );
}

#[tokio::test]
async fn unusable_configuration_and_sources_exit_two() {
    let rig = rig("", "").await;
    // No provider opts in.
    let plain = rig.config.with_file_name("plain.toml");
    std::fs::write(
        &plain,
        "listen = \"127.0.0.1:0\"\n[providers.p]\nbase_url = \"http://127.0.0.1:1\"\napi_key_env = \"DISCOVERY_TEST_KEY\"\n[targets.t]\nendpoints = [{ provider = \"p\", model = \"m\" }]\n",
    )
    .unwrap();
    let out = tokio::task::spawn_blocking({
        let (plain, url) = (plain.clone(), rig.dataset_url.clone());
        move || {
            Command::new(env!("CARGO_BIN_EXE_humpyard"))
                .args([
                    "models",
                    "check",
                    "--config",
                    plain.to_str().unwrap(),
                    "--source-url",
                    &url,
                ])
                .env("DISCOVERY_TEST_KEY", KEY)
                .output()
                .unwrap()
        }
    })
    .await
    .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("discover"));

    // The dataset cannot be reached.
    let mut dead = Rig {
        world: rig.world.clone(),
        dataset_url: "http://127.0.0.1:1/api.json".into(),
        config: rig.config.clone(),
        file: rig.file.clone(),
        _dir: tempfile::tempdir().unwrap(),
    };
    let out = models(&dead, &["sync"]).await;
    assert_eq!(out.code, 2, "{}{}", out.stdout, out.stderr);
    dead.dataset_url = rig.dataset_url.clone();
}

#[tokio::test]
async fn every_repeats_and_survives_a_failed_run() {
    let rig = rig("", "").await;
    let mut full: Vec<String> = vec![
        "models".into(),
        "sync".into(),
        "--every".into(),
        "1s".into(),
    ];
    full.extend([
        "--config".into(),
        rig.config.to_string_lossy().into_owned(),
        "--source-url".into(),
        rig.dataset_url.clone(),
    ]);
    let world = rig.world.clone();
    let file = rig.file.clone();
    let out = tokio::task::spawn_blocking(move || {
        let mut child = Command::new(env!("CARGO_BIN_EXE_humpyard"))
            .args(&full)
            .env("DISCOVERY_TEST_KEY", KEY)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        // Wait for the first run to write the file, then take the provider down for the later runs.
        for _ in 0..200 {
            if file.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        assert!(file.exists(), "the first run never wrote the file");
        *world.provider_models.lock().unwrap() = None;
        std::thread::sleep(std::time::Duration::from_millis(2_600));
        child.kill().unwrap();
        child.wait_with_output().unwrap()
    })
    .await
    .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(stdout.contains("wrote 2 generated targets"), "{stdout}");
    assert!(
        stdout.matches("left alone").count() >= 1,
        "later runs still happen: {stdout}"
    );
}

#[test]
fn durations_for_every_parse() {
    use humpyard::discover::parse_every;
    use std::time::Duration;
    assert_eq!(parse_every("90s").unwrap(), Duration::from_secs(90));
    assert_eq!(parse_every("30m").unwrap(), Duration::from_mins(30));
    assert_eq!(parse_every("6h").unwrap(), Duration::from_hours(6));
    assert_eq!(parse_every("1d").unwrap(), Duration::from_hours(24));
    for bad in ["", "h", "6", "0s", "1w", "abc"] {
        assert!(parse_every(bad).is_err(), "{bad}");
    }
}

#[tokio::test]
async fn sync_refuses_while_the_master_switch_is_off_but_check_still_reports() {
    let rig = rig("", "").await;
    let text = std::fs::read_to_string(&rig.config)
        .unwrap()
        .replace("enabled = true\n", "");
    std::fs::write(&rig.config, text).unwrap();
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 2, "{}{}", out.stdout, out.stderr);
    assert!(out.stderr.contains("switched off"), "{}", out.stderr);
    assert!(!rig.file.exists());
    // `check` only reads, so it still tells the operator what turning it on would do.
    assert_eq!(models(&rig, &["check"]).await.code, 1);
}

#[tokio::test]
async fn a_provider_that_opts_out_is_left_out_and_its_targets_go_at_the_next_sync() {
    let rig = rig("", "").await;
    assert_eq!(models(&rig, &["sync"]).await.code, 0);
    assert_eq!(generated_names(&rig).len(), 2);
    let text = std::fs::read_to_string(&rig.config).unwrap().replace(
        "discover = { strengths = [\"general\"] }",
        "discover = { strengths = [\"general\"], enabled = false }",
    );
    std::fs::write(&rig.config, text).unwrap();
    // Serve stops loading them at once ...
    assert_eq!(generated_names(&rig), Vec::<String>::new());
    // ... and with no opted-in provider left, sync has nothing to do and says why.
    let out = models(&rig, &["sync"]).await;
    assert_eq!(out.code, 2, "{}{}", out.stdout, out.stderr);
    assert!(out.stderr.contains("enabled"), "{}", out.stderr);
}
