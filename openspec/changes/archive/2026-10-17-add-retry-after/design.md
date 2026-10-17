# Design

## Context

`Breaker::failure` counts consecutive failures and, at `failure_threshold`, opens for
`cooldown_secs` doubled per earlier trip up to `max_cooldown_secs`. `fails_over` in `pool.rs` decides which
errors count (429, 408, 5xx, transport and timeout). A 429 from a free tier usually comes with how long to wait.

## Decisions

- **One seam: `retry_after(error) -> Option<Duration>`.** Today it parses the body; when Switchyard's error
  carries headers it reads `Retry-After` first (delta-seconds or an HTTP date) and falls back to the body. The
  breaker and everything downstream see only a duration, so adopting #904 is a change to one function.
- **Only a few well-known body shapes.** Case-insensitive phrases `try again in <d>`, `retry after <d>`,
  `retry in <d>` and `please retry in <d>`, and the JSON fields `retryDelay` (Google, `"28s"`),
  `retry_after` and `retry-after` (seconds). A duration is `<n>ms`, `<n>s`, `<n>m`, or a combination such as
  `2m3.5s`, with a bare number meaning seconds only for the JSON fields. Only the first 4 KiB of the body is
  read. Guessing wider would turn arbitrary error prose into cooldowns.
- **Open immediately on a hint.** The provider has said it will not serve before then, so counting to
  `failure_threshold` first wastes requests and, on free tiers, can burn the quota that is about to reset.
  Without a hint, behavior is exactly as today.
- **Exactly the hinted time, not doubled, not capped by `max_cooldown_secs`.** The existing cap exists because
  a guessed cooldown can be wrong; a stated one is not a guess. It is bounded by its own limit,
  `max_retry_after_secs` (default 300), so an absurd value (or a hostile body) cannot park an endpoint for
  hours. At least 1 second, so a `0` cannot make a busy loop.
- **A late failure does not shorten or extend it.** The breaker already ignores failures that arrive while it is
  open; the hinted cooldown follows the same rule. A failed probe after the cooldown is a new failure and, if it
  carries a hint, a new cooldown.
- **429 and 503 only.** Those are the statuses that mean "come back later". Other failures keep the guessed
  cooldown.
- **Untrusted text.** The body is provider-controlled. The parser reads bounded input, returns a number, and
  nothing from the body is logged or stored beyond that number.

## Risks / trade-offs

- A provider whose phrasing is not recognized falls back to today's behavior, so the interim helps the common
  providers and loses nothing on the others.
- A hint can be shorter than the real recovery time (the next request is rate-limited again): the new cooldown
  then follows from that response, so it converges.
- Two requests in flight to one endpoint can both fail with hints; the second finds the breaker already open
  and changes nothing.
