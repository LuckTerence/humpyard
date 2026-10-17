//! Strengths and pool routes (ADR 0021). A pool route takes every target that carries a strength, and
//! optionally a declared minimum context window. The membership is worked out once, when the
//! configuration loads or reloads, and written into the route's target list, so a pool is exactly a
//! hand-written route afterwards and nothing is decided per request. Strengths never remove a target.

use std::collections::BTreeMap;

use super::schema::{PoolSpec, RawConfig};
use super::{ConfigError, invalid};

const MAX_STRENGTHS: usize = 8;
const MAX_LABEL_LEN: usize = 32;

/// How one pool route resolved: who is in, and who carries the strength but was left out.
#[derive(Clone, Debug, PartialEq)]
pub struct PoolReport {
    pub strength: String,
    pub min_context_window: Option<u64>,
    pub min_quality: Option<f64>,
    /// Target names, in the order the route uses as its list (sorted).
    pub members: Vec<String>,
    pub left_out: Vec<LeftOut>,
}

/// A target that carries the strength but does not qualify.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    pub target: String,
    /// `context_window_below`, `context_window_undeclared`, `quality_below` or `quality_undeclared`.
    pub reason: &'static str,
}

pub(super) fn valid_label(label: &str) -> bool {
    (1..=MAX_LABEL_LEN).contains(&label.len())
        && label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

pub(super) fn validate_strengths(target: &str, strengths: &[String]) -> Result<(), ConfigError> {
    if strengths.len() > MAX_STRENGTHS {
        return Err(invalid(format!(
            "target `{target}`: at most {MAX_STRENGTHS} strengths"
        )));
    }
    for (i, strength) in strengths.iter().enumerate() {
        if !valid_label(strength) {
            return Err(invalid(format!(
                "target `{target}`: strength `{strength}` must be 1 to {MAX_LABEL_LEN} characters of lowercase letters, digits, `-` and `_`"
            )));
        }
        if strengths[..i].contains(strength) {
            return Err(invalid(format!(
                "target `{target}`: strength `{strength}` is listed twice"
            )));
        }
    }
    Ok(())
}

/// Validates every target's strengths, then resolves each pool route in `raw`, filling in its target
/// list. Returns how each pool resolved, by route name.
pub(super) fn resolve(raw: &mut RawConfig) -> Result<BTreeMap<String, PoolReport>, ConfigError> {
    for (id, target) in &raw.targets {
        validate_strengths(id, &target.strengths)?;
        if target.quality.is_some_and(|q| !(0.0..=1.0).contains(&q)) {
            return Err(invalid(format!(
                "target `{id}`: `quality` must be between 0 and 1"
            )));
        }
    }
    let mut reports = BTreeMap::new();
    let RawConfig {
        targets, routes, ..
    } = raw;
    for (route_id, spec) in routes.iter_mut() {
        let Some((list, pool, has_weights)) = spec.pool_parts_mut() else {
            continue;
        };
        let Some(pool) = pool.cloned() else {
            continue;
        };
        if !list.is_empty() {
            return Err(invalid(format!(
                "route `{route_id}`: `pool` and `targets` cannot be combined"
            )));
        }
        if has_weights {
            return Err(invalid(format!(
                "route `{route_id}`: `weights` cannot be combined with `pool`"
            )));
        }
        check_pool(route_id, &pool)?;
        // BTreeMap iteration is by name, which is the order the route uses.
        let mut report = PoolReport {
            strength: pool.strength.clone(),
            min_context_window: pool.min_context_window,
            min_quality: pool.min_quality,
            members: Vec::new(),
            left_out: Vec::new(),
        };
        for (name, target) in targets.iter() {
            if !target.strengths.contains(&pool.strength) {
                continue;
            }
            match qualifies(pool.min_context_window, pool.min_quality, target) {
                Ok(()) => report.members.push(name.clone()),
                Err(reason) => report.left_out.push(LeftOut {
                    target: name.clone(),
                    reason,
                }),
            }
        }
        if report.members.is_empty() {
            return Err(invalid(empty_pool_message(
                route_id, &pool, &report, targets,
            )));
        }
        list.clone_from(&report.members);
        reports.insert(route_id.clone(), report);
    }
    Ok(reports)
}

fn check_pool(route_id: &str, pool: &PoolSpec) -> Result<(), ConfigError> {
    if !valid_label(&pool.strength) {
        return Err(invalid(format!(
            "route `{route_id}`: pool strength `{}` must be 1 to {MAX_LABEL_LEN} characters of lowercase letters, digits, `-` and `_`",
            pool.strength
        )));
    }
    if pool.min_context_window == Some(0) {
        return Err(invalid(format!(
            "route `{route_id}`: `min_context_window` must be at least 1"
        )));
    }
    if pool.min_quality.is_some_and(|q| !(0.0..=1.0).contains(&q)) {
        return Err(invalid(format!(
            "route `{route_id}`: `min_quality` must be between 0 and 1"
        )));
    }
    Ok(())
}

/// Whether a target meets a pool's minimum window: some endpoint declares at least that window. An
/// undeclared window does not qualify (this is selection by the operator, not request-time removal).
fn qualifies(
    min_window: Option<u64>,
    min_quality: Option<f64>,
    target: &super::schema::RawTarget,
) -> Result<(), &'static str> {
    if let Some(min) = min_window {
        let declared: Vec<u64> = target
            .endpoints
            .iter()
            .filter_map(|e| e.context_window)
            .collect();
        if declared.is_empty() {
            return Err("context_window_undeclared");
        }
        if !declared.iter().any(|window| *window >= min) {
            return Err("context_window_below");
        }
    }
    // Likewise a quality the operator has not stated does not qualify for a quality floor.
    if let Some(min) = min_quality {
        match target.quality {
            None => return Err("quality_undeclared"),
            Some(quality) if quality < min => return Err("quality_below"),
            Some(_) => {}
        }
    }
    Ok(())
}

fn empty_pool_message(
    route_id: &str,
    pool: &PoolSpec,
    report: &PoolReport,
    targets: &BTreeMap<String, super::schema::RawTarget>,
) -> String {
    if report.left_out.is_empty() {
        let mut in_use: Vec<&str> = targets
            .values()
            .flat_map(|t| t.strengths.iter().map(String::as_str))
            .collect();
        in_use.sort_unstable();
        in_use.dedup();
        let known = if in_use.is_empty() {
            "no target declares any strengths".to_string()
        } else {
            format!("strengths in use: {}", in_use.join(", "))
        };
        format!(
            "route `{route_id}`: pool strength `{}` matches no target ({known})",
            pool.strength
        )
    } else {
        let left: Vec<String> = report
            .left_out
            .iter()
            .map(|l| format!("{} ({})", l.target, l.reason))
            .collect();
        format!(
            "route `{route_id}`: no target with strength `{}` qualifies; left out: {}",
            pool.strength,
            left.join(", ")
        )
    }
}
