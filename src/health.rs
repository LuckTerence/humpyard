//! Endpoint health: a circuit breaker per endpoint. After enough consecutive failures an endpoint
//! is skipped for a cooldown that doubles on each repeated trip; when it ends, one request is
//! admitted as a probe. An endpoint that states when it will serve again (a retry time) is cooled for
//! exactly that long instead. The breaker only decides; the pool decides what counts as a failure and
//! never lets health deny service (see ADR 0009).

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::clock::Clock;
use crate::config::HealthConfig;

/// What the breaker says about sending a request to its endpoint now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    /// Healthy: send it.
    Allowed,
    /// The cooldown ended: send it as the single recovery probe.
    Probe,
    /// Cooling down (or a probe is already out): skip.
    Skip,
}

/// A change worth logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    /// The breaker opened for this many milliseconds.
    Opened { cooldown_ms: i64 },
    /// A previously open breaker closed again.
    Recovered,
}

/// A point-in-time view for reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub state: State,
    pub consecutive_failures: u32,
    /// Milliseconds until the cooldown ends (0 when healthy or already due for a probe).
    pub cooldown_remaining_ms: i64,
    /// Why the breaker last opened; `None` while it never has since it closed.
    pub cooldown_source: Option<CooldownSource>,
}

/// What set the length of a cooldown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CooldownSource {
    /// Consecutive failures, with the configured (doubling) cooldown.
    Failures,
    /// The endpoint stated when it would serve again.
    RetryAfter,
}

impl CooldownSource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Failures => "failures",
            Self::RetryAfter => "retry_after",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Healthy,
    CoolingDown,
    /// The cooldown ended; the next request (or the one in flight) is the probe.
    Probing,
}

impl State {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::CoolingDown => "cooling_down",
            Self::Probing => "probing",
        }
    }
}

#[derive(Default)]
struct Inner {
    consecutive_failures: u32,
    /// Times the breaker opened since it last closed; sets the cooldown length.
    trips: u32,
    open_until_ms: Option<i64>,
    probing: bool,
    source: Option<CooldownSource>,
}

pub struct Breaker {
    config: HealthConfig,
    clock: Arc<dyn Clock>,
    inner: Mutex<Inner>,
}

