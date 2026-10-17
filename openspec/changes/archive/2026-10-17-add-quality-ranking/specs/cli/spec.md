# Spec Delta

## ADDED Requirements

### Requirement: The migration carries measured quality
`humpyard migrate-modelrelay` SHALL write `quality` on a generated target from the model's score only when ModelRelay's data marks the score as measured; an estimated score SHALL be written as a comment and SHALL become a declared `quality` only with `--estimated-quality`. The report SHALL count the targets with a declared quality.

#### Scenario: Measured and estimated
- **WHEN** one model has a measured score of 0.74 and another an estimated 0.58
- **THEN** the first target declares `quality = 0.74`, the second carries the estimate only as a comment, and the report says 1 of 2 declare a quality
