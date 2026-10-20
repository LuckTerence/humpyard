# ADR Review

## Reviewed

ADR 0025 (the UI has no state of its own) makes a CLI and a UI clients of one API, with configuration changes
only as patches or expiring overrides. This change is the read-only first client and adds no way to change
anything. ADR 0022's rule that `serve` never fetches is unaffected: `ctl` is a separate command.

## Outcome

No new ADR: it applies ADR 0025 to its first client.