impl Breaker {
    pub fn new(config: HealthConfig, clock: Arc<dyn Clock>) -> Self {
        Self {
            config,
            clock,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// The settings this breaker was built with.
    pub fn config(&self) -> HealthConfig {
        self.config
    }

    fn enabled(&self) -> bool {
        self.config.failure_threshold > 0
    }

    /// Whether to send a request now. Admitting the probe is a state change: concurrent callers
    /// see `Skip` until the probe resolves or is released.
    pub fn admit(&self) -> Admission {
        if !self.enabled() {
            return Admission::Allowed;
        }
        let mut inner = self.inner.lock();
        match inner.open_until_ms {
            None => Admission::Allowed,
            Some(until) if self.clock.now_ms() < until => Admission::Skip,
            Some(_) if inner.probing => Admission::Skip,
            Some(_) => {
                inner.probing = true;
                Admission::Probe
            }
        }
    }

    /// A request to the endpoint succeeded: back to healthy.
    pub fn success(&self) -> Option<Transition> {
        if !self.enabled() {
            return None;
        }
        let mut inner = self.inner.lock();
        let was_open = inner.open_until_ms.is_some();
        *inner = Inner::default();
        was_open.then_some(Transition::Recovered)
    }

    /// A request failed in a way that says the endpoint is unhealthy (the caller decides which).
    pub fn failure(&self) -> Option<Transition> {
        if !self.enabled() {
            return None;
        }
        let mut inner = self.inner.lock();
        if inner.probing {
            return Some(self.trip(&mut inner));
        }
        if inner.open_until_ms.is_some() {
            return None; // a late failure from a request sent before the breaker opened
        }
        inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
        (inner.consecutive_failures >= self.config.failure_threshold).then(|| self.trip(&mut inner))
    }

    /// A request failed and the endpoint stated when it would serve again: cool it for exactly
    /// `wait` (already clamped by the caller), without waiting for `failure_threshold` failures and
    /// without the doubling. It does not count as a trip, so a later unannounced failure streak is not
    /// escalated by it. Failures that arrive while it is cooling change nothing.
    pub fn failure_for(&self, wait: Duration) -> Option<Transition> {
        if !self.enabled() {
            return None;
        }
        let mut inner = self.inner.lock();
        if inner.open_until_ms.is_some() && !inner.probing {
            return None; // a late failure from a request sent before the breaker opened
        }
        inner.consecutive_failures = inner.consecutive_failures.saturating_add(1);
        inner.probing = false;
        let cooldown_ms = i64::try_from(wait.as_millis()).unwrap_or(i64::MAX);
        inner.open_until_ms = Some(self.clock.now_ms().saturating_add(cooldown_ms));
        inner.source = Some(CooldownSource::RetryAfter);
        Some(Transition::Opened { cooldown_ms })
    }

    /// The admitted probe ended without a verdict (a non-health error, or the request was
    /// cancelled): let the next request probe instead.
    pub fn release_probe(&self) {
        self.inner.lock().probing = false;
    }

    /// Milliseconds until the cooldown ends; 0 when healthy or already due.
    pub fn cooldown_remaining_ms(&self) -> i64 {
        self.inner
            .lock()
            .open_until_ms
            .map_or(0, |until| (until - self.clock.now_ms()).max(0))
    }

    pub fn snapshot(&self) -> Snapshot {
        let inner = self.inner.lock();
        let now = self.clock.now_ms();
        let (state, remaining) = match inner.open_until_ms {
            None => (State::Healthy, 0),
            Some(until) if now < until => (State::CoolingDown, until - now),
            Some(_) => (State::Probing, 0),
        };
        Snapshot {
            state,
            consecutive_failures: inner.consecutive_failures,
            cooldown_remaining_ms: remaining,
            cooldown_source: inner.source,
        }
    }

    fn trip(&self, inner: &mut Inner) -> Transition {
        inner.trips = inner.trips.saturating_add(1);
        inner.probing = false;
        let cooldown_ms = self.cooldown_ms(inner.trips);
        inner.open_until_ms = Some(self.clock.now_ms().saturating_add(cooldown_ms));
        inner.source = Some(CooldownSource::Failures);
        Transition::Opened { cooldown_ms }
    }

    /// `cooldown_secs` doubled for every earlier trip, capped at `max_cooldown_secs`.
    fn cooldown_ms(&self, trips: u32) -> i64 {
        let doublings = trips.saturating_sub(1).min(32);
        let secs = self
            .config
            .cooldown_secs
            .saturating_mul(1_u64 << doublings)
            .min(self.config.max_cooldown_secs);
        i64::try_from(secs.saturating_mul(1000)).unwrap_or(i64::MAX)
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

    fn breaker(threshold: u32) -> (Breaker, Arc<Manual>) {
        let clock = Arc::new(Manual(AtomicI64::new(1_000_000)));
        let config = HealthConfig {
            failure_threshold: threshold,
            cooldown_secs: 10,
            max_cooldown_secs: 35,
            max_retry_after_secs: 300,
            skip_cold_targets: true,
        };
        (Breaker::new(config, clock.clone()), clock)
    }

    fn advance(clock: &Manual, secs: i64) {
        clock.0.fetch_add(secs * 1000, Ordering::SeqCst);
    }

    #[test]
    fn opens_only_after_the_threshold_of_consecutive_failures() {
        let (b, _) = breaker(3);
        assert_eq!(b.failure(), None);
        assert_eq!(b.failure(), None);
        assert_eq!(b.admit(), Admission::Allowed);
        assert_eq!(
            b.failure(),
            Some(Transition::Opened {
                cooldown_ms: 10_000
            })
        );
        assert_eq!(b.admit(), Admission::Skip);
    }

    #[test]
    fn a_success_resets_the_count() {
        let (b, _) = breaker(3);
        b.failure();
        b.failure();
        assert_eq!(b.success(), None, "nothing to recover from while closed");
        b.failure();
        b.failure();
        assert_eq!(b.admit(), Admission::Allowed);
    }

    #[test]
    fn after_the_cooldown_exactly_one_probe_is_admitted() {
        let (b, clock) = breaker(1);
        b.failure();
        advance(&clock, 9);
        assert_eq!(b.admit(), Admission::Skip);
        advance(&clock, 2);
        assert_eq!(b.snapshot().state, State::Probing);
        assert_eq!(b.admit(), Admission::Probe);
        assert_eq!(b.admit(), Admission::Skip, "the probe is out");
        assert_eq!(b.success(), Some(Transition::Recovered));
        assert_eq!(b.admit(), Admission::Allowed);
        assert_eq!(b.snapshot().state, State::Healthy);
    }

    #[test]
    fn a_failed_probe_reopens_with_a_doubled_cooldown_up_to_the_cap() {
        let (b, clock) = breaker(1);
        assert_eq!(
            b.failure(),
            Some(Transition::Opened {
                cooldown_ms: 10_000
            })
        );
        for expected in [20_000, 35_000, 35_000] {
            advance(&clock, 40);
            assert_eq!(b.admit(), Admission::Probe);
            assert_eq!(
                b.failure(),
                Some(Transition::Opened {
                    cooldown_ms: expected
                })
            );
        }
    }

    #[test]
    fn recovery_resets_the_cooldown_growth() {
        let (b, clock) = breaker(1);
        b.failure();
        advance(&clock, 11);
        b.admit();
        b.failure(); // 20s
        advance(&clock, 21);
        b.admit();
        b.success();
        assert_eq!(
            b.failure(),
            Some(Transition::Opened {
                cooldown_ms: 10_000
            })
        );
    }

    #[test]
    fn a_late_failure_while_open_does_not_extend_the_cooldown() {
        let (b, _) = breaker(1);
        b.failure();
        let before = b.cooldown_remaining_ms();
        assert_eq!(b.failure(), None);
        assert_eq!(b.cooldown_remaining_ms(), before);
    }

    #[test]
    fn a_released_probe_lets_the_next_request_probe() {
        let (b, clock) = breaker(1);
        b.failure();
        advance(&clock, 11);
        assert_eq!(b.admit(), Admission::Probe);
        b.release_probe();
        assert_eq!(b.admit(), Admission::Probe);
    }

    #[test]
    fn threshold_zero_disables_everything() {
        let (b, _) = breaker(0);
        for _ in 0..10 {
            assert_eq!(b.failure(), None);
        }
        assert_eq!(b.admit(), Admission::Allowed);
        assert_eq!(b.snapshot().state, State::Healthy);
    }

    fn advance_ms(clock: &Manual, ms: i64) {
        clock.0.fetch_add(ms, Ordering::SeqCst);
    }

    #[test]
    fn a_stated_retry_time_opens_at_once_for_exactly_that_long() {
        let (b, clock) = breaker(3);
        assert_eq!(
            b.failure_for(Duration::from_secs(20)),
            Some(Transition::Opened {
                cooldown_ms: 20_000
            })
        );
        assert_eq!(b.admit(), Admission::Skip);
        advance_ms(&clock, 19_999);
        assert_eq!(b.admit(), Admission::Skip);
        advance_ms(&clock, 1);
        assert_eq!(b.admit(), Admission::Probe);
        assert_eq!(
            b.snapshot().cooldown_source,
            Some(CooldownSource::RetryAfter)
        );
    }

    #[test]
    fn a_stated_time_is_not_capped_by_the_guessed_cooldown_or_doubled() {
        let (b, _) = breaker(3);
        // The test config caps guessed cooldowns at 35 s; a stated 120 s is honored in full.
        assert_eq!(
            b.failure_for(Duration::from_secs(120)),
            Some(Transition::Opened {
                cooldown_ms: 120_000
            })
        );
        assert_eq!(b.cooldown_remaining_ms(), 120_000);
    }

    #[test]
    fn late_failures_do_not_change_a_stated_cooldown() {
        let (b, clock) = breaker(3);
        b.failure_for(Duration::from_secs(20));
        advance_ms(&clock, 5_000);
        assert_eq!(b.failure_for(Duration::from_secs(60)), None);
        assert_eq!(b.failure(), None);
        assert_eq!(b.cooldown_remaining_ms(), 15_000);
    }

    #[test]
    fn a_failed_probe_with_a_new_stated_time_reopens_for_that_time() {
        let (b, clock) = breaker(3);
        b.failure_for(Duration::from_secs(10));
        advance_ms(&clock, 10_000);
        assert_eq!(b.admit(), Admission::Probe);
        assert_eq!(
            b.failure_for(Duration::from_secs(40)),
            Some(Transition::Opened {
                cooldown_ms: 40_000
            })
        );
        assert_eq!(b.admit(), Admission::Skip);
    }

    #[test]
    fn a_stated_time_does_not_escalate_the_guessed_cooldown() {
        let (b, clock) = breaker(3);
        b.failure_for(Duration::from_secs(10));
        advance_ms(&clock, 10_000);
        assert_eq!(b.admit(), Admission::Probe);
        b.success();
        // Three ordinary failures trip the breaker at the base cooldown, not a doubled one.
        for _ in 0..2 {
            assert_eq!(b.failure(), None);
        }
        assert_eq!(
            b.failure(),
            Some(Transition::Opened {
                cooldown_ms: 10_000
            })
        );
        assert_eq!(b.snapshot().cooldown_source, Some(CooldownSource::Failures));
    }

    #[test]
    fn a_disabled_breaker_ignores_stated_times() {
        let (b, _) = breaker(0);
        assert_eq!(b.failure_for(Duration::from_secs(20)), None);
        assert_eq!(b.admit(), Admission::Allowed);
    }
}
