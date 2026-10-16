# Design

## Context

`request_info` builds Switchyard's `Metadata` from the headers and takes the session id from it. The same
`Metadata` is handed to the routing algorithm, so a session id affects state (escalation latches,
capable-hold turns), and it is recorded on decisions and usage rows, so it feeds the session analytics.

## Decisions

- **Fallbacks run after Switchyard's, never before.** A client that sends a name Switchyard recognizes keeps
  exactly today's behavior; the new names only matter when none of those carries an id. This keeps the
  change additive and ordered by how explicit the name is (`x-switchyard-session-id` first).
- **Set it on the metadata, not only on the decision.** If a fallback id were recorded but not given to
  Switchyard, analytics would see a session that the algorithm treats as stateless. The id is written into
  `Metadata.session_id`, so both agree.
- **Only the two session-shaped names.** `session_id` is what OpenAI-style clients (OpenClaw's affinity mode
  among them) send; `x-session-affinity` is sent alongside it. `x-client-request-id` identifies a request, so
  using it would make every request its own session. `prompt_cache_key` stays out because it is a body field
  clients may set to a constant, which would silently merge unrelated conversations into one session and
  share algorithm state between them; if it proves safe in practice it can be added with its own decision.
- **Validate, do not fail.** An id outside 1 to 256 visible ASCII characters is ignored, because a client's
  header is untrusted text that ends up in the ledger and in statistics responses, and a bad value must never
  cost a request.

## Risks / trade-offs

- A client that sends the same `session_id` for unrelated conversations merges them, as it already could
  with `x-switchyard-session-id`. Per-key statistics isolation keeps it from crossing keys.
- Turning on session ids for a client that had none starts per-session algorithm state for its routes. That is
  the intended behavior of those algorithms, but it is a change for that client.
