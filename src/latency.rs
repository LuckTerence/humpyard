//! Observed time per target, and the ordering a `fastest` route derives from it (ADR 0020).
//!
//! One smoothed figure per target, kept in memory and updated from the attempts the gateway already
//! times, so nothing is sent that a client did not ask for. The figure is the time until the upstream
//! answered: the full time for a buffered call, and until the response began for a streamed one.
//! Only answered attempts update it: a failure counted as slow would make a dying target look merely
//! slow, and as fast would be worse; failures are health's business (ADR 0018). A restart starts empty,
//! which only costs a few exploratory requests.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::clock::Clock;
use crate::num::{f64_from_u64, u64_from_f64_rounded};

/// Weight of the newest sample in the smoothed figure.
const WEIGHT: f64 = 0.3;

/// What is known about one target's speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Figure {
    /// The smoothed time to answer, in milliseconds.
    pub ms: f64,
    /// Answered attempts that went into it.
    pub samples: u32,
    /// When the newest sample arrived (unix milliseconds).
    pub updated_ms: i64,
}

/// The smoothed answer time of every target that has answered.
pub struct Latency {
    clock: Arc<dyn Clock>,
    figures: Mutex<HashMap<String, Figure>>,
}

impl Latency {
    pub fn new(clock: Arc<dyn Clock>) -> Self {
        Self {
            clock,
            figures: Mutex::new(HashMap::new()),
        }
    }

    /// An attempt on `target` was answered after `ms` milliseconds.
    pub fn record(&self, target: &str, ms: u64) {
        let sample = f64_from_u64(ms);
        let now = self.clock.now_ms();
        let mut figures = self.figures.lock();
        match figures.get_mut(target) {
            Some(figure) => {
                figure.ms += WEIGHT * (sample - figure.ms);
                figure.samples = figure.samples.saturating_add(1);
                figure.updated_ms = now;
            }
            None => {
                figures.insert(
                    target.to_string(),
                    Figure {
                        ms: sample,
                        samples: 1,
                        updated_ms: now,
                    },
                );
            }
        }
    }

    pub fn figure(&self, target: &str) -> Option<Figure> {
        self.figures.lock().get(target).copied()
    }

    /// The figure, only when it is trustworthy: enough samples, and recent enough.
    fn measured(&self, target: &str, settings: &Settings) -> Option<Figure> {
        let now = self.clock.now_ms();
        self.figure(target).filter(|f| {
            f.samples >= settings.min_samples
                && now.saturating_sub(f.updated_ms) <= settings.max_age_ms
        })
    }

    /// Orders `targets` (the eligible ones, in the route's written order) for one request: unmeasured
    /// targets first in written order, then measured ones fastest first; with probability
    /// `settings.explore` a random target goes first instead. `draw` yields uniform values in `[0, 1)`.
    ///
    /// With `settings.quality_weight` above 0, measured targets are ordered by
    /// `(1 - w) * speed + w * quality` instead (`quality` from `quality`, or `settings.unknown_quality`),
    /// ties by observed speed; with 0 the order is exactly the speed order.
    pub fn rank(
        &self,
        targets: &[String],
        settings: &Settings,
        quality: &dyn Fn(&str) -> Option<f64>,
        mut draw: impl FnMut() -> f64,
    ) -> Ranked {
        let mut unmeasured = Vec::new();
        let mut measured: Vec<(&String, f64)> = Vec::new();
        let mut scores = Vec::with_capacity(targets.len());
        for target in targets {
            let declared = quality(target);
            if let Some(figure) = self.measured(target, settings) {
                measured.push((target, figure.ms));
                let speed = (settings.latency_target_ms / figure.ms.max(1.0)).min(1.0);
                let blended = (1.0 - settings.quality_weight) * speed
                    + settings.quality_weight * declared.unwrap_or(settings.unknown_quality);
                scores.push(Score {
                    target: target.clone(),
                    observed_ms: Some(figure.ms),
                    speed: Some(speed),
                    quality: declared,
                    score: (settings.quality_weight > 0.0).then_some(blended),
                });
            } else {
                unmeasured.push(target.clone());
                scores.push(Score {
                    target: target.clone(),
                    observed_ms: None,
                    speed: None,
                    quality: declared,
                    score: None,
                });
            }
        }
        let score_of = |target: &str| {
            scores
                .iter()
                .find(|s| s.target == target)
                .and_then(|s| s.score)
                .unwrap_or(0.0)
        };
        // A stable sort keeps the written order between equal figures. With no quality weight this is the
        // plain speed order; with one, the blended score decides and speed breaks ties.
        measured.sort_by(|a, b| {
            if settings.quality_weight > 0.0 {
                score_of(b.0)
                    .total_cmp(&score_of(a.0))
                    .then(a.1.total_cmp(&b.1))
            } else {
                a.1.total_cmp(&b.1)
            }
        });
        let mut order = unmeasured;
        order.extend(measured.into_iter().map(|(target, _)| target.clone()));
        let mut reason = Reason::Fastest;
        if order.len() > 1 && settings.explore > 0.0 && draw() < settings.explore {
            let pick = pick_index(draw(), order.len());
            let chosen = order.remove(pick);
            order.insert(0, chosen);
            reason = Reason::Explore;
        }
        Ranked {
            order,
            reason,
            scores,
        }
    }
}

