# Design

## Decisions

- Add the two statuses to the existing `is_unavailable_answer`, so health counting, within-target failover, the
  `unavailable` class, the relabel/restore round trip and the default-on rule for pools all follow without
  new mechanism.
- 402 is endpoint-level (this key has no credit here) but not request-level, so it belongs with 401/403, not
  with 400/422.
