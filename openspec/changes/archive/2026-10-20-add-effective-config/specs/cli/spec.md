# Spec Delta

## ADDED Requirements

### Requirement: ctl config shows the effective configuration
`humpyard ctl config` SHALL show the gateway's effective configuration as text with the source of each target, print the response body unchanged with `--json`, and print only the fingerprint with `--fingerprint`. A refusal because the key may not read the configuration SHALL exit 1.

#### Scenario: Fingerprint for a drift check
- **WHEN** `humpyard ctl config --fingerprint` is run against a gateway
- **THEN** it prints the fingerprint of the configuration that gateway is serving and nothing else

#### Scenario: A key that may not read it
- **WHEN** the key lacks `read_config`
- **THEN** the command prints the gateway's message and exits 1

### Requirement: check-config prints the fingerprint
`humpyard check-config` SHALL print the fingerprint the configuration would have when loaded, computed from the same files by the same function the gateway uses, so a repository can be compared with a running gateway.

#### Scenario: Matching files
- **WHEN** `check-config` is run on the files a gateway loaded
- **THEN** the fingerprint it prints equals the one the gateway reports
