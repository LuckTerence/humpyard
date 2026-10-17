# ADR Review

## Reviewed

ADR 0014 (public data is only ever a suggestion) governs the output: a printed fragment to review and merge,
never an applied change. ADR 0021 (strengths) defines what the carried-over tags may do. ADR 0019
(capabilities) defines what the copied windows may do. ADR 0016 and the secrets rules (no plaintext secrets in
config or logs) govern keys: only environment variable names are written.

## Outcome

No decision needs superseding. The command is a new surface that follows ADR 0014.

## New ADRs

None; the choices are recorded in `design.md` and the migration guide.
