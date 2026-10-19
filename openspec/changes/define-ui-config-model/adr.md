# ADR Review

## Reviewed

ADR 0014 (suggestions are proposed, never applied at runtime), ADR 0015 (advisory, fails open), ADR 0022 (generated
file kept separate), ADR 0018 (health as a planning input). This change follows the same pattern: nothing
changes the running gateway except reviewed configuration, an explicit separate generated file, and bounded
runtime inputs that fail safe.

The architecture document's "Direction: operating humpyard" says database-managed keys sit beside config keys
"so config-as-code and runtime management can coexist". This change keeps that goal but bounds it: database keys
are credentials that can only narrow a profile defined in the file.

## Outcome

New ADR 0025 records the decision. `docs/architecture.md` and `docs/roadmap.md` are updated to match.
