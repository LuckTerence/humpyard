# ADR Review

## Reviewed

ADR 0022 (discovery: the provider must list a model before it is used) is the model for the live check. ADR 0025
(the UI has no state of its own; keys are credentials bounded by profiles) is consistent: `init` only creates a new
file and edits nothing, and the exposure rule mirrors the sharing gate.

## Outcome

No new ADR: this applies accepted principles to a new entry point.
