# ADR Review

## Reviewed

ADR 0009 (endpoint health lives in the pool and fails open) deferred exactly this and gave the
reason: a policy cannot know whether a route has a healthy alternative. This change keeps its core
promise (health never denies service) and answers the deferred reason with a fail-open second pass in
the planning step, where the route's topology is known. ADR 0008 (routing policy seam) and ADR 0017
(strict routes) define the planning step it extends. ADR 0016 governs the new metric's labels.

## Outcome

The relationship between health and eligibility is a durable commitment: health may remove a target
from a plan only when another remains. That is ADR 0018. It extends ADR 0009 and does not supersede it.

## New ADRs

- [0018](../../../docs/adr/0018-cold-targets-are-skipped-only-when-an-alternative-remains.md): Cold targets are skipped only when an alternative remains.