/// How a `fastest` route weighs targets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    /// Probability that a request tries a random eligible target first.
    pub explore: f64,
    /// Answered attempts a target needs before its figure is believed.
    pub min_samples: u32,
    /// A figure older than this counts as unmeasured.
    pub max_age_ms: i64,
    /// How much declared quality counts against speed (0 to 1; 0 is the pure speed order).
    pub quality_weight: f64,
    /// The answer time (milliseconds) that earns full speed marks.
    pub latency_target_ms: f64,
    /// The quality assumed for a target that declares none.
    pub unknown_quality: f64,
}

/// The order one request will use, and why its first target is first.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub order: Vec<String>,
    pub reason: Reason,
    /// How each target scored (for `explain`), in the order the targets were given.
    pub scores: Vec<Score>,
}

/// One target's figures behind an order.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub target: String,
    /// The smoothed time to answer, when the target is measured.
    pub observed_ms: Option<f64>,
    /// `min(1, latency_target_ms / observed_ms)`, when measured.
    pub speed: Option<f64>,
    /// The declared quality, if any.
    pub quality: Option<f64>,
    /// `(1 - w) * speed + w * quality`, when measured and the route has a quality weight.
    pub score: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The first target is the fastest known, or the first one still to be measured.
    Fastest,
    /// The first target was drawn at random.
    Explore,
}

impl Reason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fastest => "fastest",
            Self::Explore => "explore",
        }
    }
}

/// `draw` in `[0, 1)` as an index below `len`.
fn pick_index(draw: f64, len: usize) -> usize {
    let scaled = draw * f64_from_u64(u64::try_from(len).unwrap_or(u64::MAX));
    // Rounding `x - 0.5` is the floor of `x` for the non-negative values drawn here.
    usize::try_from(u64_from_f64_rounded(scaled - 0.5))
        .unwrap_or(usize::MAX)
        .min(len.saturating_sub(1))
}

/// A small seedable generator (`SplitMix64`) for the exploration draw. It does not need to be
/// unpredictable, only reproducible when a route is given a `seed`.
pub struct Draws(Mutex<u64>);

impl Draws {
    pub fn new(seed: Option<u64>) -> Self {
        let seed = seed.unwrap_or_else(|| getrandom::u64().unwrap_or(0x9E37_79B9_7F4A_7C15));
        Self(Mutex::new(seed))
    }

    /// The next value in `[0, 1)`.
    pub fn next(&self) -> f64 {
        let mut state = self.0.lock();
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // 53 random bits make an exactly representable fraction below one.
        f64_from_u64(z >> 11) / 9_007_199_254_740_992.0
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};

    use super::*;

    struct Manual(AtomicI64);
    impl Clock for Manual {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn tracker() -> (Latency, Arc<Manual>) {
        let clock = Arc::new(Manual(AtomicI64::new(1_000_000)));
        (Latency::new(clock.clone()), clock)
    }

    fn settings() -> Settings {
        Settings {
            explore: 0.0,
            min_samples: 3,
            max_age_ms: 600_000,
            quality_weight: 0.0,
            latency_target_ms: 2000.0,
            unknown_quality: 0.5,
        }
    }

    fn no_quality(_: &str) -> Option<f64> {
        None
    }

