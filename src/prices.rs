//! Price sources: parse a public price list, compare it with a config, and propose entries.
//!
//! Pure functions plus one network fetch, used only by the `humpyard prices` commands. The serving
//! process never calls any of this (ADR 0014): prices reach it only through the config file, after
//! an operator has reviewed them. Source data is untrusted, so every number is validated and a
//! record that fails is reported, never defaulted.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Duration;

use serde_json::{Value, json};

use crate::config::{Config, Price, PriceTier};

/// Where models.dev publishes its dataset.
pub const MODELS_DEV_URL: &str = "https://models.dev/api.json";

/// No real price is anywhere near this many USD per million tokens; larger values are data errors.
const MAX_PRICE: f64 = 10_000.0;
/// The dataset is a few megabytes; refuse anything wildly larger.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
const FETCH_TIMEOUT: Duration = Duration::from_secs(30);
/// Models.dev's older field for the tier above 200K prompt tokens, used when `tiers` is absent.
const LEGACY_TIER_ABOVE_TOKENS: u64 = 200_000;

#[derive(Debug, thiserror::Error)]
pub enum PriceError {
    #[error("{0}")]
    Source(String),
    #[error("{0}")]
    Config(String),
}

/// One model's price as a source states it.
#[derive(Clone, Debug, PartialEq)]
pub struct PriceRecord {
    pub input: f64,
    pub output: f64,
    pub cached_input: Option<f64>,
    pub tiers: Vec<PriceTier>,
    /// Things the source says that this record cannot express (for example peak-hour pricing).
    pub notes: Vec<String>,
}

/// What a source says about one provider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceData {
    /// A label for reports, for example `models-dev:opencode-go`.
    pub source: String,
    /// Models with a usable price.
    pub records: BTreeMap<String, PriceRecord>,
    /// Models the source lists but whose price could not be used, with the reason.
    pub unusable: BTreeMap<String, String>,
}

fn valid(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|n| n.is_finite() && (0.0..=MAX_PRICE).contains(n))
}

fn optional_price(object: &Value, key: &str) -> Result<Option<f64>, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => valid(value)
            .map(Some)
            .ok_or_else(|| format!("`{key}` is not a number between 0 and {MAX_PRICE}")),
    }
}

fn required_price(object: &Value, key: &str) -> Result<f64, String> {
    optional_price(object, key)?.ok_or_else(|| format!("`{key}` is missing"))
}

fn parse_cost(cost: &Value) -> Result<PriceRecord, String> {
    if !cost.is_object() {
        return Err("`cost` is not an object".into());
    }
    let mut record = PriceRecord {
        input: required_price(cost, "input")?,
        output: required_price(cost, "output")?,
        cached_input: optional_price(cost, "cache_read")?,
        tiers: Vec::new(),
        notes: Vec::new(),
    };
    if let Some(tiers) = cost.get("tiers").and_then(Value::as_array) {
        for tier in tiers {
            let spec = tier.get("tier");
            let kind = spec.and_then(|t| t.get("type")).and_then(Value::as_str);
            let size = spec.and_then(|t| t.get("size")).and_then(Value::as_u64);
            match (kind, size) {
                (Some("context"), Some(above_tokens)) => record.tiers.push(PriceTier {
                    above_tokens,
                    input: required_price(tier, "input")?,
                    output: required_price(tier, "output")?,
                    cached_input: optional_price(tier, "cache_read")?,
                }),
                (Some(other), _) => record.notes.push(format!(
                    "a `{other}` tier is not modelled; the base price is used"
                )),
                (None, _) => return Err("a tier has no usable `tier.type`".into()),
            }
        }
    } else if let Some(over) = cost.get("context_over_200k").filter(|v| v.is_object()) {
        record.tiers.push(PriceTier {
            above_tokens: LEGACY_TIER_ABOVE_TOKENS,
            input: required_price(over, "input")?,
            output: required_price(over, "output")?,
            cached_input: optional_price(over, "cache_read")?,
        });
    }
    record.tiers.sort_by_key(|t| t.above_tokens);
    Ok(record)
}

