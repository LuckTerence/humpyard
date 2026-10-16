# Tasks

- [ ] 1.1 `request_info`: the two fallback headers after Switchyard's, validated (1 to 256 visible ASCII), written into `Metadata.session_id`; unit tests for order, priority, blank and overlong values, spaces and control characters, and `x-client-request-id` being ignored
- [ ] 1.2 End to end: requests with a `session_id` header appear as one session in `group_by=session` and on the decision, and requests with an unusable value are served with no session
- [ ] 1.3 Docs: the recognized headers in `routing.md`, and how to make OpenClaw send a session id (`compat.sendSessionAffinityHeaders`) in `clients.md`; the roadmap's session item notes it