    fn blend(weight: f64) -> Settings {
        Settings {
            quality_weight: weight,
            latency_target_ms: 2000.0,
            ..settings()
        }
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    fn feed(latency: &Latency, target: &str, ms: u64, times: u32) {
        for _ in 0..times {
            latency.record(target, ms);
        }
    }

    #[test]
    fn the_first_sample_is_the_figure_and_later_ones_are_smoothed() {
        let (latency, _) = tracker();
        latency.record("a", 1000);
        assert_eq!(latency.figure("a").map(|f| f.ms), Some(1000.0));
        latency.record("a", 0);
        let figure = latency.figure("a").unwrap();
        assert!((figure.ms - 700.0).abs() < 1e-9, "{}", figure.ms);
        assert_eq!(figure.samples, 2);
        assert_eq!(latency.figure("nobody"), None);
    }

    #[test]
    fn measured_targets_are_ordered_fastest_first() {
        let (latency, _) = tracker();
        feed(&latency, "slow", 900, 3);
        feed(&latency, "mid", 400, 3);
        feed(&latency, "quick", 100, 3);
        let ranked = latency.rank(
            &names(&["slow", "mid", "quick"]),
            &settings(),
            &no_quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["quick", "mid", "slow"]));
        assert_eq!(ranked.reason, Reason::Fastest);
    }

    #[test]
    fn unmeasured_targets_go_first_in_written_order() {
        let (latency, _) = tracker();
        feed(&latency, "known", 100, 3);
        feed(&latency, "thin", 50, 2); // below min_samples
        let ranked = latency.rank(
            &names(&["known", "new", "thin", "newer"]),
            &settings(),
            &no_quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["new", "thin", "newer", "known"]));
    }

    #[test]
    fn a_stale_figure_counts_as_unmeasured() {
        let (latency, clock) = tracker();
        feed(&latency, "a", 100, 3);
        feed(&latency, "b", 900, 3);
        clock.0.fetch_add(600_001, Ordering::SeqCst);
        feed(&latency, "b", 900, 1); // b is fresh again, a is not
        let ranked = latency.rank(&names(&["b", "a"]), &settings(), &no_quality, || 0.99);
        assert_eq!(
            ranked.order,
            names(&["a", "b"]),
            "stale a is tried first to be measured again"
        );
    }

    #[test]
    fn equal_figures_keep_the_written_order() {
        let (latency, _) = tracker();
        feed(&latency, "x", 300, 3);
        feed(&latency, "y", 300, 3);
        let ranked = latency.rank(&names(&["y", "x"]), &settings(), &no_quality, || 0.99);
        assert_eq!(ranked.order, names(&["y", "x"]));
    }

    #[test]
    fn exploration_puts_a_random_target_first() {
        let (latency, _) = tracker();
        feed(&latency, "a", 100, 3);
        feed(&latency, "b", 200, 3);
        feed(&latency, "c", 300, 3);
        let explore = Settings {
            explore: 0.5,
            ..settings()
        };
        // First draw 0.1 < 0.5 selects exploration; second draw 0.9 picks index 2 of [a, b, c].
        let mut draws = [0.1, 0.9].into_iter();
        let ranked = latency.rank(&names(&["a", "b", "c"]), &explore, &no_quality, || {
            draws.next().unwrap()
        });
        assert_eq!(ranked.order, names(&["c", "a", "b"]));
        assert_eq!(ranked.reason, Reason::Explore);
        // A draw at or above the probability does not explore.
        let mut draws = [0.5].into_iter();
        let ranked = latency.rank(&names(&["a", "b", "c"]), &explore, &no_quality, || {
            draws.next().unwrap()
        });
        assert_eq!(ranked.order, names(&["a", "b", "c"]));
        assert_eq!(ranked.reason, Reason::Fastest);
    }

    #[test]
    fn zero_exploration_never_draws_and_one_target_never_explores() {
        let (latency, _) = tracker();
        feed(&latency, "a", 100, 3);
        let always = Settings {
            explore: 1.0,
            ..settings()
        };
        let ranked = latency.rank(&names(&["a"]), &always, &no_quality, || {
            panic!("no draw for one target")
        });
        assert_eq!(ranked.order, names(&["a"]));
        let ranked = latency.rank(&names(&["a", "b"]), &settings(), &no_quality, || {
            panic!("explore = 0 never draws")
        });
        assert_eq!(ranked.order.len(), 2);
    }