/// Reads one provider's prices out of a models.dev document.
pub fn parse_models_dev(body: &str, provider_id: &str) -> Result<SourceData, PriceError> {
    let root: Value = serde_json::from_str(body)
        .map_err(|e| PriceError::Source(format!("the price source is not valid JSON: {e}")))?;
    let models = root
        .get(provider_id)
        .ok_or_else(|| {
            PriceError::Source(format!(
                "the source has no provider `{provider_id}` (models.dev provider ids look like `opencode-go`)"
            ))
        })?
        .get("models")
        .and_then(Value::as_object)
        .ok_or_else(|| PriceError::Source(format!("provider `{provider_id}` lists no models")))?;
    let mut data = SourceData {
        source: format!("models-dev:{provider_id}"),
        ..SourceData::default()
    };
    for (id, model) in models {
        match model.get("cost") {
            None | Some(Value::Null) => {
                data.unusable
                    .insert(id.clone(), "the source gives no price".into());
            }
            Some(cost) => match parse_cost(cost) {
                Ok(record) => {
                    data.records.insert(id.clone(), record);
                }
                Err(reason) => {
                    data.unusable.insert(id.clone(), reason);
                }
            },
        }
    }
    Ok(data)
}

/// The models.dev provider id in a source such as `models-dev:opencode-go`.
pub fn parse_source(spec: &str) -> Result<&str, PriceError> {
    match crate::config::parse_price_source(spec) {
        Some(("models-dev", id)) => Ok(id),
        _ => Err(PriceError::Config(format!(
            "unknown price source `{spec}`; the available source is `models-dev:<provider id>`"
        ))),
    }
}

/// One provider's comparison with its source.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub provider: String,
    pub source: String,
    pub rows: Vec<Row>,
}

/// Accepts https, and http only to a loopback host (so tests can serve a fixture).
fn check_url(url: &str) -> Result<reqwest::Url, PriceError> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| PriceError::Config(format!("`{url}` is not a URL: {e}")))?;
    let loopback = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if parsed.scheme() == "https" || (parsed.scheme() == "http" && loopback) {
        Ok(parsed)
    } else {
        Err(PriceError::Config(format!(
            "`{url}`: price sources must be fetched over https"
        )))
    }
}

/// Downloads a price source. No credentials are ever sent; redirects are not followed; the body is
/// size-limited.
pub async fn fetch(url: &str) -> Result<String, PriceError> {
    let url = check_url(url)?;
    let client = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| PriceError::Source(e.to_string()))?;
    let mut response = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| PriceError::Source(format!("cannot reach {url}: {e}")))?;
    if !response.status().is_success() {
        return Err(PriceError::Source(format!(
            "{url} answered {}",
            response.status()
        )));
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| PriceError::Source(format!("reading {url}: {e}")))?
    {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(PriceError::Source(format!(
                "{url} is larger than {} MB; refusing it",
                MAX_BODY_BYTES / (1024 * 1024)
            )));
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body).map_err(|_| PriceError::Source(format!("{url} is not UTF-8 text")))
}

/// How one configured endpoint compares with the source.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// The configured price matches the source.
    Ok,
    /// Both have a price and they differ.
    Differs,
    /// The source has a price and the config has none.
    Unpriced,
    /// The source does not list the model at all.
    MissingAtSource,
    /// The source lists the model but its price could not be used.
    Unusable(String),
}

