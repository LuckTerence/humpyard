//! The generated targets file (ADR 0022): what `humpyard models sync` writes and `serve` reads.
//!
//! `serve` never fetches anything; it only reads this file, at load and on every reload. The file is
//! untrusted input produced by another process, so it is parsed strictly and accepted or ignored as a
//! whole: it may hold nothing but `[targets.auto-*]` on providers that opted in to discovery, with every
//! endpoint priced at zero, no endpoint a hand-written target already uses, and the class always the
//! configured one (a generated target can never claim a class such as `private`). Anything else, or a
//! missing or empty file, leaves the gateway serving with its main configuration alone.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::Deserialize;

use super::pools::validate_strengths;
use super::schema::{Endpoint, RawConfig, RawTarget};

/// Every generated target is named with this prefix, so it is recognizable and cannot collide with a
/// hand-written one by accident.
pub const PREFIX: &str = "auto-";
/// More than this many generated targets is not a plausible free-model list.
const MAX_TARGETS: usize = 500;

/// What became of the generated file at one load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryState {
    /// No `[discovery] file` is configured.
    Off,
    /// A file is configured but `[discovery] enabled` is false, so it is not read.
    Disabled,
    /// A file is configured but does not exist yet (the first sync has not run).
    Missing,
    /// The file exists and is empty.
    Empty,
    /// The file was accepted and this many targets were merged.
    Loaded(usize),
    /// The file was rejected as a whole, for this reason; the main configuration serves alone.
    Ignored(String),
}

impl DiscoveryState {
    /// A short word for logs and the explain report.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Disabled => "disabled",
            Self::Missing => "missing",
            Self::Empty => "empty",
            Self::Loaded(_) => "loaded",
            Self::Ignored(_) => "ignored",
        }
    }

    /// How many generated targets were merged.
    #[must_use]
    pub fn loaded(&self) -> usize {
        match self {
            Self::Loaded(n) => *n,
            _ => 0,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    targets: BTreeMap<String, Generated>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Generated {
    #[serde(default)]
    class: Option<String>,
    #[serde(default)]
    strengths: Vec<String>,
    endpoints: Vec<Endpoint>,
}

/// Reads the configured file through `read` and merges its targets into `raw`. Returns the names it added
/// and what happened; on any problem nothing is added.
pub(super) fn load(
    raw: &mut RawConfig,
    read: &dyn Fn(&Path) -> Option<String>,
) -> (BTreeSet<String>, DiscoveryState) {
    let Some(path) = raw.discovery.file.clone() else {
        return (BTreeSet::new(), DiscoveryState::Off);
    };
    if !raw.discovery.enabled {
        return (BTreeSet::new(), DiscoveryState::Disabled);
    }
    let Some(text) = read(&path) else {
        return (BTreeSet::new(), DiscoveryState::Missing);
    };
    if text.trim().is_empty() {
        return (BTreeSet::new(), DiscoveryState::Empty);
    }
    match parse(&text, raw) {
        Ok(targets) => {
            let names: BTreeSet<String> = targets.iter().map(|(name, _)| name.clone()).collect();
            let count = names.len();
            raw.targets.extend(targets);
            (names, DiscoveryState::Loaded(count))
        }
        Err(reason) => (BTreeSet::new(), DiscoveryState::Ignored(reason)),
    }
}

fn is_zero(value: f64) -> bool {
    value == 0.0
}

fn parse(text: &str, raw: &RawConfig) -> Result<Vec<(String, RawTarget)>, String> {
    let file: File = toml::from_str(text)
        .map_err(|e| format!("not a generated targets file (only [targets.auto-*] tables): {e}"))?;
    if file.targets.len() > MAX_TARGETS {
        return Err(format!(
            "{} generated targets, more than the {MAX_TARGETS} allowed",
            file.targets.len()
        ));
    }
    // Providers that have opted in, and those that have opted out again with `enabled = false`: the targets
    // of the latter are skipped at once (the next sync removes them from the file), while a provider that
    // never opted in still makes the file invalid.
    let opted_in: BTreeSet<&str> = raw
        .providers
        .iter()
        .filter(|(_, p)| p.discover.as_ref().is_some_and(|d| d.enabled))
        .map(|(name, _)| name.as_str())
        .collect();
    let opted_out: BTreeSet<&str> = raw
        .providers
        .iter()
        .filter(|(_, p)| p.discover.as_ref().is_some_and(|d| !d.enabled))
        .map(|(name, _)| name.as_str())
        .collect();
    let hand_written: BTreeSet<(&str, &str)> = raw
        .targets
        .values()
        .flat_map(|t| &t.endpoints)
        .map(|e| (e.provider.as_str(), e.model.as_str()))
        .collect();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut out = Vec::new();
    for (name, target) in file.targets {
        if !name.starts_with(PREFIX) {
            return Err(format!("target `{name}` is not named `{PREFIX}*`"));
        }
        if raw.targets.contains_key(&name) {
            return Err(format!("target `{name}` is also written by hand"));
        }
        if target.endpoints.is_empty() {
            return Err(format!("target `{name}` has no endpoints"));
        }
        // A generated target can never claim another class; the configured one is applied below.
        if target
            .class
            .as_deref()
            .is_some_and(|class| class != raw.discovery.class)
        {
            return Err(format!(
                "target `{name}` claims a class other than `{}`",
                raw.discovery.class
            ));
        }
        validate_strengths(&name, &target.strengths).map_err(|e| e.to_string())?;
        if target
            .endpoints
            .iter()
            .any(|e| opted_out.contains(e.provider.as_str()))
        {
            continue;
        }
        for endpoint in &target.endpoints {
            if !opted_in.contains(endpoint.provider.as_str()) {
                return Err(format!(
                    "target `{name}` uses provider `{}`, which has not opted in to discovery",
                    endpoint.provider
                ));
            }
            let pair = (endpoint.provider.as_str(), endpoint.model.as_str());
            if hand_written.contains(&pair) {
                return Err(format!(
                    "target `{name}`: `{}/{}` is already used by a hand-written target",
                    endpoint.provider, endpoint.model
                ));
            }
            if !seen.insert((endpoint.provider.clone(), endpoint.model.clone())) {
                return Err(format!(
                    "`{}/{}` appears in more than one generated target",
                    endpoint.provider, endpoint.model
                ));
            }
            let free = endpoint.price.as_ref().is_some_and(|p| {
                is_zero(p.input)
                    && is_zero(p.output)
                    && p.cached_input.is_none_or(is_zero)
                    && p.tiers.is_empty()
            });
            if !free {
                return Err(format!(
                    "target `{name}`: `{}/{}` is not priced at zero; generated targets are free models only",
                    endpoint.provider, endpoint.model
                ));
            }
        }
        out.push((
            name,
            RawTarget {
                endpoints: target.endpoints,
                class: Some(raw.discovery.class.clone()),
                strengths: target.strengths,
                quality: None,
            },
        ));
    }
    Ok(out)
}
