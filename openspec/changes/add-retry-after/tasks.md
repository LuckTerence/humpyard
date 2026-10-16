# Tasks

## 1. Parser and breaker
- [x] 1.1 `src/retry_after.rs`: the body parser (phrases, `retryDelay`, `retry_after`, durations in ms/s/m/combinations), bounded input; unit tests for each shape, units, case, junk, zero, negative, huge values and a body past 4 KiB
- [x] 1.2 `[health] max_retry_after_secs` (default 300): config, validation and a docs row
- [x] 1.3 `Breaker::failure_for(duration)`: open immediately for exactly that long, no doubling, late failures ignored; unit tests for each scenario and that the guessed path is unchanged

## 2. Wiring and visibility
- [x] 2.1 `src/pool.rs`: on a 429 or 503 failure read the retry time and call the breaker; a counter `humpyard_retry_after_honored_total{provider,source}` and docs in `metrics.md`
- [x] 2.2 Endpoint health snapshot shows the cooldown source; a test through `/v1/health` or the explain output
- [x] 2.3 End to end: a mock upstream answering 429 with "try again in N seconds" is skipped for that time while another target serves, then probed; a hint-less 429 behaves as before

## 3. Docs and follow-up
- [x] 3.1 `routing.md` (Endpoint health), `configuration.md`, the ADR 0018 amendment, the roadmap marks step 1 shipped with the header read still to follow Switchyard#904