impl Status {
    pub fn label(&self) -> &'static str {
        match self {
            Status::Ok => "ok",
            Status::Differs => "differs",
            Status::Unpriced => "unpriced",
            Status::MissingAtSource => "missing-at-source",
            Status::Unusable(_) => "unusable",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub target: String,
    pub provider: String,
    pub model: String,
    pub configured: Option<Price>,
    pub source: Option<PriceRecord>,
    pub status: Status,
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * 1.0_f64.max(a.abs()).max(b.abs())
}

/// Whether a cached price agrees: a source that states none leaves nothing to disagree about.
fn cached_agrees(configured: Option<f64>, input: f64, source: Option<f64>) -> bool {
    source.is_none_or(|c| approx(configured.unwrap_or(input), c))
}

fn tiers_agree(configured: &[PriceTier], source: &[PriceTier]) -> bool {
    configured.len() == source.len()
        && configured.iter().zip(source).all(|(c, s)| {
            c.above_tokens == s.above_tokens
                && approx(c.input, s.input)
                && approx(c.output, s.output)
                && cached_agrees(c.cached_input, c.input, s.cached_input)
        })
}

fn same(configured: &Price, source: &PriceRecord) -> bool {
    approx(configured.input, source.input)
        && approx(configured.output, source.output)
        && cached_agrees(
            configured.cached_input,
            configured.input,
            source.cached_input,
        )
        && tiers_agree(&configured.tiers, &source.tiers)
}

/// Compares every endpoint of `provider` in `config` with the source. Models are matched by exact id.
pub fn compare(config: &Config, provider: &str, data: &SourceData) -> Vec<Row> {
    let mut rows = Vec::new();
    for (target, endpoints) in &config.targets {
        for endpoint in endpoints.iter().filter(|e| e.provider == provider) {
            let (source, status) = if let Some(record) = data.records.get(&endpoint.model) {
                let status = match &endpoint.price {
                    None => Status::Unpriced,
                    Some(price) if same(price, record) => Status::Ok,
                    Some(_) => Status::Differs,
                };
                (Some(record.clone()), status)
            } else if let Some(reason) = data.unusable.get(&endpoint.model) {
                (None, Status::Unusable(reason.clone()))
            } else {
                (None, Status::MissingAtSource)
            };
            rows.push(Row {
                target: target.clone(),
                provider: endpoint.provider.clone(),
                model: endpoint.model.clone(),
                configured: endpoint.price.clone(),
                source,
                status,
            });
        }
    }
    rows
}

/// Whether anything needs attention (drift).
pub fn has_drift(rows: &[Row]) -> bool {
    rows.iter().any(|r| r.status != Status::Ok)
}

fn price_text(input: f64, output: f64, cached: Option<f64>) -> String {
    match cached {
        Some(c) => format!("input = {input:?}, output = {output:?}, cached_input = {c:?}"),
        None => format!("input = {input:?}, output = {output:?}"),
    }
}

/// A price as it is written in the config, including any tiers.
fn full_text(input: f64, output: f64, cached: Option<f64>, tiers: &[PriceTier]) -> String {
    let mut text = price_text(input, output, cached);
    if !tiers.is_empty() {
        let list: Vec<String> = tiers
            .iter()
            .map(|t| {
                format!(
                    "{{ above_tokens = {}, {} }}",
                    t.above_tokens,
                    price_text(t.input, t.output, t.cached_input)
                )
            })
            .collect();
        add(&mut text, format_args!(", tiers = [{}]", list.join(", ")));
    }
    text
}

fn describe(price: Option<&Price>) -> String {
    price.map_or_else(
        || "no price".to_string(),
        |p| full_text(p.input, p.output, p.cached_input, &p.tiers),
    )
}

fn source_text(record: &PriceRecord) -> String {
    full_text(
        record.input,
        record.output,
        record.cached_input,
        &record.tiers,
    )
}

fn add(out: &mut String, text: std::fmt::Arguments<'_>) {
    // Writing to a String cannot fail.
    let _ = out.write_fmt(text);
}

/// A TOML-comment-annotated list of the `price` entries the source proposes, for review.
pub fn render_snippet(rows: &[Row], source: &str, read_on: &str) -> String {
    let mut out = format!(
        "# Proposed prices from {source}, read {read_on}. USD per million tokens.\n# Review, then copy into the matching `endpoints` entries; nothing was changed.\n"
    );
    for row in rows {
        add(
            &mut out,
            format_args!(
                "\n# [targets.{}] {}/{}: {}\n",
                row.target,
                row.provider,
                row.model,
                row.status.label()
            ),
        );
        match (&row.source, &row.status) {
            (Some(record), _) => {
                add(
                    &mut out,
                    format_args!("price = {{ {} }}\n", source_text(record)),
                );
                if row.status == Status::Differs {
                    add(
                        &mut out,
                        format_args!("#   currently: {}\n", describe(row.configured.as_ref())),
                    );
                }
                for line in &record.notes {
                    add(&mut out, format_args!("#   note: {line}\n"));
                }
            }
            (None, Status::Unusable(reason)) => {
                add(
                    &mut out,
                    format_args!("# NOT PROPOSED: the source's price is unusable ({reason})\n"),
                );
            }
            (None, _) => out.push_str("# NOT PROPOSED: the source does not list this model\n"),
        }
    }
    out
}

/// A human-readable drift report.
pub fn render_report(rows: &[Row], source: &str, read_on: &str) -> String {
    let mut out = format!("Prices checked against {source}, read {read_on}.\n\n");
    for row in rows {
        add(
            &mut out,
            format_args!(
                "{:<18} [targets.{}] {}/{}",
                row.status.label(),
                row.target,
                row.provider,
                row.model
            ),
        );
        match (&row.status, &row.source) {
            (Status::Differs, Some(record)) => add(
                &mut out,
                format_args!(
                    "\n    configured: {}\n    source:     {}",
                    describe(row.configured.as_ref()),
                    source_text(record)
                ),
            ),
            (Status::Unpriced, Some(record)) => add(
                &mut out,
                format_args!("\n    source: {}", source_text(record)),
            ),
            (Status::Unusable(reason), _) => add(&mut out, format_args!("\n    {reason}")),
            _ => {}
        }
        out.push('\n');
    }
    let attention = rows.iter().filter(|r| r.status != Status::Ok).count();
    add(
        &mut out,
        format_args!(
            "\n{} endpoint(s) checked, {attention} need attention.\n",
            rows.len()
        ),
    );
    out
}

/// One report as JSON.
pub fn report_json(rows: &[Row], source: &str, read_on: &str) -> Value {
    let price = |p: Option<&Price>| {
        p.map(|p| {
            json!({
                "input": p.input,
                "output": p.output,
                "cached_input": p.cached_input,
                "tiers": p.tiers.iter().map(|t| json!({
                    "above_tokens": t.above_tokens,
                    "input": t.input,
                    "output": t.output,
                    "cached_input": t.cached_input,
                })).collect::<Vec<_>>(),
            })
        })
    };
    json!({
        "source": source,
        "read_on": read_on,
        "drift": has_drift(rows),
        "endpoints": rows.iter().map(|r| json!({
            "target": r.target,
            "provider": r.provider,
            "model": r.model,
            "status": r.status.label(),
            "reason": match &r.status { Status::Unusable(reason) => Some(reason.as_str()), _ => None },
            "configured": price(r.configured.as_ref()),
            "source": r.source.as_ref().map(|s| json!({
                "input": s.input,
                "output": s.output,
                "cached_input": s.cached_input,
                "tiers": s.tiers.iter().map(|t| json!({
                    "above_tokens": t.above_tokens,
                    "input": t.input,
                    "output": t.output,
                    "cached_input": t.cached_input,
                })).collect::<Vec<_>>(),
                "notes": s.notes,
            })),
        })).collect::<Vec<_>>(),
    })
}

/// Several reports as one JSON document, for scheduled jobs: `drift` is true when any report has
/// drift; each report names its provider and source.
pub fn render_json(reports: &[Report], read_on: &str) -> Value {
    json!({
        "read_on": read_on,
        "drift": reports.iter().any(|r| has_drift(&r.rows)),
        "reports": reports.iter().map(|r| {
            let mut report = report_json(&r.rows, &r.source, read_on);
            report["provider"] = json!(r.provider);
            report
        }).collect::<Vec<_>>(),
    })
}