    #[test]
    fn draws_are_reproducible_and_in_range() {
        let a = Draws::new(Some(7));
        let b = Draws::new(Some(7));
        for _ in 0..1000 {
            let (x, y) = (a.next(), b.next());
            assert!((x - y).abs() < f64::EPSILON);
            assert!((0.0..1.0).contains(&x), "{x}");
        }
        assert!((Draws::new(Some(8)).next() - Draws::new(Some(7)).next()).abs() > f64::EPSILON);
    }

    #[test]
    fn weight_zero_is_the_speed_order_whatever_the_qualities() {
        let (latency, _) = tracker();
        feed(&latency, "quick-weak", 300, 3);
        feed(&latency, "slow-strong", 900, 3);
        let quality = |t: &str| (t == "slow-strong").then_some(0.99);
        let ranked = latency.rank(
            &names(&["slow-strong", "quick-weak"]),
            &blend(0.0),
            &quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["quick-weak", "slow-strong"]));
        assert!(
            ranked.scores.iter().all(|s| s.score.is_none()),
            "no score without a weight"
        );
    }

    #[test]
    fn quality_outweighs_a_small_speed_gap() {
        // Target 2000 ms: both are under it, so speed is 1.0 for each; quality decides.
        let (latency, _) = tracker();
        feed(&latency, "fast-weak", 300, 3);
        feed(&latency, "mid-strong", 600, 3);
        let quality = |t: &str| Some(if t == "mid-strong" { 0.8 } else { 0.4 });
        let ranked = latency.rank(
            &names(&["fast-weak", "mid-strong"]),
            &blend(0.7),
            &quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["mid-strong", "fast-weak"]));
        let strong = ranked
            .scores
            .iter()
            .find(|s| s.target == "mid-strong")
            .unwrap();
        assert!((strong.score.unwrap() - (0.3 * 1.0 + 0.7 * 0.8)).abs() < 1e-9);
    }

    #[test]
    fn a_very_slow_strong_model_does_not_lead() {
        let (latency, _) = tracker();
        feed(&latency, "glacial-great", 20_000, 3);
        feed(&latency, "brisk-ok", 800, 3);
        let quality = |t: &str| Some(if t == "glacial-great" { 0.9 } else { 0.6 });
        let ranked = latency.rank(
            &names(&["glacial-great", "brisk-ok"]),
            &blend(0.5),
            &quality,
            || 0.99,
        );
        // speed 0.1 for the slow one: 0.5*0.1 + 0.5*0.9 = 0.50; speed 1.0 for the other: 0.5 + 0.3 = 0.80.
        assert_eq!(ranked.order, names(&["brisk-ok", "glacial-great"]));
    }

    #[test]
    fn an_undeclared_quality_counts_as_the_neutral_value_and_ties_break_by_speed() {
        let (latency, _) = tracker();
        feed(&latency, "unknown", 300, 3);
        feed(&latency, "declared", 100, 3);
        // Both reach full speed marks; `declared` has 0.5 as well, so they tie and the faster one leads.
        let quality = |t: &str| (t == "declared").then_some(0.5);
        let ranked = latency.rank(
            &names(&["unknown", "declared"]),
            &blend(0.5),
            &quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["declared", "unknown"]));
        // Below the neutral value an unknown target outranks a declared weak one.
        let weak = |t: &str| (t == "declared").then_some(0.1);
        let ranked = latency.rank(&names(&["declared", "unknown"]), &blend(0.5), &weak, || {
            0.99
        });
        assert_eq!(ranked.order, names(&["unknown", "declared"]));
    }

    #[test]
    fn unmeasured_targets_still_go_first_with_a_quality_weight() {
        let (latency, _) = tracker();
        feed(&latency, "known-strong", 100, 3);
        let quality = |_: &str| Some(0.9);
        let ranked = latency.rank(
            &names(&["known-strong", "fresh"]),
            &blend(0.9),
            &quality,
            || 0.99,
        );
        assert_eq!(ranked.order, names(&["fresh", "known-strong"]));
        let fresh = ranked.scores.iter().find(|s| s.target == "fresh").unwrap();
        assert_eq!(fresh.score, None, "an unmeasured target has no score yet");
    }
}
