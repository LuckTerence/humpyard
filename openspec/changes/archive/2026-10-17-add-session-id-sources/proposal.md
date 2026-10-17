# Proposal

## Why

The session analytics (add-session-analytics) and Switchyard's per-session algorithm state both start from a
session id taken from the request headers. Switchyard recognizes `x-switchyard-session-id`,
`x-claude-code-session-id`, `x-nemo-relay-session-id`, `x-session-id`, `session-id`, a Dynamo header and
Codex's turn-metadata header. On the reference deployment 0 of 202 answered calls carried one: OpenClaw sends
none by default, and when asked to (`compat.sendSessionAffinityHeaders`) it sends `session_id` (with an
underscore) and `x-session-affinity`, which are not on that list, so humpyard would ignore them. The
session views are empty for an agent that cannot be made to speak one of the recognized names.

## What Changes

- **Two more header names are accepted as a session id**, only when none of Switchyard's recognized
  headers carries one: `session_id` and `x-session-affinity`. Order: Switchyard's headers first, then
  `session_id`, then `x-session-affinity`.
- **A usable session id** is 1 to 256 visible ASCII characters (no spaces or control characters); anything else
  is ignored, never an error.
- **The id is used everywhere a session id already is**: the decision, the usage rows (so
  `group_by=session` and the switch report see it) and Switchyard's per-session algorithm state.
- **Docs** say which headers are recognized and how to make OpenClaw send one.
- **Out of scope:** reading `prompt_cache_key` from the request body (it is a cache-routing hint that a client
  may set to a constant shared by many conversations, which would merge unrelated requests into one session),
  `x-client-request-id` (it is per request, not per session), and any change to how Switchyard uses a session.

## Capabilities

### Modified Capabilities
- `gateway-api`: the session id of a request.

## Impact

`src/server.rs` (`request_info`). Docs: `routing.md`, `clients.md`. No config, schema or API change.
