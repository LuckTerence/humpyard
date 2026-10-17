# Spec Delta

## ADDED Requirements

### Requirement: migrate-modelrelay command
The CLI SHALL provide `humpyard migrate-modelrelay` that reads a ModelRelay setup from its admin API (`--url`, with the admin token taken from the environment variable named by `--token-env`) or from saved responses (`--config FILE --models FILE`), and prints a humpyard configuration fragment containing providers, one target per usable model, and pool routes. It SHALL NOT modify any configuration file, SHALL NOT overwrite an existing `--out` file, and SHALL NOT request or write any API key value.

#### Scenario: From saved responses
- **WHEN** the command is run with `--config` and `--models` files
- **THEN** it prints a fragment and needs no network and no token

#### Scenario: Keys are never copied
- **WHEN** a provider has a key in ModelRelay
- **THEN** the fragment contains `api_key_env = "<PROVIDER>_API_KEY"` and no key value, and the key endpoint is never requested

#### Scenario: Existing output file
- **WHEN** `--out` names a file that exists
- **THEN** the command fails without writing

### Requirement: Drop-in routes
The fragment SHALL define routes named `auto-fastest` and `tag:<strength>` for each strength present, and a `+min_ctx:N` variant of each for every `--min-ctx N`, so existing clients keep their model strings. `auto-fastest` SHALL be a `fastest` route over every generated target and `tag:<strength>` a `fastest` pool of that strength.

#### Scenario: Tag routes
- **WHEN** generated targets carry `general` and `coding` and `--min-ctx 32000` is given
- **THEN** the fragment defines `tag:general`, `tag:coding`, `tag:general+min_ctx:32000`, `tag:coding+min_ctx:32000`, `auto-fastest` and `auto-fastest+min_ctx:32000`

### Requirement: Targets from models
Each model that is enabled, not banned and on an enabled provider that has a key SHALL become a target with `class = "public"` (overridable), `strengths` limited to the shared vocabulary, and its reported context window as a declared `context_window`.

#### Scenario: Banned and unkeyed models are skipped
- **WHEN** a model is banned, or its provider is disabled or has no key
- **THEN** it has no target and appears in the report with the reason

### Requirement: Report and self-check
The command SHALL report what did not carry over (extra keys of a multi-account provider, unknown providers, unknown tags, skipped models) and SHALL load the fragment through the configuration parser with placeholder keys, failing with the parser's message if it does not load.

#### Scenario: A fragment that would not load
- **WHEN** the generated fragment fails to load
- **THEN** the command exits non-zero and prints the error instead of declaring success
