# Spec Delta

## ADDED Requirements

### Requirement: ctl is a read-only client for a running gateway
The CLI SHALL provide `humpyard ctl` with the subcommands `status`, `models`, `health`, `stats`, `decisions`, `quota`, `explain` and `whoami`, each calling an existing gateway endpoint with the caller's key. It SHALL NOT change the gateway's configuration, keys or state, and SHALL print the response body unchanged with `--json`.

#### Scenario: A summary
- **WHEN** `humpyard ctl status` is run against a reachable gateway
- **THEN** it prints whether the gateway is reachable, the loaded configuration version, any endpoints being skipped and the key's spend

#### Scenario: JSON for scripts
- **WHEN** `--json` is given
- **THEN** the output is the endpoint's response body without reformatting

### Requirement: The key is read from the environment and protected
The gateway address SHALL come from `--url` or `HUMPYARD_URL`. The key SHALL be read only from the environment variable named by `--key-env` (default `HUMPYARD_API_KEY`), SHALL NOT be accepted as a flag value, and SHALL NOT be printed or logged. Redirects SHALL NOT be followed.

#### Scenario: Key variable unset
- **WHEN** the named variable is unset or empty
- **THEN** the command fails naming the variable and sends no request

#### Scenario: A redirect
- **WHEN** the gateway answers with a redirect
- **THEN** the command does not follow it and reports the redirect as an error

### Requirement: Plain HTTP only to this machine
A `http://` address SHALL be accepted only for a loopback host unless `--allow-insecure-http` is given; `https://` SHALL be accepted for any host.

#### Scenario: Plain HTTP to another host
- **WHEN** the address is `http://` and the host is not loopback and the flag is absent
- **THEN** the command refuses before sending anything and says why

#### Scenario: A port-forward
- **WHEN** the address is `http://127.0.0.1:8080`
- **THEN** the request is sent

### Requirement: Exit codes distinguish refusal from failure
The command SHALL exit 0 when the gateway answers successfully, 1 when it answers 401, 403, 404 or 429, and 2 when it cannot be reached or its answer cannot be understood.

#### Scenario: A wrong key
- **WHEN** the gateway answers 401
- **THEN** the command exits 1 and prints the status and the gateway's message

#### Scenario: Gateway down
- **WHEN** the connection fails
- **THEN** the command exits 2
