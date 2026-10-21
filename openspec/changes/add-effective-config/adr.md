# ADR Review

## Reviewed

ADR 0025 (the UI has no state of its own; effective configuration with provenance and a fingerprint are the base for
everything else). This change is its first buildable piece and is read-only, so it adds no way to change anything.
ADR 0016 (metrics use bounded labels): the fingerprint label is a single live series per process, with the old one
removed on reload.

## Outcome

No new ADR. `read_config` is recorded in the design as a step toward profiles.
