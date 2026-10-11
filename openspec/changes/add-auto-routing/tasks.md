# Tasks

## 1. The auto pattern (configuration and docs)

- [ ] 1.1 Example rule set and `docs/routing.md` recipe: one `auto` route, intent rules (private tag, sub-agent, long prompt, tools), safe catch-all; verify each rule with `explain` tests
- [ ] 1.2 Document client-side intent hints (static provider headers, one provider entry per intent) in docs/clients.md

## 2. Classifier-backed rule action (only if rules prove too coarse)

- [ ] 2.1 Proposal and ADR for a `classify` action with timeout and fallback pool
- [ ] 2.2 Implementation, tests (slow, failing and inconclusive classifier all fall back), explain support

## 3. Depends on the router decision log

- [ ] 3.1 Review real decisions to tune the rules; revisit fail-closed private routing and stickiness
