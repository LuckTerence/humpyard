# 0019. Declared capabilities remove targets that cannot serve a request

- Status: proposed
- Date: 2026-10-15
- Supersedes: none
- Change: `openspec/changes/add-capability-eligibility`

## Context

A model has limits the gateway does not know: a context window, and whether it supports tools or images.
Today a request that exceeds a window is sent anyway, rejected by the provider, and moved on by the
`overflow` fallback, which costs a round trip and a recorded failure. A request that uses tools, sent to a
model without tool support, may fail or be answered without the tools. The planning step (ADR 0008, 0017,
0018) already removes targets before the algorithm runs; a target that cannot serve the request belongs
there. Two things make it delicate: the gateway's prompt estimate errs high on purpose, so using it to
exclude targets would wrongly remove ones that fit, and declared data can be stale.

## Decision

An endpoint may declare `context_window`, `tools` and `vision`; undeclared means capable. In the planning
step, after classes and policy and before health, a target is removed for a request when none of its
endpoints can serve it. A removal is made only on certainty: a declared `false`, or a prompt estimate that
errs low (not high) and still exceeds every declared window. When nothing can serve the request, it is
refused with 400 `no_capable_target` and a named reason before any upstream call. Capability only
removes, is recorded on the decision and counted, and capability proposals from public data are only ever
suggestions (ADR 0014).

Alternatives considered: relying on failure and the `overflow` fallback (the status quo; it costs a failed
call and cannot catch missing tool support); the existing high estimate (it excludes targets that fit,
which is the costly direction); probing providers for limits at runtime (extra network on the serving
path, against ADR 0014's spirit); learning limits from failures (nondeterministic and slow to correct);
failing open when nothing is capable (it sends a request known to fail upstream, unlike health, where the
signal is soft).

## Consequences

Doomed requests stop reaching targets that cannot take them, silent degradation from dropped tools becomes
a clear error, and the decision log says which target was removed and why. A stale or wrong declaration
refuses requests until corrected, so the message names it and the importer reports drift. Borderline
prompts still cost one failed call, which is the price of never removing a target that could have served.
Endpoint-level skipping inside a target remains a later step. Decisions gain a stored `incapable_targets`
field (ledger schema v6) holding configuration names only.
